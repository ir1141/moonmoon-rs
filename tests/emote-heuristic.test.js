import { describe, test, expect } from "bun:test";
import { isEmoteCandidate } from "../static/lib/emote-heuristic.js";

describe("isEmoteCandidate", () => {
  test.each(["TANIMURA", "Pog", "KEKW", "monkaS", "OMEGALUL", "5Head", "peepoHappy"])(
    "accepts emote-shaped word %p",
    (w) => {
      expect(isEmoteCandidate(w)).toBe(true);
    },
  );

  test.each(["the", "and", "lol", "hello", "yes", "no"])(
    "rejects all-lowercase word %p",
    (w) => {
      expect(isEmoteCandidate(w)).toBe(false);
    },
  );

  test.each(["", "a", "Z"])("rejects too-short word %p", (w) => {
    expect(isEmoteCandidate(w)).toBe(false);
  });

  test("rejects too-long word", () => {
    expect(isEmoteCandidate("A".repeat(26))).toBe(false);
  });

  test.each(["hi!", "what?", "@user", "https://x", "co-op", "it's", "foo.bar"])(
    "rejects word with punctuation %p",
    (w) => {
      expect(isEmoteCandidate(w)).toBe(false);
    },
  );

  test.each([null, undefined, 42, {}])("rejects non-string %p", (w) => {
    expect(isEmoteCandidate(w)).toBe(false);
  });
});
