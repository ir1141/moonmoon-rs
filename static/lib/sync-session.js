import {
  HISTORY_KEY,
  buildSyncBlob,
  historyFromBlob,
  loadHistoryStore,
  mergeHistory,
  saveHistoryStore,
} from "./history-state.js";
import { isValidToken } from "./token.js";
import { storageGet, storageRemove, storageSet } from "./storage.js";

export const TOKEN_KEY = "moonmoon_sync_token";
export const META_KEY = "moonmoon_sync_meta";
export const LOCAL_POLL_MS = 2_000;
export const MAX_PUSH_STALENESS_MS = 5_000;
export const REMOTE_POLL_MS = 15_000;

/**
 * Keep one browser's Watch history converged with its remote sync blob.
 * Browser effects and HTTP are supplied as adapters so callers and tests use
 * the same interface.
 *
 * @param {{
 *   storage: {
 *     getItem?(key: string): string | null,
 *     setItem?(key: string, value: string): void,
 *     removeItem?(key: string): void,
 *   } | null,
 *   transport: {
 *     pull(token: string, options?: { boot: true }): Promise<any>,
 *     push(token: string, body: any, options?: { keepalive: true }): Promise<void>,
 *   },
 *   runtime: {
 *     now(): number,
 *     isVisible(): boolean,
 *     onWindow(type: string, listener: (event?: any) => void): () => void,
 *     onDocument(type: string, listener: (event?: any) => void): () => void,
 *     dispatchHistoryChanged(): void,
 *     setTimeout(listener: () => void, delay: number): any,
 *     clearTimeout(id: any): void,
 *     setInterval(listener: () => void, delay: number): any,
 *     clearInterval(id: any): void,
 *     warn?(message: string, error: unknown): void,
 *   },
 * }} options
 */
export function createSyncSession({ storage, transport, runtime }) {
  let started = false;
  let dirty = false;
  let applyingRemote = false;
  let lastPushAt = null;
  let lastHistoryStr = storageGet(storage, HISTORY_KEY) || "";
  let pushTimer = null;
  let localPollTimer = null;
  let remotePollTimer = null;
  let pullInFlight = null;
  let pushInFlight = null;
  const removeListeners = [];

  function warn(message, error) {
    if (runtime.warn) runtime.warn(message, error);
  }

  function getToken() {
    const token = storageGet(storage, TOKEN_KEY) || "";
    return isValidToken(token) ? token : "";
  }

  function setToken(token) {
    if (token && isValidToken(token)) storageSet(storage, TOKEN_KEY, token);
    else storageRemove(storage, TOKEN_KEY);
  }

  function getHistory() {
    return loadHistoryStore(storage);
  }

  function hasHistory(store) {
    return Object.keys(store).length > 0;
  }

  function publishHistory(store) {
    applyingRemote = true;
    saveHistoryStore(storage, store);
    lastHistoryStr = storageGet(storage, HISTORY_KEY) || "";
    runtime.dispatchHistoryChanged();
    applyingRemote = false;
  }

  function clearPushTimer() {
    if (pushTimer === null) return;
    runtime.clearTimeout(pushTimer);
    pushTimer = null;
  }

  function schedulePush() {
    if (!dirty || !getToken() || pullInFlight || pushInFlight) return;

    const elapsed = lastPushAt === null ? Infinity : runtime.now() - lastPushAt;
    if (elapsed >= MAX_PUSH_STALENESS_MS) {
      void push();
      return;
    }
    if (pushTimer !== null) return;

    pushTimer = runtime.setTimeout(
      () => {
        pushTimer = null;
        void push();
      },
      MAX_PUSH_STALENESS_MS - Math.max(0, elapsed),
    );
  }

  function noteLocalChange() {
    if (applyingRemote) return;
    const current = storageGet(storage, HISTORY_KEY) || "";
    if (current === lastHistoryStr) return;
    lastHistoryStr = current;
    dirty = true;
  }

  function historyChanged() {
    noteLocalChange();
    schedulePush();
  }

  // A hidden page may be frozen or discarded before a timer fires, so flush
  // now, and skip the pre-push pull: its round trip would not finish.
  function flushOnHide() {
    noteLocalChange();
    if (dirty) void push({ beforeHide: true });
  }

  function applyRemote(data) {
    const local = getHistory();
    const remote = historyFromBlob(data.blob);
    const remoteContribution = mergeHistory(local, remote);
    if (remoteContribution.changed) publishHistory(remoteContribution.merged);
    storageSet(
      storage,
      META_KEY,
      JSON.stringify({
        last_pulled_updated_at: data.updated_at || 0,
        last_pulled_at: runtime.now(),
      }),
    );
    return {
      remoteChanged: remoteContribution.changed,
      localAhead: mergeHistory(remote, local).changed,
    };
  }

  // The server stores whole blobs last-write-wins, so merge the current
  // remote in first; otherwise another device's push since our last pull is
  // overwritten. A failed pull falls back to pushing what we have.
  async function pullBeforePush(token) {
    try {
      const data = await transport.pull(token);
      if (data) applyRemote(data);
    } catch (error) {
      warn("pre-push pull failed", error);
    }
  }

  async function push({ beforeHide = false } = {}) {
    clearPushTimer();
    const token = getToken();
    if (!token) {
      dirty = false;
      return null;
    }
    if (pullInFlight) {
      dirty = true;
      return null;
    }
    if (pushInFlight) {
      dirty = true;
      return pushInFlight;
    }

    dirty = false;
    lastPushAt = runtime.now();
    pushInFlight = (async () => {
      if (!beforeHide) await pullBeforePush(token);
      const body = {
        blob: buildSyncBlob(getHistory()),
        updated_at: runtime.now(),
      };
      if (beforeHide) await transport.push(token, body, { keepalive: true });
      else await transport.push(token, body);
      return body;
    })();
    try {
      return await pushInFlight;
    } catch (error) {
      dirty = true;
      warn("push failed", error);
      return null;
    } finally {
      pushInFlight = null;
      schedulePush();
    }
  }

  // Resolves to null only when there is no token or the pull failed; a
  // missing remote blob is a successful pull with nothing to apply.
  async function pull({ boot = false } = {}) {
    const token = getToken();
    if (!token) return null;
    if (pullInFlight) return pullInFlight.catch(() => null);

    pullInFlight = (async () => {
      const data = boot
        ? await transport.pull(token, { boot: true })
        : await transport.pull(token);
      if (!data) {
        if (hasHistory(getHistory())) dirty = true;
        return { changed: false, updated_at: null };
      }
      const { remoteChanged, localAhead } = applyRemote(data);
      if (localAhead) dirty = true;
      return { changed: remoteChanged, updated_at: data.updated_at };
    })();

    try {
      return await pullInFlight;
    } catch (error) {
      warn("pull failed", error);
      return null;
    } finally {
      pullInFlight = null;
      schedulePush();
    }
  }

  async function connect(token) {
    if (!isValidToken(token)) return false;
    setToken(token);
    lastPushAt = null;
    lastHistoryStr = storageGet(storage, HISTORY_KEY) || "";
    return (await pull()) !== null;
  }

  function disconnect() {
    setToken("");
    clearPushTimer();
    dirty = false;
    lastPushAt = null;
  }

  async function pullIfVisible() {
    if (runtime.isVisible()) await pull();
  }

  async function start() {
    if (started) return pullInFlight;
    started = true;

    // This remains a fallback for other tabs and older writers. Same-page
    // writers notify the session directly with moonmoon:historyChanged.
    localPollTimer = runtime.setInterval(historyChanged, LOCAL_POLL_MS);
    remotePollTimer = runtime.setInterval(() => {
      void pullIfVisible();
    }, REMOTE_POLL_MS);

    removeListeners.push(
      runtime.onWindow("moonmoon:historyChanged", historyChanged),
      runtime.onWindow("storage", (event) => {
        if (event && event.key === HISTORY_KEY) historyChanged();
      }),
      runtime.onWindow("focus", () => {
        void pullIfVisible();
      }),
      runtime.onWindow("pageshow", () => {
        void pullIfVisible();
      }),
      // pagehide fires after every beforeunload handler, so it also sees the
      // player's final position save.
      runtime.onWindow("pagehide", flushOnHide),
      runtime.onDocument("visibilitychange", () => {
        if (runtime.isVisible()) void pull();
        else flushOnHide();
      }),
    );

    // Guaranteed migration point, even before a Watch history read.
    loadHistoryStore(storage);
    lastHistoryStr = storageGet(storage, HISTORY_KEY) || "";
    return pull({ boot: true });
  }

  function stop() {
    clearPushTimer();
    if (localPollTimer !== null) runtime.clearInterval(localPollTimer);
    if (remotePollTimer !== null) runtime.clearInterval(remotePollTimer);
    localPollTimer = null;
    remotePollTimer = null;
    while (removeListeners.length) removeListeners.pop()();
    started = false;
  }

  return {
    start,
    stop,
    connect,
    disconnect,
    getToken,
    setToken,
    pull,
    push,
  };
}
