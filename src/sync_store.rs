use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

/// Hard cap for incoming sync blobs. The full payload (including JSON
/// envelope) must fit. 256 KiB is ~150x the size of a fully-populated
/// 500-entry resume map, so this is generous but still bounded.
pub const MAX_BLOB_BYTES: usize = 256 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SyncBlob {
    /// Opaque blob the server never inspects — currently `{ "resume": {...} }`.
    pub blob: serde_json::Value,
    /// Client-supplied milliseconds since epoch. Used for last-write-wins
    /// across the whole blob; per-VOD merging is the client's job.
    pub updated_at: i64,
}

pub struct SyncStore {
    path: PathBuf,
    /// Single Mutex covers both the map and the on-disk file — file writes
    /// happen while holding the lock so concurrent PUTs serialize cleanly
    /// without an interleaved-snapshot race. Throughput is not a concern.
    inner: Mutex<HashMap<String, SyncBlob>>,
}

impl SyncStore {
    pub fn new_in_memory(path: PathBuf) -> Self {
        Self {
            path,
            inner: Mutex::new(HashMap::new()),
        }
    }

    pub async fn get(&self, token: &str) -> Option<SyncBlob> {
        self.inner.lock().await.get(token).cloned()
    }

    pub async fn insert_in_memory(&self, token: String, blob: SyncBlob) {
        self.inner.lock().await.insert(token, blob);
    }

    pub async fn len(&self) -> usize {
        self.inner.lock().await.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blob(value: &str, updated_at: i64) -> SyncBlob {
        SyncBlob {
            blob: serde_json::json!({ "resume": { "v": value } }),
            updated_at,
        }
    }

    #[tokio::test]
    async fn get_missing_returns_none() {
        let s = SyncStore::new_in_memory("/tmp/never-used.json".into());
        assert!(s.get("MISSING").await.is_none());
    }

    #[tokio::test]
    async fn insert_then_get_roundtrips() {
        let s = SyncStore::new_in_memory("/tmp/never-used.json".into());
        s.insert_in_memory("ABC".into(), blob("first", 100)).await;
        let got = s.get("ABC").await.unwrap();
        assert_eq!(got.updated_at, 100);
        assert_eq!(got.blob["resume"]["v"], "first");
    }

    #[tokio::test]
    async fn insert_overwrites_existing_token() {
        let s = SyncStore::new_in_memory("/tmp/never-used.json".into());
        s.insert_in_memory("ABC".into(), blob("first", 100)).await;
        s.insert_in_memory("ABC".into(), blob("second", 200)).await;
        let got = s.get("ABC").await.unwrap();
        assert_eq!(got.updated_at, 200);
        assert_eq!(got.blob["resume"]["v"], "second");
        assert_eq!(s.len().await, 1);
    }
}
