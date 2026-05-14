import { describe, expect, test } from "bun:test";
import {
  formatChatTimestamp,
  isChatTimestampEnabled,
} from "../static/lib/chat-timestamps.js";

describe("formatChatTimestamp", () => {
  test.each([
    [0, "0:00"],
    [62.9, "1:02"],
    [3661, "1:01:01"],
    [-5, "0:00"],
  ])("formats %p seconds as %p", (seconds, expected) => {
    expect(formatChatTimestamp(seconds)).toBe(expected);
  });
});

describe("isChatTimestampEnabled", () => {
  test("is enabled only by the persisted true value", () => {
    expect(isChatTimestampEnabled("true")).toBe(true);
    expect(isChatTimestampEnabled("false")).toBe(false);
    expect(isChatTimestampEnabled(null)).toBe(false);
  });
});
