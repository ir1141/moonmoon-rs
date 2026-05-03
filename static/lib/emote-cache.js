// Persistent name → emote-record cache backed by the browser Cache API.
// Survives reloads and restarts, shares the origin's storage budget (much
// larger than localStorage's ~5MB), and is per-profile (not synced across
// devices — that's intentional: emotes are global state, every viewer gets
// the same answer, so re-fetching once per device is fine).
//
// Records: { hit: true, url, provider, owner? } | { hit: false }
// Negative entries are cached too so we never re-query a known miss.

const CACHE_NAME = "moonmoon-emote-cache-v2";
const OBSOLETE_CACHES = ["moonmoon-emote-cache-v1"];
const KEY_PREFIX = "https://emote-cache.moonmoon.local/";

if (
  typeof caches !== "undefined" &&
  caches &&
  typeof caches.delete === "function"
) {
  for (const old of OBSOLETE_CACHES) {
    caches.delete(old).catch(function () {});
  }
}

function keyFor(name) {
  return KEY_PREFIX + encodeURIComponent(name);
}

function cacheAvailable() {
  return (
    typeof caches !== "undefined" && caches && typeof caches.open === "function"
  );
}

export async function getCachedEmote(name) {
  if (!cacheAvailable()) return null;
  try {
    const cache = await caches.open(CACHE_NAME);
    const res = await cache.match(keyFor(name));
    if (!res) return null;
    return await res.json();
  } catch (err) {
    console.warn("[EmoteCache] read failed:", err);
    return null;
  }
}

export async function setCachedEmote(name, record) {
  if (!cacheAvailable()) return;
  try {
    const cache = await caches.open(CACHE_NAME);
    const body = JSON.stringify(record);
    await cache.put(
      keyFor(name),
      new Response(body, { headers: { "Content-Type": "application/json" } }),
    );
  } catch (err) {
    console.warn("[EmoteCache] write failed:", err);
  }
}
