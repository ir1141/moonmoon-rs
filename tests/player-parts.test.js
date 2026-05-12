import { describe, test, expect } from "bun:test";
import {
  initialPartDurations,
  parseYoutubePartsDataset,
} from "../static/lib/player-parts.js";

describe("parseYoutubePartsDataset", () => {
  test("parses structured parts with durations", () => {
    const parts = parseYoutubePartsDataset(
      JSON.stringify([
        { id: "vod-1", duration: 10800 },
        { id: "vod-2", duration: 3594 },
      ]),
      "",
    );

    expect(parts).toEqual([
      { id: "vod-1", duration: 10800 },
      { id: "vod-2", duration: 3594 },
    ]);
  });

  test("falls back to legacy youtube ids", () => {
    const parts = parseYoutubePartsDataset("", JSON.stringify(["a", "b"]));

    expect(parts).toEqual([
      { id: "a", duration: null },
      { id: "b", duration: null },
    ]);
  });

  test("drops malformed entries", () => {
    const parts = parseYoutubePartsDataset(
      JSON.stringify([{ id: "ok", duration: 10 }, { duration: 20 }, null]),
      "",
    );

    expect(parts).toEqual([{ id: "ok", duration: 10 }]);
  });
});

describe("initialPartDurations", () => {
  test("uses API durations with max fallback for missing values", () => {
    expect(
      initialPartDurations(
        [
          { id: "a", duration: 100 },
          { id: "b", duration: null },
        ],
        null,
        10800,
      ),
    ).toEqual([100, 10800]);
  });

  test("lets matching cached durations override API durations", () => {
    expect(
      initialPartDurations(
        [
          { id: "a", duration: 100 },
          { id: "b", duration: 200 },
        ],
        [101, 202],
        10800,
      ),
    ).toEqual([101, 202]);
  });

  test("ignores cached durations with a mismatched shape", () => {
    expect(
      initialPartDurations([{ id: "a", duration: 100 }], [101, 202], 10800),
    ).toEqual([100]);
  });
});
