import { describe, expect, test } from "bun:test";
import {
  shouldFinalizePlaybackAtTick,
  shouldSaveResume,
} from "../static/lib/player-completion.js";

describe("shouldFinalizePlaybackAtTick", () => {
  test("only finalizes the final part near a known duration", () => {
    expect(
      shouldFinalizePlaybackAtTick({
        currentPart: 0,
        partCount: 2,
        duration: 100,
        currentTime: 99,
      }),
    ).toBe(false);

    expect(
      shouldFinalizePlaybackAtTick({
        currentPart: 1,
        partCount: 2,
        duration: 0,
        currentTime: 99,
      }),
    ).toBe(false);

    expect(
      shouldFinalizePlaybackAtTick({
        currentPart: 1,
        partCount: 2,
        duration: 100,
        currentTime: 97.9,
      }),
    ).toBe(false);

    expect(
      shouldFinalizePlaybackAtTick({
        currentPart: 1,
        partCount: 2,
        duration: 100,
        currentTime: 98,
      }),
    ).toBe(true);
  });
});

describe("shouldSaveResume", () => {
  test("skips resume persistence once the VOD has completed", () => {
    expect(shouldSaveResume({ completed: true })).toBe(false);
    expect(shouldSaveResume({ completed: false })).toBe(true);
  });
});
