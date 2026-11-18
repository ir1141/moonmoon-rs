import { describe, expect, test } from "bun:test";
import {
  chatStripeClass,
  CHAT_STRIPE_ALT_CLASS,
  dropRenderedChat,
} from "./chat-stripe.js";

describe("chatStripeClass", () => {
  test("shades every other message, leaving the first unshaded", () => {
    const classes = [0, 1, 2, 3, 4, 5].map(chatStripeClass);
    expect(classes).toEqual([
      "",
      CHAT_STRIPE_ALT_CLASS,
      "",
      CHAT_STRIPE_ALT_CLASS,
      "",
      CHAT_STRIPE_ALT_CLASS,
    ]);
  });
});

describe("dropRenderedChat", () => {
  const messages = ["m0", "m1", "m2", "m3", "m4", "m5", "m6", "m7", "m8", "m9"];

  for (const rendered of [2, 7, 8, 10]) {
    test(`after ${rendered} rendered, unrendered messages keep their stripes`, () => {
      const next = dropRenderedChat(messages, rendered);
      expect(next.messages.slice(next.index)).toEqual(messages.slice(rendered));
      for (let i = next.index; i < next.messages.length; i++) {
        const original = messages.indexOf(next.messages[i]);
        expect(chatStripeClass(i)).toBe(chatStripeClass(original));
      }
      expect(next.messages[next.index - 1]).toBe(messages[rendered - 1]);
      expect(next.index).toBeLessThanOrEqual(2);
    });
  }

  test("nothing rendered keeps the buffer as is", () => {
    expect(dropRenderedChat(messages, 0)).toEqual({ messages, index: 0 });
  });
});
