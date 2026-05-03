// Cheap gate that decides whether a word is worth looking up against a
// third-party emote API. We skip obvious non-emotes so we don't fire a
// fetch per word: too short / too long, anything with punctuation, and
// anything all-lowercase (real emote names almost always carry an
// uppercase letter or digit).
//
// We also reject a small case-insensitive blocklist of common English
// words. The shape rules above let through sentence-start capitalization
// ("The", "You") and ALLCAPS emphasis ("GIVE", "STOP", "MOON"); with
// three providers in the lookup race, those words frequently false-
// positive against some random foreign-channel emote. The blocklist is
// surgical — it only filters words that would otherwise pass the shape
// rules, so real emote names like "Pog", "KEKW", "PogU" are unaffected.

const COMMON_WORDS = new Set([
  // articles / pronouns / demonstratives
  "a", "an", "the",
  "i", "me", "my", "mine", "myself",
  "you", "your", "yours", "yourself",
  "he", "him", "his", "himself",
  "she", "her", "hers", "herself",
  "it", "its", "itself",
  "we", "us", "our", "ours", "ourselves",
  "they", "them", "their", "theirs", "themselves",
  "this", "that", "these", "those",
  // conjunctions / prepositions
  "and", "or", "but", "nor", "for", "yet", "so", "if",
  "of", "at", "by", "in", "on", "to", "up", "as", "from", "with",
  "into", "onto", "out", "down", "off", "over", "under",
  // be / aux verbs
  "is", "am", "are", "was", "were", "be", "been", "being",
  "have", "has", "had", "having",
  "do", "does", "did", "done", "doing",
  // modals
  "can", "could", "will", "would", "shall", "should", "may", "might", "must",
  // negations / affirmations
  "no", "not", "none", "never", "nothing",
  "yes", "yeah", "yep", "yup", "nope",
  // common verbs (chat-frequent)
  "get", "got", "give", "gave", "take", "took", "make", "made",
  "go", "goes", "went", "gone", "come", "came",
  "see", "saw", "seen", "look", "find", "found",
  "think", "thought", "know", "knew", "say", "said", "tell", "told",
  "want", "ask", "use", "try", "call", "work",
  "run", "ran", "sit", "sat", "stop", "start",
  "move", "hit", "win", "won", "lose", "lost", "kill", "die", "died",
  "help", "play", "hear", "heard", "feel", "felt",
  "keep", "kept", "leave", "left", "send", "sent",
  "read", "write", "wrote", "hold", "held",
  "bring", "brought", "buy", "bought", "build", "built",
  "live", "lived", "love", "loved", "hate", "hated",
  "let", "lets",
  // common adverbs / adjectives
  "now", "then", "here", "there", "soon", "later", "today", "yesterday", "tomorrow",
  "what", "why", "how", "when", "where", "who", "which", "whose",
  "all", "any", "some", "every", "many", "much", "more", "most", "less", "few", "fewer",
  "very", "too", "also", "only", "just", "even", "still", "well",
  "good", "great", "best", "better", "bad", "worse", "worst",
  "big", "small", "new", "old", "hot", "cold", "fast", "slow",
  "true", "false", "real", "fake", "right", "wrong",
  "nice", "cool", "fine", "fun", "funny",
  "like",
  // greetings / common interjections
  "hi", "hello", "hey", "bye",
  "ok", "okay", "wow", "yo", "yay", "huh", "haha",
  "thanks", "ty", "please", "plz", "pls", "sorry",
  // chat slang
  "lol", "lmao", "lmfao", "rofl", "wtf", "omg", "ffs", "imo", "imho", "tbh",
  "idk", "idc", "brb", "tldr", "tl",
  "gg", "ez", "ezpz", "hf", "gl", "rip", "smh", "fyi", "til",
  "btw", "atm", "asap", "tbf", "thx", "np",
  "hype", "moon", "moonmoon",
  // contractions stripped of apostrophe — note that "were", "well",
  // "lets" are already covered above as the verb / adverb forms
  // (same lowercased token), so they aren't repeated here
  "im", "ive", "id", "ill",
  "youre", "youve", "youll", "youd",
  "hes", "shes", "theyre", "theyve", "theyll", "theyd",
  "weve", "wed",
  "isnt", "arent", "wasnt", "werent",
  "dont", "doesnt", "didnt",
  "cant", "couldnt", "wont", "wouldnt", "shouldnt",
  "hasnt", "havent", "hadnt",
  // common nouns (chat-frequent)
  "guy", "guys", "man", "men", "woman", "women", "boy", "girl", "kid", "kids",
  "dude", "bro", "sis", "mom", "dad",
  "day", "time", "year", "week", "month", "hour", "min", "sec",
  "way", "thing", "things", "stuff",
  "game", "stream", "chat", "vod",
  // numerals as words
  "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten",
  // chat reactions / slang
  "wholesome", "vibes", "vibe", "vibing", "cap", "nocap", "fr",
  "actually", "literally", "probably", "honestly", "basically",
  "obviously", "definitely", "maybe", "because", "cuz", "cause",
  "anyway", "anyways", "first", "last", "same", "sure",
  "welcome", "almost", "kinda", "sorta",
  "pov", "pfp", "dm", "dms", "op", "irl", "afk", "rn", "ngl",
  // twitch-isms
  "mod", "mods", "sub", "subs", "subbed", "bits", "drop", "drops",
  "twitch", "youtube", "yt", "tts", "ban", "banned", "mute",
  "muted", "spoiler", "spoilers", "vods", "clip", "clips",
  // common english words that slipped through real chat (data-driven)
  "already", "cop", "cops", "em", "eternal", "hourly", "keto",
  "looks", "looking", "marry", "married", "sherry", "started", "starting",
  // swears / expletives (same shape-rule false-positive problem — chat
  // dumps these in ALLCAPS or sentence-start case all the time)
  "fuck", "fucking", "fucked", "fucker", "fuckers", "fucks",
  "fck", "fkn", "fuk", "fking", "fkin",
  "shit", "shitty", "shits", "bullshit", "bs",
  "damn", "damned", "goddamn", "goddamned", "dammit", "damnit",
  "ass", "asses", "asshole", "assholes", "arse", "arsehole",
  "bitch", "bitches", "biatch",
  "crap", "crappy",
  "hell",
  "dick", "dicks", "cock", "cocks",
  "cunt", "cunts", "twat", "twats", "prick", "pricks",
  "wank", "wanker", "wankers", "bollocks",
  "mf", "mfs", "mfer", "mfers", "motherfucker", "motherfuckers",
  "whore", "whores", "slut", "sluts", "thot", "thots",
  "bastard", "bastards",
  "piss", "pissed", "pissing",
]);

export function isEmoteCandidate(word) {
  if (typeof word !== "string") return false;
  if (word.length < 2 || word.length > 25) return false;
  if (!/^[A-Za-z0-9_]+$/.test(word)) return false;
  if (!/[A-Z0-9]/.test(word)) return false;
  if (/^\d+$/.test(word)) return false; // pure numbers ("825", "2026")
  if (COMMON_WORDS.has(word.toLowerCase())) return false;
  return true;
}
