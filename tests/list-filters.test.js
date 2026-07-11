import { describe, expect, test } from "bun:test";

import {
  datePresetForRange,
  nextSortIndex,
  rangeForDatePreset,
  typeaheadIndex,
} from "../static/lib/list-filters.js";

describe("list filter date presets", () => {
  test("30 and 90 day presets clamp to archive bounds", () => {
    expect(
      rangeForDatePreset("30", "2026-06-07", "2026-05-20", "2026-06-01"),
    ).toEqual({
      from: "2026-05-20",
      to: "2026-06-01",
    });
    expect(
      rangeForDatePreset("90", "2026-06-07", "2026-01-01", "2026-06-01"),
    ).toEqual({
      from: "2026-03-03",
      to: "2026-06-01",
    });
  });

  test("server-rendered ranges can identify active presets", () => {
    expect(
      datePresetForRange("", "", "2026-06-07", "2019-01-01", "2026-06-07"),
    ).toBe("all");
    expect(
      datePresetForRange(
        "2026-03-09",
        "2026-06-07",
        "2026-06-07",
        "2019-01-01",
        "2026-06-07",
      ),
    ).toBe("90");
    expect(
      datePresetForRange(
        "2026-05-01",
        "2026-05-31",
        "2026-06-07",
        "2019-01-01",
        "2026-06-07",
      ),
    ).toBe("custom");
  });
});

describe("sort listbox keyboard navigation", () => {
  test("arrow keys step and wrap around the menu", () => {
    expect(nextSortIndex("ArrowDown", 0, 4)).toBe(1);
    expect(nextSortIndex("ArrowDown", 3, 4)).toBe(0);
    expect(nextSortIndex("ArrowUp", 1, 4)).toBe(0);
    expect(nextSortIndex("ArrowUp", 0, 4)).toBe(3);
  });

  test("with no option focused, each arrow enters from its own end", () => {
    expect(nextSortIndex("ArrowDown", -1, 4)).toBe(0);
    expect(nextSortIndex("ArrowUp", -1, 4)).toBe(3);
  });

  test("Home and End jump to the edges", () => {
    expect(nextSortIndex("Home", 2, 4)).toBe(0);
    expect(nextSortIndex("End", 2, 4)).toBe(3);
  });

  test("keys the listbox does not own return null", () => {
    expect(nextSortIndex("Tab", 1, 4)).toBeNull();
    expect(nextSortIndex("a", 1, 4)).toBeNull();
    expect(nextSortIndex("ArrowDown", 0, 0)).toBeNull();
  });
});

describe("sort listbox typeahead", () => {
  const labels = ["Newest First", "Oldest First", "Longest", "Shortest"];

  test("a single character moves to the next match", () => {
    expect(typeaheadIndex(labels, "o", -1)).toBe(1);
    expect(typeaheadIndex(labels, "s", 1)).toBe(3);
  });

  test("repeating one character cycles through the matches", () => {
    expect(typeaheadIndex(["Alpha", "Beta", "Anchor"], "a", 0)).toBe(2);
    expect(typeaheadIndex(["Alpha", "Beta", "Anchor"], "a", 2)).toBe(0);
  });

  test("a longer buffer keeps the option it already matched", () => {
    expect(typeaheadIndex(labels, "lo", 2)).toBe(2);
    expect(typeaheadIndex(labels, "ol", 2)).toBe(1);
  });

  test("matching is case insensitive", () => {
    expect(typeaheadIndex(labels, "NEW", -1)).toBe(0);
  });

  test("no match, empty query, and empty menu return null", () => {
    expect(typeaheadIndex(labels, "z", 0)).toBeNull();
    expect(typeaheadIndex(labels, "", 0)).toBeNull();
    expect(typeaheadIndex([], "a", -1)).toBeNull();
  });
});
