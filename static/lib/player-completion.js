export const PLAYBACK_COMPLETION_THRESHOLD = 2;

export function shouldFinalizePlaybackAtTick(options) {
  options = options || {};
  const currentPart = Number(options.currentPart);
  const partCount = Number(options.partCount);
  const duration = Number(options.duration);
  const currentTime = Number(options.currentTime);
  const threshold =
    Number.isFinite(options.threshold) && options.threshold >= 0
      ? options.threshold
      : PLAYBACK_COMPLETION_THRESHOLD;

  if (!Number.isInteger(currentPart) || !Number.isInteger(partCount)) {
    return false;
  }
  if (partCount <= 0 || currentPart !== partCount - 1) return false;
  if (!Number.isFinite(duration) || duration <= 0) return false;
  if (!Number.isFinite(currentTime)) return false;

  return currentTime >= Math.max(0, duration - threshold);
}

export function shouldSaveResume(options) {
  return !(options && options.completed === true);
}

// The unload write may only record positions the session earned. Playing
// ticks already persist every second, so a session that never played (or is
// still parked on its last played save) has nothing to add - writing anyway
// would re-stamp a restored position as the newest entry and, through
// last-write-wins sync, erase every other device's progress.
export function shouldSaveOnUnload(options) {
  options = options || {};
  const lastSaved = options.lastPlaybackSavedTime;
  const currentTime = Number(options.currentTime);
  if (lastSaved === null || lastSaved === undefined) return false;
  if (!Number.isFinite(currentTime)) return false;
  return Math.floor(currentTime) !== Math.floor(Number(lastSaved));
}
