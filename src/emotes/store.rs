use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

use tokio::sync::Mutex as AsyncMutex;

use crate::emotes::EmoteRecord;

/// What we cache for a failed lookup. The unit struct lets us store "miss"
/// without paying the size of a full record, and lets `lookup` return a
/// three-way state cleanly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedEntry {
    Hit(EmoteRecord),
    Miss,
}

/// One provider's search answer: `None` = provider unreachable,
/// `Some(None)` = answered with no match, `Some(Some(_))` = hit.
pub type ProviderSearchResult = Option<Option<EmoteRecord>>;

impl ResolvedEntry {
    /// Combine per-provider search results, given in provider-preference
    /// order. The first hit wins; at least one clean miss makes the whole
    /// search a cacheable [`ResolvedEntry::Miss`]; all-unreachable returns
    /// `None` so the next lookup retries instead of caching a transient
    /// failure.
    pub fn from_search_results(results: [ProviderSearchResult; 3]) -> Option<Self> {
        let any_answered = results.iter().any(Option::is_some);
        if let Some(hit) = results.into_iter().flatten().flatten().next() {
            return Some(ResolvedEntry::Hit(hit));
        }
        any_answered.then_some(ResolvedEntry::Miss)
    }
}

/// Soft cap on the resolved map. Exceeding it evicts a batch down to
/// `EVICT_TO` (we keep the prefetched map intact). 50k × ~200 bytes ≈ 10 MB.
pub const RESOLVED_CAP: usize = 50_000;

/// When the resolved map hits `RESOLVED_CAP`, evict down to this many entries
/// instead of clearing it entirely, so a full cache only forces ~10% of names
/// to re-resolve rather than all of them.
const EVICT_TO: usize = RESOLVED_CAP * 9 / 10;

pub struct EmoteIndex {
    /// Channel + global emotes from each of the three providers. Built once
    /// at boot, replaced atomically every 6h. Read path only — never mutated
    /// in place.
    pub prefetched: HashMap<String, EmoteRecord>,
    /// Search-fallback hits and misses. Mutated through the RwLock from the
    /// `/api/emotes/lookup/{name}` handler.
    resolved: RwLock<HashMap<String, ResolvedEntry>>,
    /// Per-name gate: only one upstream search runs at a time for a given
    /// name. Concurrent callers wait, then read the populated cache. The map
    /// is only ever locked briefly (never across an await), so a sync mutex is
    /// correct and lets the RAII `InFlightGuard` reclaim entries on drop.
    in_flight: Mutex<HashMap<String, Arc<AsyncMutex<()>>>>,
}

impl EmoteIndex {
    pub fn new(prefetched: HashMap<String, EmoteRecord>) -> Self {
        Self {
            prefetched,
            resolved: RwLock::new(HashMap::new()),
            in_flight: Mutex::new(HashMap::new()),
        }
    }

    /// Three-way lookup: prefetched hit, resolved hit, resolved miss, or
    /// unknown (caller must run a search).
    pub fn lookup(&self, name: &str) -> Lookup {
        if let Some(r) = self.prefetched.get(name) {
            return Lookup::Hit(r.clone());
        }
        if let Some(entry) = self.resolved.read().expect("not poisoned").get(name) {
            return match entry {
                ResolvedEntry::Hit(r) => Lookup::Hit(r.clone()),
                ResolvedEntry::Miss => Lookup::Miss,
            };
        }
        Lookup::Unknown
    }

    /// Record a resolved hit or miss. When the cap is exceeded, evicts a batch
    /// down to `EVICT_TO` instead of wiping everything, so a full cache only
    /// forces ~10% of names to re-resolve rather than all of them.
    pub fn record(&self, name: String, entry: ResolvedEntry) {
        let mut map = self.resolved.write().expect("not poisoned");
        if map.len() >= RESOLVED_CAP {
            let drop_count = map.len() - EVICT_TO;
            let victims: Vec<String> = map.keys().take(drop_count).cloned().collect();
            for k in &victims {
                map.remove(k);
            }
            tracing::info!(
                "emote resolved cache hit cap; evicted {} entries",
                victims.len()
            );
        }
        map.insert(name, entry);
    }

    /// Resolve a name through the cache, running `search` at most once across
    /// all concurrent callers for the same name (single-flight). `search`
    /// returns `Some(entry)` to cache a hit/miss, or `None` for a transient
    /// failure (reported as a miss to this caller but left uncached so a later
    /// caller retries).
    ///
    /// The per-name gate entry is reclaimed by an RAII `InFlightGuard`, so a
    /// caller whose future is dropped mid-search (e.g. the client disconnects
    /// during the upstream call) does not leak its `in_flight` entry.
    pub async fn lookup_or_resolve<F, Fut>(&self, name: &str, search: F) -> Lookup
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Option<ResolvedEntry>>,
    {
        match self.lookup(name) {
            Lookup::Unknown => {}
            resolved => return resolved,
        }

        let guard = {
            let mut in_flight = self.in_flight.lock().expect("not poisoned");
            let gate = Arc::clone(
                in_flight
                    .entry(name.to_string())
                    .or_insert_with(|| Arc::new(AsyncMutex::new(()))),
            );
            InFlightGuard {
                map: &self.in_flight,
                name,
                gate,
            }
        };
        let _hold = guard.gate.lock().await;

        // A prior holder may have populated the cache while we waited.
        match self.lookup(name) {
            Lookup::Unknown => {}
            resolved => return resolved,
        }

        let outcome = search().await;
        let result = match &outcome {
            Some(ResolvedEntry::Hit(record)) => Lookup::Hit(record.clone()),
            Some(ResolvedEntry::Miss) | None => Lookup::Miss,
        };
        if let Some(entry) = outcome {
            self.record(name.to_string(), entry);
        }
        result
    }

    #[cfg(test)]
    pub fn resolved_len(&self) -> usize {
        self.resolved.read().unwrap().len()
    }

    #[cfg(test)]
    pub fn in_flight_len(&self) -> usize {
        self.in_flight.lock().expect("not poisoned").len()
    }
}

/// RAII cleanup for a single-flight gate entry. On drop it removes the
/// per-name entry from `in_flight` once no other caller still holds the gate
/// (`strong_count == 2` -> only the map slot and this guard's clone remain).
/// Running cleanup in `Drop` makes it robust to cancellation: if the owning
/// future is dropped mid-search, the entry is still reclaimed. Removing an
/// entry a late waiter is about to reuse is harmless — that waiter just
/// re-creates the gate and re-checks the cache.
struct InFlightGuard<'a> {
    map: &'a Mutex<HashMap<String, Arc<AsyncMutex<()>>>>,
    name: &'a str,
    gate: Arc<AsyncMutex<()>>,
}

impl Drop for InFlightGuard<'_> {
    fn drop(&mut self) {
        let mut map = self.map.lock().expect("not poisoned");
        if Arc::strong_count(&self.gate) == 2 {
            map.remove(self.name);
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Lookup {
    Hit(EmoteRecord),
    Miss,
    Unknown,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emotes::EmoteProvider;

    fn rec(name: &str) -> EmoteRecord {
        EmoteRecord {
            url: format!("https://x/{name}"),
            provider: EmoteProvider::SevenTv,
            owner: None,
        }
    }

    #[test]
    fn from_search_results_first_hit_wins_in_provider_order() {
        let entry = ResolvedEntry::from_search_results([
            None,
            Some(Some(rec("bttv"))),
            Some(Some(rec("ffz"))),
        ]);
        assert_eq!(entry, Some(ResolvedEntry::Hit(rec("bttv"))));
    }

    #[test]
    fn from_search_results_hit_beats_clean_miss() {
        let entry = ResolvedEntry::from_search_results([Some(None), None, Some(Some(rec("ffz")))]);
        assert_eq!(entry, Some(ResolvedEntry::Hit(rec("ffz"))));
    }

    #[test]
    fn from_search_results_one_clean_miss_is_cacheable() {
        let entry = ResolvedEntry::from_search_results([None, Some(None), None]);
        assert_eq!(entry, Some(ResolvedEntry::Miss));
    }

    #[test]
    fn from_search_results_all_unreachable_is_transient() {
        assert_eq!(ResolvedEntry::from_search_results([None, None, None]), None);
    }

    #[test]
    fn prefetched_hit_wins_over_resolved() {
        let mut pre = HashMap::new();
        pre.insert("PogU".to_string(), rec("pre"));
        let idx = EmoteIndex::new(pre);
        idx.record("PogU".to_string(), ResolvedEntry::Hit(rec("res")));
        match idx.lookup("PogU") {
            Lookup::Hit(r) => assert_eq!(r.url, "https://x/pre"),
            other => panic!("expected prefetched hit, got {other:?}"),
        }
    }

    #[test]
    fn resolved_miss_short_circuits_subsequent_lookups() {
        let idx = EmoteIndex::new(HashMap::new());
        assert_eq!(idx.lookup("Nope"), Lookup::Unknown);
        idx.record("Nope".to_string(), ResolvedEntry::Miss);
        assert_eq!(idx.lookup("Nope"), Lookup::Miss);
    }

    #[test]
    fn resolved_evicts_batch_when_cap_exceeded() {
        let idx = EmoteIndex::new(HashMap::new());
        for i in 0..RESOLVED_CAP {
            idx.record(format!("e{i}"), ResolvedEntry::Miss);
        }
        assert_eq!(idx.resolved_len(), RESOLVED_CAP);
        // Exceeding the cap evicts a batch (not a full wipe), then inserts.
        idx.record("overflow".to_string(), ResolvedEntry::Miss);
        assert_eq!(idx.resolved_len(), EVICT_TO + 1);
        assert_eq!(idx.lookup("overflow"), Lookup::Miss);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_lookups_for_same_name_search_once() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let idx = Arc::new(EmoteIndex::new(HashMap::new()));
        let calls = Arc::new(AtomicUsize::new(0));

        let mut handles = Vec::new();
        for _ in 0..16 {
            let idx = Arc::clone(&idx);
            let calls = Arc::clone(&calls);
            handles.push(tokio::spawn(async move {
                idx.lookup_or_resolve("Greetings", || async move {
                    calls.fetch_add(1, Ordering::SeqCst);
                    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                    Some(ResolvedEntry::Hit(EmoteRecord {
                        url: "https://x/g".into(),
                        provider: EmoteProvider::Bttv,
                        owner: None,
                    }))
                })
                .await
            }));
        }

        for h in handles {
            assert!(matches!(h.await.unwrap(), Lookup::Hit(_)));
        }
        // Single-flight: all 16 callers shared one upstream search.
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        // And the result is cached for later callers.
        assert!(matches!(idx.lookup("Greetings"), Lookup::Hit(_)));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn lookups_for_different_names_do_not_block_each_other() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let idx = Arc::new(EmoteIndex::new(HashMap::new()));
        let calls = Arc::new(AtomicUsize::new(0));

        let mut handles = Vec::new();
        for i in 0..8 {
            let idx = Arc::clone(&idx);
            let calls = Arc::clone(&calls);
            handles.push(tokio::spawn(async move {
                idx.lookup_or_resolve(&format!("name{i}"), || async move {
                    calls.fetch_add(1, Ordering::SeqCst);
                    Some(ResolvedEntry::Miss)
                })
                .await
            }));
        }
        for h in handles {
            assert_eq!(h.await.unwrap(), Lookup::Miss);
        }
        // Distinct names each search once — the gate is per-name, not global.
        assert_eq!(calls.load(Ordering::SeqCst), 8);
    }

    #[tokio::test]
    async fn cancelled_lookup_does_not_leak_in_flight_entry() {
        let idx = Arc::new(EmoteIndex::new(HashMap::new()));
        // Drive the lookup until it suspends at the upstream search, then drop
        // the future by letting the timeout elapse (simulates a client
        // disconnecting mid-lookup).
        let cancelled = tokio::time::timeout(
            std::time::Duration::from_millis(20),
            idx.lookup_or_resolve("Cancelled", || async {
                tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                Some(ResolvedEntry::Miss)
            }),
        )
        .await;
        assert!(cancelled.is_err(), "the search should still be in flight");
        // The RAII guard reclaimed the gate entry on cancellation — no leak.
        assert_eq!(idx.in_flight_len(), 0);
        // Nothing was cached, so a later caller still resolves fresh.
        assert_eq!(idx.lookup("Cancelled"), Lookup::Unknown);
    }
}
