import { describe, expect, test } from "bun:test";
import {
  shouldFinalizePlaybackAtTick,
  shouldSaveOnUnload,
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

describe("shouldSaveOnUnload", () => {
  test("never saves a restored position the session did not play", () => {
    // The mobile-clobber regression: a tab restored to an old position and
    // hard-reloaded must not re-stamp that position as the newest write,
    // or last-write-wins erases every other device's progress.
    expect(
      shouldSaveOnUnload({ lastPlaybackSavedTime: null, currentTime: 2520 }),
    ).toBe(false);
  });

  test("skips the re-stamp when parked exactly at the last played save", () => {
    expect(
      shouldSaveOnUnload({ lastPlaybackSavedTime: 2520, currentTime: 2520 }),
    ).toBe(false);
    expect(
      shouldSaveOnUnload({
        lastPlaybackSavedTime: 2520.4,
        currentTime: 2520.9,
      }),
    ).toBe(false);
  });

  test("captures playback or seeks that moved past the last save", () => {
    expect(
      shouldSaveOnUnload({ lastPlaybackSavedTime: 2520, currentTime: 2521 }),
    ).toBe(true);
    expect(
      shouldSaveOnUnload({ lastPlaybackSavedTime: 2521, currentTime: 2520 }),
    ).toBe(true);
  });

  test("rejects unusable current positions", () => {
    expect(
      shouldSaveOnUnload({ lastPlaybackSavedTime: 2520, currentTime: NaN }),
    ).toBe(false);
    expect(shouldSaveOnUnload({})).toBe(false);
  });
});
