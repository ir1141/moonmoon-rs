import { describe, expect, test } from "bun:test";
import {
  buildContinueResumeUrl,
  selectContinueWatchingEntries,
} from "../static/lib/continue-watching.js";

describe("selectContinueWatchingEntries", () => {
  test("selects latest valid in-progress entries", () => {
    const result = selectContinueWatchingEntries(
      {
        a: { state: "in_progress", time: 11, updated: 100 },
        b: { state: "in_progress", time: 30, updated: 300 },
        c: { state: "in_progress", time: 25, updated: 200 },
      },
      { limit: 2 },
    );

    expect(result).toEqual([
      { id: "b", time: 30, updated: 300 },
      { id: "c", time: 25, updated: 200 },
    ]);
  });

  test("ignores watched, invalid and too-early entries", () => {
    const result = selectContinueWatchingEntries({
      empty: null,
      early: { state: "in_progress", time: 5, updated: 500 },
      stale: { state: "in_progress", time: 60 },
      watched: { state: "watched", updated: 900 },
      good: { state: "in_progress", time: 61.8, updated: 600 },
    });

    expect(result).toEqual([{ id: "good", time: 61, updated: 600 }]);
  });

  test("defaults to the single latest in-progress entry", () => {
    const result = selectContinueWatchingEntries({
      older: { state: "in_progress", time: 100, updated: 200 },
      newest: { state: "in_progress", time: 40, updated: 900 },
      middle: { state: "in_progress", time: 80, updated: 500 },
    });

    expect(result).toEqual([{ id: "newest", time: 40, updated: 900 }]);
  });

  test("handles non-object stores", () => {
    expect(selectContinueWatchingEntries(null)).toEqual([]);
    expect(selectContinueWatchingEntries("oops")).toEqual([]);
  });
});

describe("buildContinueResumeUrl", () => {
  test("builds a server-rendered resume hero URL for the latest entry", () => {
    const url = buildContinueResumeUrl({ id: "1430", time: 3724 });

    expect(url).toBe("/history/resume?id=1430&time=3724");
  });
});
