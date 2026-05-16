import { describe, expect, test } from "bun:test";
import {
  chatDistanceFromBottom,
  nextChatAutoScrollState,
} from "../static/lib/chat-autoscroll.js";

describe("chatDistanceFromBottom", () => {
  test("measures how far the chat viewport is from the bottom", () => {
    expect(
      chatDistanceFromBottom({
        scrollHeight: 500,
        scrollTop: 350,
        clientHeight: 100,
      }),
    ).toBe(50);
  });

  test("clamps browser rounding overscroll to zero", () => {
    expect(
      chatDistanceFromBottom({
        scrollHeight: 500,
        scrollTop: 401,
        clientHeight: 100,
      }),
    ).toBe(0);
  });
});

describe("nextChatAutoScrollState", () => {
  test("pauses only when a user scroll moves far enough from the bottom", () => {
    expect(
      nextChatAutoScrollState(
        { scrollHeight: 800, scrollTop: 550, clientHeight: 100 },
        { currentAutoScroll: true, userInitiated: true },
      ),
    ).toEqual({ autoScroll: false, paused: true });
  });

  test("keeps auto-scroll enabled for layout shifts while emotes load", () => {
    expect(
      nextChatAutoScrollState(
        { scrollHeight: 800, scrollTop: 550, clientHeight: 100 },
        { currentAutoScroll: true, userInitiated: false },
      ),
    ).toEqual({ autoScroll: true, paused: false });
  });

  test("does not resume a user-paused chat from non-user scroll events", () => {
    expect(
      nextChatAutoScrollState(
        { scrollHeight: 800, scrollTop: 700, clientHeight: 100 },
        { currentAutoScroll: false, userInitiated: false },
      ),
    ).toEqual({ autoScroll: false, paused: true });
  });
});
