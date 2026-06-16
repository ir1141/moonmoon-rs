// The persistent name → emote-record cache is no longer used: the server now
// owns the cache (see src/handlers/emotes.rs). This file remains only to
// delete the old caches from browsers that visited before the migration —
// otherwise they'd carry 30k+ stale entries forever (Cache API entries are
// not garbage-collected until the origin's storage budget is hit).

const OBSOLETE_CACHES = [
  "moonmoon-emote-cache-v1",
  "moonmoon-emote-cache-v2",
  "moonmoon-emote-cache-v3",
];

if (
  typeof caches !== "undefined" &&
  caches &&
  typeof caches.delete === "function"
) {
  for (const old of OBSOLETE_CACHES) {
    caches.delete(old).catch(function () {});
  }
}
