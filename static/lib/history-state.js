function storageGet(storage, key) {
  if (!storage) return null;
  if (typeof storage.getItem === "function") return storage.getItem(key);
  if (typeof storage.get === "function") return storage.get(key);
  return null;
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

function normalizeResumeEntry(entry) {
  if (!entry || typeof entry !== "object" || Array.isArray(entry)) return null;
  const updated = normalizeUpdated(entry.updated);
  if (updated === null) return null;

  const time = Number(entry.time);
  return {
    state: "in_progress",
    time: Number.isFinite(time) && time >= 0 ? Math.floor(time) : 0,
    updated,
  };
}

function normalizeWatchedEntry(entry) {
  if (entry === true) return { state: "watched", updated: 0 };
  if (typeof entry === "number") {
    const updated = normalizeUpdated(entry);
    return updated === null ? null : { state: "watched", updated };
  }
  if (!entry || typeof entry !== "object" || Array.isArray(entry)) return null;

  const updated = normalizeUpdated(entry.updated || 0);
  return updated === null ? null : { state: "watched", updated };
}

export function buildHistoryEntries(resumeStore, watchedStore) {
  const entriesById = new Map();

  if (resumeStore && typeof resumeStore === "object" && !Array.isArray(resumeStore)) {
    for (const [id, entry] of Object.entries(resumeStore)) {
      if (!id) continue;
      const normalized = normalizeResumeEntry(entry);
      if (normalized) entriesById.set(id, { id, ...normalized });
    }
  }

  if (watchedStore && typeof watchedStore === "object" && !Array.isArray(watchedStore)) {
    for (const [id, entry] of Object.entries(watchedStore)) {
      if (!id) continue;
      const normalized = normalizeWatchedEntry(entry);
      if (!normalized) continue;

      const existing = entriesById.get(id);
      if (!existing || normalized.updated > existing.updated) {
        entriesById.set(id, { id, ...normalized });
      }
    }
  }

  return Array.from(entriesById.values()).sort((a, b) =>
    b.updated === a.updated ? a.id.localeCompare(b.id) : b.updated - a.updated,
  );
}

export function serializeHistoryRequest(entries, sort = "recent") {
  const params = new URLSearchParams({
    ids: entries.map((entry) => entry.id).join(","),
    times: entries
      .map((entry) =>
        entry.state === "in_progress" && Number.isFinite(entry.time)
          ? String(Math.floor(entry.time))
          : "",
      )
      .join(","),
    states: entries.map((entry) => entry.state).join(","),
    sort: sort === "game" ? "game" : "recent",
  });

  return params;
}
