use crate::emotes::EmoteRecord;
use std::collections::HashMap;
use std::sync::RwLock;

/// What we cache for a failed lookup. The unit struct lets us store "miss"
/// without paying the size of a full record, and lets `lookup` return a
/// three-way state cleanly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedEntry {
    Hit(EmoteRecord),
    Miss,
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
}

impl EmoteIndex {
    pub fn new(prefetched: HashMap<String, EmoteRecord>) -> Self {
        Self {
            prefetched,
            resolved: RwLock::new(HashMap::new()),
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
            tracing::info!("emote resolved cache hit cap; evicted {} entries", victims.len());
        }
        map.insert(name, entry);
    }

    #[cfg(test)]
    pub fn resolved_len(&self) -> usize {
        self.resolved.read().unwrap().len()
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
}
