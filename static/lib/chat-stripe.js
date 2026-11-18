export const CHAT_STRIPE_ALT_CLASS = "chat-msg--alt";

// Parity is a pure function of the render ordinal, not DOM position, so a row's
// stripe is fixed at creation and survives the top-pruning that would flicker a
// CSS :nth-child stripe.
export function chatStripeClass(ordinal) {
  return ordinal % 2 === 1 ? CHAT_STRIPE_ALT_CLASS : "";
}

// Drops already-rendered messages from the front of the buffer. An even drop
// count keeps the next row's stripe continuous with the rows on screen, and
// the last rendered message stays so the prefetch check still sees progress.
export function dropRenderedChat(messages, index) {
  const drop = index - 1 - ((index - 1) % 2);
  if (drop <= 0) return { messages, index };
  return { messages: messages.slice(drop), index: index - drop };
}
