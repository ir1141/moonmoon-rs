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
 *     pull(token: string): Promise<any>,
 *     push(token: string, body: any): Promise<void>,
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

  function historyChanged() {
    if (applyingRemote) return;
    const current = storageGet(storage, HISTORY_KEY) || "";
    if (current === lastHistoryStr) return;
    lastHistoryStr = current;
    dirty = true;
    schedulePush();
  }

  async function push() {
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
    const body = {
      blob: buildSyncBlob(getHistory()),
      updated_at: runtime.now(),
    };
    pushInFlight = transport.push(token, body);
    try {
      await pushInFlight;
      return body;
    } catch (error) {
      dirty = true;
      warn("push failed", error);
      return null;
    } finally {
      pushInFlight = null;
      schedulePush();
    }
  }

  async function pull() {
    const token = getToken();
    if (!token) return null;
    if (pullInFlight) return pullInFlight;

    pullInFlight = (async () => {
      const data = await transport.pull(token);
      const local = getHistory();
      if (!data) {
        if (hasHistory(local)) dirty = true;
        return null;
      }

      const remote = historyFromBlob(data.blob);
      const remoteContribution = mergeHistory(local, remote);
      const localContribution = mergeHistory(remote, local);
      if (remoteContribution.changed) publishHistory(remoteContribution.merged);
      if (localContribution.changed) dirty = true;

      storageSet(
        storage,
        META_KEY,
        JSON.stringify({
          last_pulled_updated_at: data.updated_at || 0,
          last_pulled_at: runtime.now(),
        }),
      );
      return {
        changed: remoteContribution.changed,
        updated_at: data.updated_at,
      };
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
    await pull();
    return true;
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
      runtime.onWindow("beforeunload", () => {
        if (dirty) void push();
      }),
      runtime.onDocument("visibilitychange", () => {
        void pullIfVisible();
      }),
    );

    // Guaranteed migration point, even before a Watch history read.
    loadHistoryStore(storage);
    lastHistoryStr = storageGet(storage, HISTORY_KEY) || "";
    return pull();
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
