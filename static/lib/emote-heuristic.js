// Cheap gate that decides whether a word is worth looking up against the 7TV
// API. We skip obvious non-emotes (lowercase common English words, anything
// with punctuation, anything too short or too long) so we don't fire a fetch
// per word. Emote names are conventionally CamelCase, ALLCAPS, or mix in
// digits, so requiring at least one uppercase letter or digit catches the
// vast majority while rejecting normal prose.

export function isEmoteCandidate(word) {
  if (typeof word !== "string") return false;
  if (word.length < 2 || word.length > 25) return false;
  if (!/^[A-Za-z0-9_]+$/.test(word)) return false;
  if (!/[A-Z0-9]/.test(word)) return false;
  return true;
}
