// `parts` accepts either {duration} objects (the raw server payload) or plain
// numbers (the resolved partDurations array the player walks); both must agree
// on unknown durations or the chat clock drifts from the video.
export function computeChatDelay(totalVodSeconds, parts, maxPartDuration) {
  if (!(totalVodSeconds > 0) || !Array.isArray(parts)) return 0;
  // Estimate unknown part durations at the 3h cap, exactly as the playback
  // timeline does (see initialPartDurations). If the two disagreed, an unknown
  // part would count toward player time but not the YouTube total, inflating
  // the delay and pushing chat ahead of the video.
  const fallback =
    typeof maxPartDuration === "number" && maxPartDuration > 0
      ? maxPartDuration
      : 0;
  let totalYoutube = 0;
  for (const part of parts) {
    const duration = typeof part === "number" ? part : part && part.duration;
    totalYoutube +=
      typeof duration === "number" && duration > 0 ? duration : fallback;
  }
  return Math.max(0, totalVodSeconds - totalYoutube);
}

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
