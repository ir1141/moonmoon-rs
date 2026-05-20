import { expect, test } from "bun:test";
import {
  chatEmptyStatusText,
  chatErrorStatusText,
  chatLoadStatusText,
  playerFallbackText,
} from "./player-feedback.js";

test("chat status announces loading before requesting chat", () => {
  expect(chatLoadStatusText()).toBe("Loading chat...");
});

test("empty chat result explains the current timestamp has no messages", () => {
  expect(chatEmptyStatusText()).toBe("No chat at this timestamp");
});

test("chat failure exposes unavailable copy for the retry state", () => {
  expect(chatErrorStatusText()).toBe("Chat unavailable");
});

test("player fallback copy distinguishes missing videos and player failures", () => {
  expect(playerFallbackText("missing-video")).toContain(
    "No playable YouTube video",
  );
  expect(playerFallbackText("api-failed")).toContain("Player unavailable");
});
