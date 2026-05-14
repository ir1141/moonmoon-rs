export function formatChatTimestamp(seconds) {
  const numericSeconds = Number(seconds);
  const safeSeconds = Number.isFinite(numericSeconds) ? numericSeconds : 0;
  const floored = Math.max(0, Math.floor(safeSeconds));
  const hours = Math.floor(floored / 3600);
  const minutes = Math.floor((floored % 3600) / 60);
  const secs = floored % 60;

  if (hours > 0) {
    return `${hours}:${String(minutes).padStart(2, "0")}:${String(secs).padStart(2, "0")}`;
  }

  return `${minutes}:${String(secs).padStart(2, "0")}`;
}

export function isChatTimestampEnabled(value) {
  return value === "true";
}
