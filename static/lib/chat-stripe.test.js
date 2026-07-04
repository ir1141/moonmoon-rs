import { describe, expect, test } from "bun:test";
import { chatStripeClass, CHAT_STRIPE_ALT_CLASS } from "./chat-stripe.js";

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
