import { describe, expect, test } from "bun:test";

import {
  datePresetForRange,
  rangeForDatePreset,
} from "../static/lib/list-filters.js";

describe("list filter date presets", () => {
  test("30 and 90 day presets clamp to archive bounds", () => {
    expect(rangeForDatePreset("30", "2026-06-07", "2026-05-20", "2026-06-01")).toEqual({
      from: "2026-05-20",
      to: "2026-06-01",
    });
    expect(rangeForDatePreset("90", "2026-06-07", "2026-01-01", "2026-06-01")).toEqual({
      from: "2026-03-03",
      to: "2026-06-01",
    });
  });

  test("server-rendered ranges can identify active presets", () => {
    expect(datePresetForRange("", "", "2026-06-07", "2019-01-01", "2026-06-07")).toBe(
      "all",
    );
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
