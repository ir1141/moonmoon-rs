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

  test.each(["JK", "SC", "UD"])(
    "rejects short uppercase initial token %p",
    (w) => {
      expect(isEmoteCandidate(w)).toBe(false);
    },
  );

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
    // chat reactions / slang
    "Wholesome", "Vibes", "VIBES", "Cap", "CAP",
    "Actually", "ACTUALLY", "Literally", "LITERALLY", "Probably",
    "Honestly", "Basically", "Obviously", "Definitely", "Maybe", "MAYBE",
    "Because", "Cuz", "CUZ", "Anyway", "Anyways",
    "First", "FIRST", "Last", "LAST", "Same", "SAME", "Sure", "SURE",
    "Welcome", "WELCOME", "Sorry", "SORRY",
    "POV", "Pov", "PFP", "Pfp", "DM", "OP", "IRL", "AFK", "RN", "NGL",
    // twitch-isms
    "Mod", "MOD", "Mods", "MODS", "Sub", "SUB", "Subs", "Bits", "BITS",
    "Drop", "Drops", "Live", "LIVE", "Twitch", "TWITCH",
    "YouTube", "YOUTUBE", "YT", "TTS", "Ban", "BAN", "Mute", "MUTE",
    "Spoiler", "SPOILER", "Spoilers", "Vods", "VODS", "Clip", "Clips",
    // data-driven from cache misses
    "ALREADY", "Already", "COP", "Cop", "Cops", "EM", "Em",
    "ETERNAL", "Eternal", "HOURLY", "Hourly", "KETO", "Keto",
    "LOOKS", "Looks", "Looking", "Marry", "MARRY", "Married",
    "SHERRY", "Sherry", "STARTED", "Started",
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

  // Real emotes that ARE common English words — must pass the heuristic
  // even though they look blocklist-shaped. Verified as real emotes on
  // multiple providers via search API.
  test.each(["BRUH", "Bruh", "BASED", "Based", "CRINGE", "Cringe",
             "BANGER", "Banger", "SUS", "Sus"])(
    "accepts emote-name English word %p",
    (w) => {
      expect(isEmoteCandidate(w)).toBe(true);
    },
  );

  // Pure-number tokens ("825", "2026") look candidate-shaped because
  // they pass the "has digit" check, but no emote is just digits.
  test.each(["825", "2026", "100", "1", "00"])(
    "rejects pure-digit token %p",
    (w) => {
      expect(isEmoteCandidate(w)).toBe(false);
    },
  );

  // Digit-prefix emotes like 5Head must still pass.
  test.each(["5Head", "4Head", "3Head"])(
    "accepts digit-prefix emote %p",
    (w) => {
      expect(isEmoteCandidate(w)).toBe(true);
    },
  );
});
