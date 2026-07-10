// The single client-side contract for watch history: storage key, entry
// shape, normalization, legacy migration, merging, sync-blob shape, and the
// resume policy. sync-session.js owns cross-device convergence; player.js,
// history.js and vod-cards.js are adapters over this module — nothing else may
// re-declare any of this.
//
// Store shape (localStorage, one entry per VOD id):
//   { <id>: { state: "in_progress", time, updated, part?, localTime? } }
//   { <id>: { state: "watched", updated } }
//
// `updated` is ms since epoch and drives all conflict resolution (per-id
// last-write-wins, local wins ties) and cap eviction. `time` is the global
// resume position in whole seconds; `part`/`localTime` are the player's
// precise per-part position and never leave the client.

export const HISTORY_KEY = "moonmoon_history";
export const LEGACY_RESUME_KEY = "moonmoon_resume";
export const LEGACY_WATCHED_KEY = "moonmoon_watched";

export const MAX_HISTORY_ENTRIES = 1000;

// A resume position at or under this many seconds is noise: not worth
// saving a card bar, a continue block, or an auto-seek for.
export const RESUME_MIN_SECONDS = 10;

export const SYNC_BLOB_VERSION = 2;

/**
 * @typedef {{
 *   state: "in_progress" | "watched",
 *   updated: number,
 *   time?: number,
 *   part?: number,
 *   localTime?: number,
 * }} HistoryEntry
 *
 * @typedef {Record<string, HistoryEntry>} HistoryStore
 */

function storageGet(storage, key) {
  if (!storage) return null;
  if (typeof storage.getItem === "function") return storage.getItem(key);
  if (typeof storage.get === "function") return storage.get(key);
  return null;
}

function storageSet(storage, key, value) {
  if (!storage) return;
  try {
    if (typeof storage.setItem === "function") storage.setItem(key, value);
    else if (typeof storage.set === "function") storage.set(key, value);
  } catch (error) {
    /* storage blocked or quota exceeded */
  }
}

function storageRemove(storage, key) {
  if (!storage) return;
  try {
    if (typeof storage.removeItem === "function") storage.removeItem(key);
    else if (typeof storage.delete === "function") storage.delete(key);
  } catch (error) {
    /* storage blocked */
  }
}

export function readJsonStore(storage, key) {
  try {
    const parsed = JSON.parse(storageGet(storage, key) || "{}");
    return parsed && typeof parsed === "object" && !Array.isArray(parsed)
      ? parsed
      : {};
  } catch (error) {
    return {};
  }
}

function normalizeUpdated(value) {
  const updated = Number(value);
  return Number.isFinite(updated) && updated >= 0 ? updated : null;
}

/**
 * @param {unknown} entry
 * @returns {HistoryEntry | null}
 */
export function normalizeHistoryEntry(entry) {
  if (!entry || typeof entry !== "object" || Array.isArray(entry)) return null;
  const record = /** @type {Record<string, unknown>} */ (entry);
  const updated = normalizeUpdated(record.updated);
  if (updated === null) return null;

  if (record.state === "watched") return { state: "watched", updated };
  if (record.state !== "in_progress") return null;

  const time = Number(record.time);
  /** @type {HistoryEntry} */
  const normalized = {
    state: "in_progress",
    time: Number.isFinite(time) && time >= 0 ? Math.floor(time) : 0,
    updated,
  };
  const part = Number(record.part);
  const localTime = Number(record.localTime);
  if (Number.isInteger(part) && part >= 0) normalized.part = part;
  if (Number.isFinite(localTime) && localTime >= 0) {
    normalized.localTime = localTime;
  }
  return normalized;
}

/**
 * @param {unknown} store
 * @returns {HistoryStore}
 */
export function normalizeHistoryStore(store) {
  /** @type {HistoryStore} */
  const next = {};
  if (!store || typeof store !== "object" || Array.isArray(store)) return next;
  for (const [id, entry] of Object.entries(store)) {
    if (!id) continue;
    const normalized = normalizeHistoryEntry(entry);
    if (normalized) next[id] = normalized;
  }
  return next;
}

function capEntries(store, maxEntries) {
  const entries = Object.entries(store);
  if (entries.length <= maxEntries) return store;
  entries.sort((a, b) => (b[1].updated || 0) - (a[1].updated || 0));
  return Object.fromEntries(entries.slice(0, maxEntries));
}

function normalizeLegacyResumeEntry(entry) {
  if (!entry || typeof entry !== "object" || Array.isArray(entry)) return null;
  const updated = normalizeUpdated(entry.updated);
  if (updated === null) return null;
  return normalizeHistoryEntry({ ...entry, state: "in_progress", updated });
}

function normalizeLegacyWatchedEntry(entry) {
  if (entry === true) return { state: "watched", updated: 0 };
  if (typeof entry === "number") {
    const updated = normalizeUpdated(entry);
    return updated === null ? null : { state: "watched", updated };
  }
  if (!entry || typeof entry !== "object" || Array.isArray(entry)) return null;
  const updated = normalizeUpdated(entry.updated || 0);
  return updated === null ? null : { state: "watched", updated };
}

// Fold the pre-unification split stores into one. A watched entry beats an
// in-progress entry for the same id only when strictly newer — the same rule
// the old cross-store history merge used.
export function migrateLegacyStores(resumeStore, watchedStore) {
  const merged = {};

  if (
    resumeStore &&
    typeof resumeStore === "object" &&
    !Array.isArray(resumeStore)
  ) {
    for (const [id, entry] of Object.entries(resumeStore)) {
      if (!id) continue;
      const normalized = normalizeLegacyResumeEntry(entry);
      if (normalized) merged[id] = normalized;
    }
  }

  if (
    watchedStore &&
    typeof watchedStore === "object" &&
    !Array.isArray(watchedStore)
  ) {
    for (const [id, entry] of Object.entries(watchedStore)) {
      if (!id) continue;
      const normalized = normalizeLegacyWatchedEntry(entry);
      if (!normalized) continue;
      const existing = merged[id];
      if (!existing || normalized.updated > existing.updated) {
        merged[id] = normalized;
      }
    }
  }

  return capEntries(merged, MAX_HISTORY_ENTRIES);
}

// Read the unified store, migrating the legacy split stores in place the
// first time (write the unified key, delete the legacy keys). May write.
export function loadHistoryStore(storage) {
  const raw = storageGet(storage, HISTORY_KEY);
  if (raw !== null && raw !== undefined) {
    try {
      return normalizeHistoryStore(JSON.parse(raw));
    } catch (error) {
      return {};
    }
  }

  const legacyResume = storageGet(storage, LEGACY_RESUME_KEY);
  const legacyWatched = storageGet(storage, LEGACY_WATCHED_KEY);
  if (legacyResume == null && legacyWatched == null) return {};

  const migrated = migrateLegacyStores(
    readJsonStore(storage, LEGACY_RESUME_KEY),
    readJsonStore(storage, LEGACY_WATCHED_KEY),
  );
  storageSet(storage, HISTORY_KEY, JSON.stringify(migrated));
  storageRemove(storage, LEGACY_RESUME_KEY);
  storageRemove(storage, LEGACY_WATCHED_KEY);
  return migrated;
}

export function saveHistoryStore(storage, store) {
  storageSet(storage, HISTORY_KEY, JSON.stringify(store));
}

export function saveResumePosition(
  store,
  id,
  position,
  updated = Date.now(),
  maxEntries = MAX_HISTORY_ENTRIES,
) {
  const next = normalizeHistoryStore(store);
  if (!id) return next;
  const time = Number(position && position.time);
  if (!Number.isFinite(time) || time <= RESUME_MIN_SECONDS) return next;
  const normalized = normalizeHistoryEntry({
    state: "in_progress",
    time,
    part: position && position.part,
    localTime: position && position.localTime,
    updated,
  });
  if (normalized) next[id] = normalized;
  return capEntries(next, maxEntries);
}

function remoteEntryWins(existing, remote) {
  if (!existing) return true;
  if (existing.state === "in_progress" && remote.state === "in_progress") {
    const existingIsMeaningful = existing.time > RESUME_MIN_SECONDS;
    const remoteIsMeaningful = remote.time > RESUME_MIN_SECONDS;
    if (existingIsMeaningful !== remoteIsMeaningful) return remoteIsMeaningful;
  }
  return remote.updated > existing.updated;
}

export function markWatched(
  store,
  id,
  updated = Date.now(),
  maxEntries = MAX_HISTORY_ENTRIES,
) {
  const next = normalizeHistoryStore(store);
  if (!id) return next;
  next[id] = { state: "watched", updated };
  return capEntries(next, maxEntries);
}

// Per-id last-write-wins on `updated`; local wins ties. `changed` means the
// remote side contributed something — the caller should write back and
// re-render only then.
export function mergeHistory(local, remote, maxEntries = MAX_HISTORY_ENTRIES) {
  const merged = normalizeHistoryStore(local);
  let changed = false;

  for (const [id, entry] of Object.entries(normalizeHistoryStore(remote))) {
    const existing = merged[id];
    if (remoteEntryWins(existing, entry)) {
      merged[id] = entry;
      changed = true;
    }
  }

  return { merged: capEntries(merged, maxEntries), changed };
}

// Sync blob shape: { v: 2, history: { <id>: entry } }. Old clients pushed
// { resume, watched } split stores; reads accept both forever so a stale tab
// on another device still round-trips through the server.
export function historyFromBlob(blob) {
  if (!blob || typeof blob !== "object" || Array.isArray(blob)) return {};
  if (blob.history) return normalizeHistoryStore(blob.history);
  return migrateLegacyStores(blob.resume, blob.watched);
}

export function buildSyncBlob(store) {
  return { v: SYNC_BLOB_VERSION, history: store };
}

export function resumePercent(time, durationSeconds) {
  const duration = Number(durationSeconds);
  const position = Number(time);

  if (
    !Number.isFinite(duration) ||
    duration <= 0 ||
    !Number.isFinite(position)
  ) {
    return 0;
  }

  return Math.max(0, Math.min((position / duration) * 100, 100));
}

// Flatten the store into request entries, most recently updated first —
// request order is the recency contract with the server render path.
export function buildHistoryEntries(store) {
  return Object.entries(normalizeHistoryStore(store))
    .map(([id, entry]) =>
      entry.state === "in_progress"
        ? { id, state: "in_progress", time: entry.time, updated: entry.updated }
        : { id, state: "watched", updated: entry.updated },
    )
    .sort((a, b) =>
      b.updated === a.updated
        ? a.id.localeCompare(b.id)
        : b.updated - a.updated,
    );
}

// Wire contract with the server render path (POST /history/vods). The same
// shape is pinned on both sides by tests/fixtures/history-request.json.
export function buildHistoryRequest(entries, sort = "recent") {
  return {
    entries: entries.map((entry) =>
      entry.state === "in_progress" && Number.isFinite(entry.time)
        ? { id: entry.id, state: entry.state, time: Math.floor(entry.time) }
        : { id: entry.id, state: entry.state },
    ),
    sort: sort === "game" ? "game" : "recent",
  };
}
