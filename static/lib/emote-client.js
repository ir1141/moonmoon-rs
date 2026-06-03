// Server-backed emote lookup. Replaces direct calls to 7TV/BTTV/FFZ — the
// Rust server is now the cache. See docs/superpowers/plans/2026-06-03-
// server-side-emote-cache.md for the architecture.

export async function fetchChannelEmotes() {
  try {
    const res = await fetch("/api/emotes/channel");
    if (!res.ok) return {};
    const body = await res.json();
    return body.emotes || {};
  } catch (err) {
    console.warn("[Emote] channel fetch failed:", err);
    return {};
  }
}

export async function lookupEmote(name) {
  try {
    const res = await fetch("/api/emotes/lookup/" + encodeURIComponent(name));
    if (!res.ok) return { hit: false, transient: true };
    return await res.json();
  } catch (err) {
    console.warn("[Emote] lookup failed for", name, err);
    return { hit: false, transient: true };
  }
}
