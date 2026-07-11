export function parseYoutubePartsDataset(partsJson, legacyIdsJson) {
  const structured = parseJsonArray(partsJson);
  if (structured) {
    return structured
      .map((part) => {
        if (!part || typeof part.id !== "string" || part.id.length === 0) {
          return null;
        }
        return {
          id: part.id,
          duration:
            typeof part.duration === "number" && part.duration > 0
              ? part.duration
              : null,
        };
      })
      .filter(Boolean);
  }

  const legacyIds = parseJsonArray(legacyIdsJson) || [];
  return legacyIds
    .filter((id) => typeof id === "string" && id.length > 0)
    .map((id) => ({ id, duration: null }));
}

export function initialPartDurations(parts, cachedDurations, maxPartDuration) {
  const durations = parts.map((part) =>
    typeof part.duration === "number" && part.duration > 0
      ? part.duration
      : maxPartDuration,
  );
  if (
    Array.isArray(cachedDurations) &&
    cachedDurations.length === parts.length
  ) {
    for (let i = 0; i < cachedDurations.length; i++) {
      if (cachedDurations[i] > 0) {
        durations[i] = cachedDurations[i];
      }
    }
  }
  return durations;
}

function parseJsonArray(value) {
  if (!value) return null;
  try {
    const parsed = JSON.parse(value);
    return Array.isArray(parsed) ? parsed : null;
  } catch (_e) {
    return null;
  }
}
