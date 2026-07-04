export const CHAT_STRIPE_ALT_CLASS = "chat-msg--alt";

// Parity is a pure function of the render ordinal, not DOM position, so a row's
// stripe is fixed at creation and survives the top-pruning that would flicker a
// CSS :nth-child stripe.
export function chatStripeClass(ordinal) {
  return ordinal % 2 === 1 ? CHAT_STRIPE_ALT_CLASS : "";
}
