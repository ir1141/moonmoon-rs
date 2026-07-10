import { describe, expect, test } from "bun:test";
import {
  LOCAL_POLL_MS,
  MAX_PUSH_STALENESS_MS,
  REMOTE_POLL_MS,
  createSyncSession,
} from "../static/lib/sync-session.js";

const TOKEN = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
const TOKEN_KEY = "moonmoon_sync_token";
const HISTORY_KEY = "moonmoon_history";

function createStorage(entries = {}) {
  const values = new Map(Object.entries(entries));
  return {
    getItem(key) {
      return values.has(key) ? values.get(key) : null;
    },
    setItem(key, value) {
      values.set(key, String(value));
    },
    removeItem(key) {
      values.delete(key);
    },
  };
}

function createRuntime() {
  let now = 0;
  let visible = true;
  let nextTimerId = 1;
  const timeouts = new Map();
  const intervals = new Map();
  const windowListeners = new Map();
  const documentListeners = new Map();

  function addListener(target, type, listener) {
    const listeners = target.get(type) || new Set();
    listeners.add(listener);
    target.set(type, listeners);
    return () => listeners.delete(listener);
  }

  function emit(target, type, event = {}) {
    for (const listener of target.get(type) || []) listener(event);
  }

  async function flush() {
    await Promise.resolve();
    await Promise.resolve();
  }

  return {
    now: () => now,
    isVisible: () => visible,
    setVisible(value) {
      visible = value;
    },
    onWindow: (type, listener) => addListener(windowListeners, type, listener),
    onDocument: (type, listener) =>
      addListener(documentListeners, type, listener),
    emitWindow: async (type, event) => {
      emit(windowListeners, type, event);
      await flush();
    },
    emitDocument: async (type, event) => {
      emit(documentListeners, type, event);
      await flush();
    },
    dispatchHistoryChanged() {
      emit(windowListeners, "moonmoon:historyChanged");
    },
    setTimeout(listener, delay) {
      const id = nextTimerId++;
      timeouts.set(id, { at: now + delay, listener });
      return id;
    },
    clearTimeout(id) {
      timeouts.delete(id);
    },
    setInterval(listener, delay) {
      const id = nextTimerId++;
      intervals.set(id, { delay, next: now + delay, listener });
      return id;
    },
    clearInterval(id) {
      intervals.delete(id);
    },
    async advance(milliseconds) {
      const end = now + milliseconds;
      while (true) {
        const dueTimeout = [...timeouts.entries()]
          .filter(([, timer]) => timer.at <= end)
          .sort((a, b) => a[1].at - b[1].at)[0];
        const dueInterval = [...intervals.entries()]
          .filter(([, timer]) => timer.next <= end)
          .sort((a, b) => a[1].next - b[1].next)[0];
        const timeoutAt = dueTimeout?.[1].at ?? Infinity;
        const intervalAt = dueInterval?.[1].next ?? Infinity;
        if (timeoutAt === Infinity && intervalAt === Infinity) break;
        if (timeoutAt <= intervalAt) {
          now = timeoutAt;
          timeouts.delete(dueTimeout[0]);
          dueTimeout[1].listener();
        } else {
          now = intervalAt;
          dueInterval[1].next += dueInterval[1].delay;
          dueInterval[1].listener();
        }
        await flush();
      }
      now = end;
      await flush();
    },
  };
}

function createTransport(initial = null) {
  let remote = initial;
  const pulls = [];
  const pushes = [];
  return {
    pulls,
    pushes,
    setRemote(value) {
      remote = value;
    },
    async pull(token) {
      pulls.push(token);
      return remote;
    },
    async push(token, body) {
      pushes.push({ token, body });
      remote = body;
    },
  };
}

function history(position, updated = position * 10) {
  return {
    "vod-42": {
      state: "in_progress",
      time: position,
      updated,
    },
  };
}

describe("Sync session outbound durability", () => {
  test("continuous playback cannot postpone pushes indefinitely", async () => {
    const storage = createStorage({ [TOKEN_KEY]: TOKEN });
    const runtime = createRuntime();
    const transport = createTransport();
    const session = createSyncSession({ storage, runtime, transport });
    await session.start();

    storage.setItem(HISTORY_KEY, JSON.stringify(history(100)));
    await runtime.advance(LOCAL_POLL_MS);
    expect(transport.pushes).toHaveLength(1);
    expect(transport.pushes[0].body.blob.history["vod-42"].time).toBe(100);

    storage.setItem(HISTORY_KEY, JSON.stringify(history(200)));
    await runtime.advance(LOCAL_POLL_MS);
    storage.setItem(HISTORY_KEY, JSON.stringify(history(300)));
    await runtime.advance(LOCAL_POLL_MS);

    expect(transport.pushes).toHaveLength(1);
    storage.setItem(HISTORY_KEY, JSON.stringify(history(400)));
    await runtime.advance(MAX_PUSH_STALENESS_MS - 2 * LOCAL_POLL_MS);
    expect(transport.pushes).toHaveLength(2);
    expect(transport.pushes[1].body.blob.history["vod-42"].time).toBe(400);
  });
});

describe("Sync session inbound freshness", () => {
  test("a meaningful remote resume repairs newer mobile startup noise", async () => {
    const storage = createStorage({
      [TOKEN_KEY]: TOKEN,
      [HISTORY_KEY]: JSON.stringify({
        "vod-42": { state: "in_progress", time: 0, updated: 20_000 },
      }),
    });
    const runtime = createRuntime();
    const transport = createTransport({
      blob: { v: 2, history: history(937, 10_000) },
      updated_at: 10_000,
    });
    const session = createSyncSession({ storage, runtime, transport });
    await session.start();

    const local = JSON.parse(storage.getItem(HISTORY_KEY) || "{}");
    expect(local["vod-42"].time).toBe(937);
  });

  test("an already-open device pulls when it becomes active", async () => {
    const storage = createStorage({ [TOKEN_KEY]: TOKEN });
    const runtime = createRuntime();
    const transport = createTransport();
    const session = createSyncSession({ storage, runtime, transport });
    await session.start();

    transport.setRemote({
      blob: { v: 2, history: history(937) },
      updated_at: 9_370,
    });
    await runtime.emitWindow("focus");

    const local = JSON.parse(storage.getItem(HISTORY_KEY) || "{}");
    expect(local["vod-42"].time).toBe(937);
    expect(transport.pulls).toHaveLength(2);
  });

  test("remote polling runs only while visible", async () => {
    const storage = createStorage({ [TOKEN_KEY]: TOKEN });
    const runtime = createRuntime();
    const transport = createTransport();
    const session = createSyncSession({ storage, runtime, transport });
    await session.start();

    await runtime.advance(REMOTE_POLL_MS);
    expect(transport.pulls).toHaveLength(2);

    runtime.setVisible(false);
    await runtime.advance(REMOTE_POLL_MS);
    expect(transport.pulls).toHaveLength(2);

    runtime.setVisible(true);
    await runtime.emitDocument("visibilitychange");
    expect(transport.pulls).toHaveLength(3);
  });
});
