import { RESUME_MIN_SECONDS } from "./history-state.js";

export function selectContinueWatchingEntries(store, options = {}) {
  const limit = options.limit || 1;
  const minTime = options.minTime || RESUME_MIN_SECONDS;

  if (!store || typeof store !== "object" || Array.isArray(store)) {
    return [];
  }

  return Object.entries(store)
    .filter(([id, entry]) => {
      if (!id || !entry || typeof entry !== "object" || Array.isArray(entry)) {
        return false;
      }

      const time = Number(entry.time);
      const updated = Number(entry.updated);
      return (
        entry.state === "in_progress" &&
        Number.isFinite(time) &&
        Number.isFinite(updated) &&
        time > minTime &&
        updated > 0
      );
    })
    .map(([id, entry]) => ({
      id,
      time: Math.floor(Number(entry.time)),
      updated: Number(entry.updated),
    }))
    .sort((a, b) => b.updated - a.updated)
    .slice(0, limit);
}

export function buildContinueResumeUrl(entry) {
  const params = new URLSearchParams({
    id: entry.id,
    time: String(entry.time),
  });

  return `/history/resume?${params.toString()}`;
}
