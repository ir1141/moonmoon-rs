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
