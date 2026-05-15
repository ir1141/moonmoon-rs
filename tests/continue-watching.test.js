import { describe, expect, test } from "bun:test";
import {
  buildContinueWatchingUrl,
  resumePercent,
  selectContinueWatchingEntries,
} from "../static/lib/continue-watching.js";

describe("selectContinueWatchingEntries", () => {
  test("selects latest valid resume entries", () => {
    const result = selectContinueWatchingEntries(
      {
        a: { time: 11, updated: 100 },
        b: { time: 30, updated: 300 },
        c: { time: 25, updated: 200 },
      },
      { limit: 2 },
    );

    expect(result).toEqual([
      { id: "b", time: 30, updated: 300 },
      { id: "c", time: 25, updated: 200 },
    ]);
  });

  test("ignores invalid and too-early entries", () => {
    const result = selectContinueWatchingEntries({
      empty: null,
      early: { time: 5, updated: 500 },
      stale: { time: 60 },
      good: { time: 61.8, updated: 600 },
    });

    expect(result).toEqual([{ id: "good", time: 61, updated: 600 }]);
  });

  test("handles non-object stores", () => {
    expect(selectContinueWatchingEntries(null)).toEqual([]);
    expect(selectContinueWatchingEntries("oops")).toEqual([]);
  });
});

describe("buildContinueWatchingUrl", () => {
  test("builds a history-grid URL for compact exact-resume cards", () => {
    const url = buildContinueWatchingUrl([
      { id: "1430", time: 3724 },
      { id: "2768249708", time: 98 },
    ]);

    expect(url).toBe(
      "/history/vods?ids=1430%2C2768249708&times=3724%2C98&sort=recent&resume_links=true&headers=false",
    );
  });
});

describe("resumePercent", () => {
  test("calculates and clamps progress", () => {
    expect(resumePercent(50, 100)).toBe(50);
    expect(resumePercent(150, 100)).toBe(100);
    expect(resumePercent(-10, 100)).toBe(0);
    expect(resumePercent(50, 0)).toBe(0);
  });
});
