export function getCachedPartDurations(store, vodId, partCount) {
  const entry = store && store[vodId];
  if (!entry || !Array.isArray(entry.durations)) return null;
  if (entry.durations.length !== partCount) return null;
  return entry.durations;
}

export function savePartDuration(
  store,
  vodId,
  partCount,
  index,
  duration,
  maxEntries,
  now,
) {
  if (!(duration > 0)) return store;
  if (index < 0 || index >= partCount) return store;

  const next = { ...store };
  const existing = next[vodId];
  let durations;
  if (
    existing &&
    Array.isArray(existing.durations) &&
    existing.durations.length === partCount
  ) {
    durations = existing.durations.slice();
  } else {
    durations = new Array(partCount).fill(0);
  }
  durations[index] = duration;
  next[vodId] = { durations, updated: now };

  const keys = Object.keys(next);
  if (keys.length > maxEntries) {
    keys.sort((a, b) => (next[a].updated || 0) - (next[b].updated || 0));
    while (keys.length > maxEntries) {
      delete next[keys.shift()];
    }
  }
  return next;
}
