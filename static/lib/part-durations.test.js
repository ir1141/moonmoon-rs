import { describe, expect, test } from "bun:test";
import { computeChatDelay } from "./part-durations.js";

describe("computeChatDelay", () => {
  test("returns the gap between the Twitch VOD and the summed YouTube parts", () => {
    // VOD 1466: 3 parts capped at 3h (10800) + remainder, Twitch VOD 28050s.
    const parts = [
      { id: "a", duration: 10800 },
      { id: "b", duration: 10800 },
      { id: "c", duration: 6279 },
    ];
    expect(computeChatDelay(28050, parts)).toBe(171);
  });

  test("is zero when the parts cover the whole VOD", () => {
    expect(computeChatDelay(3600, [{ id: "a", duration: 3600 }])).toBe(0);
  });

  test("never goes negative when parts overshoot the VOD duration", () => {
    expect(computeChatDelay(3600, [{ id: "a", duration: 4000 }])).toBe(0);
  });

  test("ignores parts with missing or non-positive durations", () => {
    const parts = [
      { id: "a", duration: 10800 },
      { id: "b", duration: null },
      { id: "c", duration: 6279 },
    ];
    expect(computeChatDelay(28050, parts)).toBe(28050 - 10800 - 6279);
  });

  test("returns 0 for missing total duration or bad input", () => {
    expect(computeChatDelay(0, [{ id: "a", duration: 100 }])).toBe(0);
    expect(computeChatDelay(100, null)).toBe(0);
  });
});
