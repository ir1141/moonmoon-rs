use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

/// Hard cap for incoming sync blobs. The full payload (including JSON
/// envelope) must fit. 256 KiB is ~150x the size of a fully-populated
/// 500-entry resume map, so this is generous but still bounded.
pub(crate) const MAX_BLOB_BYTES: usize = 256 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SyncBlob {
    /// Opaque blob the server never inspects — currently `{ "resume": {...} }`.
    pub(crate) blob: serde_json::Value,
    /// Client-supplied milliseconds since epoch. Used for last-write-wins
    /// across the whole blob; per-VOD merging is the client's job.
    pub(crate) updated_at: i64,
}

pub struct SyncStore {
    path: PathBuf,
    /// Single Mutex covers both the map and the on-disk file — see `put`.
    inner: Mutex<HashMap<String, SyncBlob>>,
}

/// Read and parse the on-disk map. Returns the underlying `io::Error`
/// (including `NotFound`) so callers can distinguish first-boot from
/// corruption.
async fn read_map(path: &Path) -> std::io::Result<HashMap<String, SyncBlob>> {
    let bytes = tokio::fs::read(path).await?;
    serde_json::from_slice(&bytes)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

/// `path` with `.tmp` appended (not substituted). `with_extension("json.tmp")`
/// would silently rewrite a pathless `sync` to `sync.json.tmp` and rename
/// to `sync` — surprising. Appending preserves whatever path was given.
fn tmp_path(path: &Path) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(".tmp");
    s.into()
}

impl SyncStore {
    pub fn new_in_memory(path: PathBuf) -> Self {
        Self {
            path,
            inner: Mutex::new(HashMap::new()),
        }
    }

    pub async fn load(path: PathBuf) -> Self {
        let map = match read_map(&path).await {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => HashMap::new(),
            Err(e) => {
                tracing::warn!("sync store load failed: {e}; starting empty");
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

    /// Insert and persist under a single lock. We *deliberately* hold the
    /// mutex across the file I/O so concurrent PUTs serialize and can't
    /// produce an interleaved on-disk snapshot. Throughput isn't a concern
    /// — the route is rate-limited and writes are small.
    pub async fn put(&self, token: String, blob: SyncBlob) -> std::io::Result<()> {
        let mut g = self.inner.lock().await;
        g.insert(token, blob);
        let bytes = serde_json::to_vec(&*g)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        let tmp = tmp_path(&self.path);
        tokio::fs::write(&tmp, &bytes).await?;
        tokio::fs::rename(&tmp, &self.path).await?;
        Ok(())
    }

    #[cfg(test)]
    pub async fn insert_in_memory(&self, token: String, blob: SyncBlob) {
        self.inner.lock().await.insert(token, blob);
    }

    #[cfg(test)]
    pub async fn len(&self) -> usize {
        self.inner.lock().await.len()
    }

    #[cfg(test)]
    pub async fn save_to_disk(&self) -> std::io::Result<()> {
        let snapshot = {
            let g = self.inner.lock().await;
            serde_json::to_vec(&*g)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?
        };
        let tmp = tmp_path(&self.path);
        tokio::fs::write(&tmp, &snapshot).await?;
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

    /// Self-cleaning tempdir. Avoids adding a `tempfile` dep just for a
    /// few round-trip tests.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let p = std::env::temp_dir().join(format!("moonmoon-sync-test-{nanos}"));
            std::fs::create_dir_all(&p).unwrap();
            Self(p)
        }

        fn join(&self, name: &str) -> PathBuf {
            self.0.join(name)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
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
        let dir = TempDir::new();
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
        let dir = TempDir::new();
        let path = dir.join("does-not-exist.json");
        let s = SyncStore::load(path).await;
        assert_eq!(s.len().await, 0);
    }

    #[tokio::test]
    async fn load_corrupt_file_starts_empty() {
        let dir = TempDir::new();
        let path = dir.join("sync.json");
        tokio::fs::write(&path, b"not valid json").await.unwrap();
        let s = SyncStore::load(path).await;
        assert_eq!(s.len().await, 0);
    }

    #[tokio::test]
    async fn put_persists_atomically() {
        let dir = TempDir::new();
        let path = dir.join("sync.json");
        let s = SyncStore::new_in_memory(path.clone());
        s.put("ZZZ".into(), blob("via-put", 42)).await.unwrap();
        let s2 = SyncStore::load(path).await;
        let got = s2.get("ZZZ").await.unwrap();
        assert_eq!(got.updated_at, 42);
        assert_eq!(got.blob["resume"]["v"], "via-put");
    }

    #[test]
    fn tmp_path_appends_rather_than_replaces() {
        assert_eq!(
            tmp_path(Path::new("sync.json")),
            PathBuf::from("sync.json.tmp")
        );
        assert_eq!(tmp_path(Path::new("sync")), PathBuf::from("sync.tmp"));
        assert_eq!(
            tmp_path(Path::new("/data/sync.json")),
            PathBuf::from("/data/sync.json.tmp")
        );
    }
}
