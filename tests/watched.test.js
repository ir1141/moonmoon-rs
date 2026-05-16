import { describe, expect, test } from "bun:test";
import {
  hasWatchedVod,
  markWatchedVod,
  mergeWatched,
  watchedVodIds,
} from "../static/lib/watched.js";

describe("watched VOD helpers", () => {
  test("recognizes current and legacy watched entries", () => {
    expect(
      watchedVodIds({ a: { updated: 100 }, b: true, c: 42, d: false }),
    ).toEqual(["a", "b", "c"]);
    expect(hasWatchedVod({ a: { updated: 100 } }, "a")).toBe(true);
    expect(hasWatchedVod({ a: { updated: 100 } }, "missing")).toBe(false);
  });

  test("marks a VOD watched without mutating the existing store", () => {
    const store = { old: { updated: 1 } };
    const next = markWatchedVod(store, "new", 200);

    expect(next).toEqual({ old: { updated: 1 }, new: { updated: 200 } });
    expect(store).toEqual({ old: { updated: 1 } });
  });

  test("prunes oldest watched entries when capped", () => {
    const next = markWatchedVod(
      {
        old: { updated: 1 },
        middle: { updated: 2 },
      },
      "new",
      3,
      2,
    );

    expect(next).toEqual({ middle: { updated: 2 }, new: { updated: 3 } });
  });

  test("merges remote watched entries by newest timestamp", () => {
    const result = mergeWatched(
      {
        a: { updated: 10 },
        b: { updated: 30 },
      },
      {
        a: { updated: 20 },
        c: true,
      },
    );

    expect(result.changed).toBe(true);
    expect(result.merged).toEqual({
      a: { updated: 20 },
      b: { updated: 30 },
      c: { updated: 0 },
    });
  });
});
