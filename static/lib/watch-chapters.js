// Pure helpers for the watch-page chapter (game) selector. The server ships the
// chapter model as a JSON array on `#vod-data`'s `data-chapters` attribute, where
// each entry is `{ name, color, start }` and `start` is a global offset in seconds
// across the whole stream (the same timeline `seekToGlobal` walks). Durations and
// timestamp labels are derived here so the data attribute stays minimal.

/**
 * Parse the `data-chapters` payload into an ordered list of chapters. Tolerates a
 * missing/malformed attribute (returns []) and skips entries without a name or a
 * finite start so a single bad row can't break the selector.
 * @param {string} raw
 * @returns {{ name: string, color: number, start: number, idx: number }[]}
 */
export function parseChapters(raw) {
  if (!raw) return [];
  let parsed;
  try {
    parsed = JSON.parse(raw);
  } catch (e) {
    return [];
  }
  if (!Array.isArray(parsed)) return [];

  const out = [];
  for (const entry of parsed) {
    if (!entry || typeof entry.name !== "string" || entry.name.length === 0) {
      continue;
    }
    const start = Number(entry.start);
    if (!Number.isFinite(start)) continue;
    const color = Number(entry.color);
    out.push({
      name: entry.name,
      color: Number.isFinite(color) ? ((color % 8) + 8) % 8 : 0,
      start: Math.max(0, Math.round(start)),
    });
  }

  out.sort((a, b) => a.start - b.start);
  return out.map((c, idx) => ({ ...c, idx }));
}

/**
 * The index of the last chapter whose start is at or before `t` (the current
 * chapter). Assumes `chapters` is ordered ascending by start.
 * @param {{ start: number }[]} chapters
 * @param {number} t
 * @returns {number}
 */
export function currentChapterIdx(chapters, t) {
  let idx = 0;
  for (let i = 0; i < chapters.length; i++) {
    if (chapters[i].start <= t) idx = i;
    else break;
  }
  return idx;
}

/**
 * Length of chapter `idx` in seconds: distance to the next chapter's start, or to
 * the stream `total` for the final chapter. Never negative.
 * @param {{ start: number }[]} chapters
 * @param {number} idx
 * @param {number} total
 * @returns {number}
 */
export function chapterDurationSecs(chapters, idx, total) {
  if (idx < 0 || idx >= chapters.length) return 0;
  const end = idx < chapters.length - 1 ? chapters[idx + 1].start : total;
  return Math.max(0, end - chapters[idx].start);
}

/**
 * Coarse "Hh MMm" / "Mm" duration label for a chapter's meta line.
 * @param {number} seconds
 * @returns {string}
 */
export function formatChapterDuration(seconds) {
  seconds = Math.max(0, Math.floor(seconds));
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  if (h > 0) return h + "h " + String(m).padStart(2, "0") + "m";
  return m + "m";
}
