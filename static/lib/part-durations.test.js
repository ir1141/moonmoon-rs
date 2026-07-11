import { describe, expect, test } from "bun:test";
import { computeChatDelay } from "./part-durations.js";

const MAX_PART_DURATION = 10800; // 3 hours

describe("computeChatDelay", () => {
  test("returns the gap between the Twitch VOD and the summed YouTube parts", () => {
    // VOD 1466: 3 parts capped at 3h (10800) + remainder, Twitch VOD 28050s.
    const parts = [
      { id: "a", duration: 10800 },
      { id: "b", duration: 10800 },
      { id: "c", duration: 6279 },
    ];
    expect(computeChatDelay(28050, parts, MAX_PART_DURATION)).toBe(171);
  });

  test("is zero when the parts cover the whole VOD", () => {
    expect(
      computeChatDelay(3600, [{ id: "a", duration: 3600 }], MAX_PART_DURATION),
    ).toBe(0);
  });

  test("never goes negative when parts overshoot the VOD duration", () => {
    expect(
      computeChatDelay(3600, [{ id: "a", duration: 4000 }], MAX_PART_DURATION),
    ).toBe(0);
  });

  test("estimates unknown part durations at the 3h cap, matching the playback timeline", () => {
    // The middle part's duration is unknown server-side, so it falls back to the
    // 3h cap — the same value getGlobalTime walks for it. The delay stays bounded
    // (171s) instead of being inflated by the whole unknown part.
    const parts = [
      { id: "a", duration: 10800 },
      { id: "b", duration: null },
      { id: "c", duration: 6279 },
    ];
    expect(computeChatDelay(28050, parts, MAX_PART_DURATION)).toBe(171);
  });

  test("falls back to no delay when a single part's duration is unknown", () => {
    // The dangerous case: one part, duration unknown. Estimating it at the cap
    // makes the gap clamp to 0 (chat aligned to player time) rather than the old
    // behavior of treating it as 0s and delaying chat by the whole VOD length.
    expect(
      computeChatDelay(7000, [{ id: "a", duration: null }], MAX_PART_DURATION),
    ).toBe(0);
  });

  test("accepts a resolved numeric durations array (the player's recompute path)", () => {
    // onPlayerReady recomputes the delay from partDurations (plain numbers)
    // after refining it with cached real durations.
    expect(
      computeChatDelay(28050, [10800, 10800, 6279], MAX_PART_DURATION),
    ).toBe(171);
    expect(computeChatDelay(7000, [0], MAX_PART_DURATION)).toBe(0);
  });

  test("returns 0 for missing total duration or bad input", () => {
    expect(
      computeChatDelay(0, [{ id: "a", duration: 100 }], MAX_PART_DURATION),
    ).toBe(0);
    expect(computeChatDelay(100, null, MAX_PART_DURATION)).toBe(0);
  });
});
