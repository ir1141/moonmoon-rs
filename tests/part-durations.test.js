import { describe, test, expect } from "bun:test";
import {
  getCachedPartDurations,
  savePartDuration,
} from "../static/lib/part-durations.js";

describe("getCachedPartDurations", () => {
  test("returns null when store has no entry for vodId", () => {
    expect(getCachedPartDurations({}, "abc", 3)).toBeNull();
  });

  test("returns null when entry length does not match partCount", () => {
    const store = { abc: { durations: [10, 20], updated: 1 } };
    expect(getCachedPartDurations(store, "abc", 3)).toBeNull();
  });

  test("returns null when entry has no durations array", () => {
    const store = { abc: { updated: 1 } };
    expect(getCachedPartDurations(store, "abc", 3)).toBeNull();
  });

  test("returns the durations array on a valid hit", () => {
    const store = { abc: { durations: [10, 20, 30], updated: 1 } };
    expect(getCachedPartDurations(store, "abc", 3)).toEqual([10, 20, 30]);
  });
});

describe("savePartDuration", () => {
  test("initializes a fresh entry with zeros and writes the index", () => {
    const next = savePartDuration({}, "abc", 3, 1, 42, 500, 1000);
    expect(next.abc.durations).toEqual([0, 42, 0]);
    expect(next.abc.updated).toBe(1000);
  });

  test("preserves other indices when updating an existing entry", () => {
    const store = { abc: { durations: [10, 20, 30], updated: 1 } };
    const next = savePartDuration(store, "abc", 3, 1, 99, 500, 2000);
    expect(next.abc.durations).toEqual([10, 99, 30]);
    expect(next.abc.updated).toBe(2000);
  });

  test("reinitializes when stored length does not match partCount", () => {
    const store = { abc: { durations: [10, 20], updated: 1 } };
    const next = savePartDuration(store, "abc", 3, 0, 7, 500, 2000);
    expect(next.abc.durations).toEqual([7, 0, 0]);
  });

  test.each([
    ["duration is zero", [1, 0, 500, 2000]],
    ["duration is negative", [1, -5, 500, 2000]],
    ["index is negative", [-1, 50, 500, 2000]],
    ["index >= partCount", [3, 50, 500, 2000]],
  ])(
    "returns the input store unchanged when %s",
    (_label, [index, duration, max, now]) => {
      const store = { abc: { durations: [10, 20, 30], updated: 1 } };
      expect(savePartDuration(store, "abc", 3, index, duration, max, now)).toBe(
        store,
      );
    },
  );

  test("LRU-evicts the oldest entry when over the cap", () => {
    const store = {
      a: { durations: [1], updated: 100 },
      b: { durations: [1], updated: 200 },
      c: { durations: [1], updated: 300 },
    };
    const next = savePartDuration(store, "d", 1, 0, 9, 3, 400);
    expect(Object.keys(next).sort()).toEqual(["b", "c", "d"]);
    expect(next.a).toBeUndefined();
    expect(next.d.durations).toEqual([9]);
  });

  test("does not mutate the input store", () => {
    const store = { abc: { durations: [10, 20, 30], updated: 1 } };
    const snapshot = JSON.parse(JSON.stringify(store));
    savePartDuration(store, "abc", 3, 1, 99, 500, 2000);
    expect(store).toEqual(snapshot);
  });
});
