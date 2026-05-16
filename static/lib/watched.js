function normalizeEntry(entry) {
  if (entry === true) return { updated: 0 };
  if (typeof entry === "number" && Number.isFinite(entry) && entry >= 0) {
    return { updated: entry };
  }
  if (entry && typeof entry === "object" && !Array.isArray(entry)) {
    const updated = Number(entry.updated || 0);
    return { updated: Number.isFinite(updated) && updated >= 0 ? updated : 0 };
  }
  return null;
}

export function watchedVodIds(store) {
  if (!store || typeof store !== "object" || Array.isArray(store)) return [];

  return Object.entries(store)
    .filter(([id, entry]) => id && normalizeEntry(entry))
    .map(([id]) => id);
}

export function hasWatchedVod(store, id) {
  if (!id || !store || typeof store !== "object" || Array.isArray(store)) {
    return false;
  }

  return !!normalizeEntry(store[id]);
}

export function markWatchedVod(
  store,
  id,
  updated = Date.now(),
  maxEntries = 500,
) {
  if (!id)
    return store && typeof store === "object" && !Array.isArray(store)
      ? { ...store }
      : {};

  const next = {};
  if (store && typeof store === "object" && !Array.isArray(store)) {
    for (const [key, entry] of Object.entries(store)) {
      const normalized = normalizeEntry(entry);
      if (key && normalized) next[key] = normalized;
    }
  }

  next[id] = { updated };

  const entries = Object.entries(next).sort(
    (a, b) => (b[1].updated || 0) - (a[1].updated || 0),
  );
  return Object.fromEntries(entries.slice(0, maxEntries));
}

export function mergeWatched(local, remote, maxEntries = 500) {
  const merged = {};
  let changed = false;

  function addEntries(store, isRemote) {
    if (!store || typeof store !== "object" || Array.isArray(store)) return;

    for (const [id, entry] of Object.entries(store)) {
      const normalized = normalizeEntry(entry);
      if (!id || !normalized) continue;

      const existing = merged[id];
      if (!existing || normalized.updated > existing.updated) {
        if (isRemote) {
          const localEntry = normalizeEntry(local && local[id]);
          if (!localEntry || normalized.updated > localEntry.updated) {
            changed = true;
          }
        }
        merged[id] = normalized;
      }
    }
  }

  addEntries(local, false);
  addEntries(remote, true);

  const entries = Object.entries(merged).sort(
    (a, b) => (b[1].updated || 0) - (a[1].updated || 0),
  );

  return {
    changed,
    merged: Object.fromEntries(entries.slice(0, maxEntries)),
  };
}
