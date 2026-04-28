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

    pub async fn load(path: PathBuf) -> Self {
        let map = match tokio::fs::read(&path).await {
            Ok(bytes) => match serde_json::from_slice::<HashMap<String, SyncBlob>>(&bytes) {
                Ok(m) => m,
                Err(e) => {
                    tracing::warn!("sync store parse failed: {e}; starting empty");
                    HashMap::new()
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => HashMap::new(),
            Err(e) => {
                tracing::warn!("sync store read failed: {e}; starting empty");
                HashMap::new()
            }
        };
        tracing::info!("sync store loaded: {} entries", map.len());
        Self {
            path,
            inner: Mutex::new(map),
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

    /// Persist the current map to `self.path` via temp-file + rename.
    pub async fn save_to_disk(&self) -> std::io::Result<()> {
        let snapshot = {
            let g = self.inner.lock().await;
            serde_json::to_vec(&*g)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?
        };
        let tmp = self.path.with_extension("json.tmp");
        tokio::fs::write(&tmp, &snapshot).await?;
        tokio::fs::rename(&tmp, &self.path).await?;
        Ok(())
    }

    /// Insert + persist atomically — the file write happens under the same
    /// lock, so two concurrent PUTs can't produce an interleaved on-disk
    /// snapshot.
    pub async fn put(&self, token: String, blob: SyncBlob) -> std::io::Result<()> {
        let mut g = self.inner.lock().await;
        g.insert(token, blob);
        let bytes = serde_json::to_vec(&*g)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        let tmp = self.path.with_extension("json.tmp");
        tokio::fs::write(&tmp, &bytes).await?;
        tokio::fs::rename(&tmp, &self.path).await?;
        Ok(())
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

    #[tokio::test]
    async fn save_then_load_roundtrips() {
        let dir = tempdir();
        let path = dir.join("sync.json");

        let s1 = SyncStore::new_in_memory(path.clone());
        s1.insert_in_memory("AAA".into(), blob("alpha", 1)).await;
        s1.insert_in_memory("BBB".into(), blob("beta", 2)).await;
        s1.save_to_disk().await.unwrap();

        let s2 = SyncStore::load(path).await;
        assert_eq!(s2.len().await, 2);
        let a = s2.get("AAA").await.unwrap();
        assert_eq!(a.blob["resume"]["v"], "alpha");
    }

    #[tokio::test]
    async fn load_missing_file_starts_empty() {
        let dir = tempdir();
        let path = dir.join("does-not-exist.json");
        let s = SyncStore::load(path).await;
        assert_eq!(s.len().await, 0);
    }

    #[tokio::test]
    async fn load_corrupt_file_starts_empty() {
        let dir = tempdir();
        let path = dir.join("sync.json");
        tokio::fs::write(&path, b"not valid json").await.unwrap();
        let s = SyncStore::load(path).await;
        assert_eq!(s.len().await, 0);
    }

    #[tokio::test]
    async fn put_persists_atomically() {
        let dir = tempdir();
        let path = dir.join("sync.json");
        let s = SyncStore::new_in_memory(path.clone());
        s.put("ZZZ".into(), blob("via-put", 42)).await.unwrap();
        let s2 = SyncStore::load(path).await;
        let got = s2.get("ZZZ").await.unwrap();
        assert_eq!(got.updated_at, 42);
        assert_eq!(got.blob["resume"]["v"], "via-put");
    }

    /// Make a unique tempdir under target/ — the project has no `tempfile`
    /// crate and we don't want to add one for these tests.
    fn tempdir() -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let p = std::env::temp_dir().join(format!("moonmoon-sync-test-{nanos}"));
        std::fs::create_dir_all(&p).unwrap();
        p
    }
}
