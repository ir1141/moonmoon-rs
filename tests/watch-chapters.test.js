import { describe, expect, test } from "bun:test";
import {
  chapterDurationSecs,
  currentChapterIdx,
  formatChapterDuration,
  parseChapters,
} from "../static/lib/watch-chapters.js";

describe("parseChapters", () => {
  test("parses a valid payload, ordering by start and assigning idx", () => {
    const raw = JSON.stringify([
      { name: "Bravo", color: 1, start: 3600 },
      { name: "Alpha", color: 0, start: 0 },
    ]);
    expect(parseChapters(raw)).toEqual([
      { name: "Alpha", color: 0, start: 0, idx: 0 },
      { name: "Bravo", color: 1, start: 3600, idx: 1 },
    ]);
  });

  test("returns [] for empty, malformed, or non-array input", () => {
    expect(parseChapters("")).toEqual([]);
    expect(parseChapters("{not json")).toEqual([]);
    expect(parseChapters(JSON.stringify({ name: "x" }))).toEqual([]);
  });

  test("skips entries missing a name or a finite start", () => {
    const raw = JSON.stringify([
      { name: "Keep", color: 2, start: 10 },
      { name: "", color: 3, start: 20 },
      { color: 4, start: 30 },
      { name: "NoStart", color: 5 },
    ]);
    expect(parseChapters(raw)).toEqual([
      { name: "Keep", color: 2, start: 10, idx: 0 },
    ]);
  });

  test("clamps start to >= 0 and normalizes color into 0..7", () => {
    const raw = JSON.stringify([{ name: "X", color: 9, start: -5 }]);
    expect(parseChapters(raw)).toEqual([
      { name: "X", color: 1, start: 0, idx: 0 },
    ]);
  });
});

describe("currentChapterIdx", () => {
  const chapters = [{ start: 0 }, { start: 100 }, { start: 250 }];

  test("returns the last chapter whose start is <= t", () => {
    expect(currentChapterIdx(chapters, 0)).toBe(0);
    expect(currentChapterIdx(chapters, 99)).toBe(0);
    expect(currentChapterIdx(chapters, 100)).toBe(1);
    expect(currentChapterIdx(chapters, 240)).toBe(1);
    expect(currentChapterIdx(chapters, 250)).toBe(2);
    expect(currentChapterIdx(chapters, 9999)).toBe(2);
  });

  test("clamps to 0 before the first chapter start", () => {
    expect(currentChapterIdx([{ start: 30 }, { start: 60 }], 10)).toBe(0);
  });
});

describe("chapterDurationSecs", () => {
  const chapters = [{ start: 0 }, { start: 100 }, { start: 250 }];

  test("uses the next chapter's start for non-final chapters", () => {
    expect(chapterDurationSecs(chapters, 0, 400)).toBe(100);
    expect(chapterDurationSecs(chapters, 1, 400)).toBe(150);
  });

  test("uses the stream total for the final chapter", () => {
    expect(chapterDurationSecs(chapters, 2, 400)).toBe(150);
  });

  test("never returns negative (total before the last start)", () => {
    expect(chapterDurationSecs(chapters, 2, 100)).toBe(0);
  });

  test("returns 0 for an out-of-range index", () => {
    expect(chapterDurationSecs(chapters, 5, 400)).toBe(0);
  });
});

describe("formatChapterDuration", () => {
  test("formats sub-hour durations as minutes", () => {
    expect(formatChapterDuration(0)).toBe("0m");
    expect(formatChapterDuration(59)).toBe("0m");
    expect(formatChapterDuration(600)).toBe("10m");
  });

  test("formats hour+ durations with zero-padded minutes", () => {
    expect(formatChapterDuration(3600)).toBe("1h 00m");
    expect(formatChapterDuration(6480)).toBe("1h 48m");
    expect(formatChapterDuration(27360)).toBe("7h 36m");
  });
});
