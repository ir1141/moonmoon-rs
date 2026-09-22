import { expect, test } from "bun:test";
import {
  chatFeedbackView,
  nextPlayerFallbackState,
  playerFallbackText,
} from "./player-feedback.js";

test("with no messages the chat body owns loading, empty, and error states", () => {
  for (const state of ["loading", "empty", "error"]) {
    expect(chatFeedbackView(state, 0)).toEqual({
      notice: state,
      headerText: "",
      headerRetry: false,
    });
  }
});

test("with messages on screen feedback stays compact in the chat header", () => {
  expect(chatFeedbackView("loading", 12)).toEqual({
    notice: null,
    headerText: "Loading chat...",
    headerRetry: false,
  });
  expect(chatFeedbackView("error", 12)).toEqual({
    notice: null,
    headerText: "Chat unavailable",
    headerRetry: true,
  });
});

test("ok state clears both the notice and the header", () => {
  expect(chatFeedbackView("ok", 0)).toEqual({
    notice: null,
    headerText: "",
    headerRetry: false,
  });
  expect(chatFeedbackView("ok", 12)).toEqual({
    notice: null,
    headerText: "",
    headerRetry: false,
  });
});

test("player fallback copy distinguishes missing videos and player failures", () => {
  expect(playerFallbackText("missing-video")).toContain(
    "No playable YouTube video",
  );
  expect(playerFallbackText("api-failed")).toContain("Player unavailable");
});

test("player fallback recovers when the YouTube player becomes ready late", () => {
  const failed = nextPlayerFallbackState(
    { shown: false, playerHidden: false, reason: null },
    { type: "show", reason: "api-failed" },
  );

  expect(failed).toEqual({
    shown: true,
    playerHidden: true,
    reason: "api-failed",
  });
  expect(nextPlayerFallbackState(failed, { type: "player-ready" })).toEqual({
    shown: false,
    playerHidden: false,
    reason: null,
  });
});
