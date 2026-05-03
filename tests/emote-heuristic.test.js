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

  // Common English words that pass the shape rules (sentence-start
  // capitalization or ALLCAPS emphasis) but should never be looked up
  // as emotes — otherwise we get false positives like "GIVE" matching
  // some random FFZ emote.
  test.each([
    "The", "THE", "A", "An", "AN", "And", "AND", "Or", "OR", "But", "BUT",
    "I", "You", "YOU", "He", "She", "We", "WE", "They", "It", "IT",
    "This", "THIS", "That", "THAT", "These", "Those",
    "Is", "IS", "Are", "ARE", "Was", "Were", "Be", "BE", "Been",
    "Have", "HAS", "Had", "Do", "DO", "Does", "Did", "DID",
    "Can", "CAN", "Will", "WILL", "Would", "Should", "Could", "May", "Must",
    "Not", "NOT", "No", "NO", "Yes", "YES", "Yeah", "YEAH",
    "Now", "NOW", "Then", "THEN", "Here", "HERE", "There", "THERE",
    "What", "WHAT", "Why", "WHY", "How", "HOW", "When", "WHEN", "Where", "WHO", "Who",
    "Get", "GET", "Got", "Give", "GIVE", "Take", "TAKE", "Make", "MAKE",
    "Go", "GO", "Goes", "Went", "Come", "COME", "Came",
    "See", "SEE", "Saw", "Look", "LOOK", "Find", "Want", "WANT",
    "Stop", "STOP", "Start", "Run", "RUN", "Move", "MOVE", "Hit", "HIT",
    "Win", "WIN", "Lose", "LOSE", "Kill", "KILL", "Help", "HELP",
    "Please", "PLEASE", "Thanks", "THANKS", "Hello", "HELLO", "Hi", "HI",
    "Ok", "OK", "Okay", "OKAY", "Wow", "WOW", "Lol", "LOL", "Wtf", "WTF",
    "Omg", "OMG", "Gg", "GG", "Ez", "EZ", "Nice", "NICE", "Cool", "COOL",
    "Good", "GOOD", "Bad", "BAD", "Like", "LIKE", "Just", "JUST",
    "Im", "IM", "Its", "ITS", "Dont", "DONT", "Cant", "CANT",
    "All", "ALL", "Some", "SOME", "Many", "More", "MORE", "Most",
    "Up", "UP", "Down", "DOWN", "Out", "OUT", "In", "IN", "On", "ON",
    "Hype", "HYPE", "Lets", "LETS", "Let", "LET", "Plz", "PLZ", "Pls", "PLS",
    "Moon", "MOON", "Moonmoon", "MOONMOON",
    // swears / expletives — same shape-rule problem (ALLCAPS emphasis
    // and capitalized exclamations dominate when chat is mad)
    "Fuck", "FUCK", "Fucking", "FUCKING", "Fucked", "FUCKED", "Fck", "FCK", "Fkn", "FKN",
    "Shit", "SHIT", "Shitty", "Bullshit", "BS",
    "Damn", "DAMN", "Goddamn", "GODDAMN",
    "Ass", "ASS", "Asshole", "ASSHOLE",
    "Bitch", "BITCH",
    "Crap", "CRAP",
    "Hell", "HELL",
    "Dick", "DICK", "Cock", "COCK",
    "Cunt", "CUNT", "Twat", "TWAT", "Prick", "PRICK",
    "Wank", "Wanker", "Bollocks", "BOLLOCKS",
    "Mf", "MF", "Mfer", "Motherfucker", "MOTHERFUCKER",
    "Whore", "WHORE", "Slut", "SLUT",
    "Bastard", "BASTARD",
    "Piss", "PISS", "Pissed", "PISSED",
  ])("rejects common English word %p (case-insensitive blocklist)", (w) => {
    expect(isEmoteCandidate(w)).toBe(false);
  });

  // Real emotes that happen to be English words but are spelled in
  // unusual casing — these should still pass.
  test.each(["Pog", "PogU", "KEKW", "LULW", "JAMM", "Sadge", "Madge"])(
    "accepts real emote despite English-word shape %p",
    (w) => {
      expect(isEmoteCandidate(w)).toBe(true);
    },
  );
});
