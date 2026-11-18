use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use tokio::sync::Mutex;

/// Hard cap for incoming sync blobs. The full payload (including JSON
/// envelope) must fit. 256 KiB is ~150x the size of a fully-populated
/// 500-entry resume map, so this is generous but still bounded.
pub(crate) const MAX_BLOB_BYTES: usize = 256 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SyncBlob {
    /// Opaque blob the server never inspects — currently `{ "resume": {...} }`.
    /// Kept as raw JSON text: a parsed `Value` costs ~16x the wire size.
    pub(crate) blob: Box<RawValue>,
    /// Client-supplied milliseconds since epoch. Used for last-write-wins
    /// across the whole blob; per-VOD merging is the client's job.
    pub(crate) updated_at: i64,
    /// Server-set seconds-since-epoch when this blob was last stored. Used for
    /// eviction order — `updated_at` is client-supplied and gameable.
    #[serde(default)]
    pub(crate) stored_at: i64,
}

/// Hard cap on distinct sync tokens; real usage is a handful of tokens.
pub(crate) const MAX_SYNC_ENTRIES: usize = 10_000;

/// Hard cap on the summed size of all stored blobs. Bounds resident memory
/// and the size of the file every `put` rewrites.
pub(crate) const MAX_STORE_BYTES: usize = 32 * 1024 * 1024;

#[derive(Clone, Copy)]
struct Limits {
    entries: usize,
    bytes: usize,
}

const LIMITS: Limits = Limits {
    entries: MAX_SYNC_ENTRIES,
    bytes: MAX_STORE_BYTES,
};

fn blob_len(blob: &SyncBlob) -> usize {
    blob.blob.get().len()
}

/// Oldest-stored tokens to drop so that `map` plus the incoming blob fits
/// within `limits`. Replacing an existing token frees its old size first.
fn tokens_to_evict(
    map: &HashMap<String, SyncBlob>,
    incoming: &str,
    incoming_len: usize,
    limits: Limits,
) -> Vec<String> {
    let mut others: Vec<(&String, &SyncBlob)> =
        map.iter().filter(|(token, _)| *token != incoming).collect();
    let mut entries = others.len() + 1;
    let mut bytes = incoming_len + others.iter().map(|(_, b)| blob_len(b)).sum::<usize>();
    others.sort_by_key(|(_, b)| b.stored_at);
    let mut evict = Vec::new();
    for (token, blob) in others {
        if entries <= limits.entries && bytes <= limits.bytes {
            break;
        }
        entries -= 1;
        bytes -= blob_len(blob);
        evict.push(token.clone());
    }
    evict
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

/// Where an unparseable store file is moved so the next `put` can't
/// overwrite the only copy of everyone's history.
fn corrupt_path(path: &Path) -> PathBuf {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let mut s = path.as_os_str().to_owned();
    s.push(format!(".corrupt-{secs}"));
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
                let aside = corrupt_path(&path);
                match tokio::fs::rename(&path, &aside).await {
                    Ok(()) => tracing::error!(
                        "sync store load failed: {e}; moved to {} and starting empty",
                        aside.display()
                    ),
                    Err(re) => tracing::error!(
                        "sync store load failed: {e}; could not move aside ({re}); starting empty"
                    ),
                }
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
    ///
    /// `stored_at` is stamped here, not by callers: it's the eviction key,
    /// and the store owning it is what keeps a client-supplied value (e.g.
    /// `i64::MAX`) from gaming eviction order.
    pub async fn put(&self, token: String, mut blob: SyncBlob) -> std::io::Result<()> {
        blob.stored_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let mut g = self.inner.lock().await;
        let evict = tokens_to_evict(&g, &token, blob_len(&blob), LIMITS);
        if !evict.is_empty() {
            tracing::warn!(
                "sync store at capacity; evicting {} oldest token(s)",
                evict.len()
            );
            for t in &evict {
                g.remove(t);
            }
        }
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
    pub(crate) async fn len(&self) -> usize {
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
            blob: raw(serde_json::json!({ "resume": { "v": value } })),
            updated_at,
            stored_at: 0,
        }
    }

    fn blob_with_stored(value: &str, updated_at: i64, stored_at: i64) -> SyncBlob {
        SyncBlob {
            blob: raw(serde_json::json!({ "resume": { "v": value } })),
            updated_at,
            stored_at,
        }
    }

    fn raw(v: serde_json::Value) -> Box<RawValue> {
        RawValue::from_string(v.to_string()).unwrap()
    }

    fn resume_v(b: &SyncBlob) -> serde_json::Value {
        serde_json::from_str::<serde_json::Value>(b.blob.get()).unwrap()["resume"]["v"].clone()
    }

    fn sized(len: usize, stored_at: i64) -> SyncBlob {
        let pad = "x".repeat(len - r#"{"p":""}"#.len());
        SyncBlob {
            blob: raw(serde_json::json!({ "p": pad })),
            updated_at: 0,
            stored_at,
        }
    }

    fn map_of(entries: &[(&str, SyncBlob)]) -> HashMap<String, SyncBlob> {
        entries
            .iter()
            .map(|(t, b)| (t.to_string(), b.clone()))
            .collect()
    }

    #[test]
    fn eviction_at_entry_cap_drops_oldest_stored() {
        let map = map_of(&[("OLD", sized(10, 100)), ("NEW", sized(10, 200))]);
        let limits = Limits {
            entries: 2,
            bytes: usize::MAX,
        };
        assert_eq!(tokens_to_evict(&map, "FRESH", 10, limits), vec!["OLD"]);
        let roomy = Limits {
            entries: 3,
            bytes: usize::MAX,
        };
        assert!(tokens_to_evict(&map, "FRESH", 10, roomy).is_empty());
        assert!(tokens_to_evict(&map, "OLD", 10, limits).is_empty());
    }

    #[test]
    fn eviction_at_byte_cap_drops_oldest_until_incoming_fits() {
        let map = map_of(&[
            ("A", sized(40, 100)),
            ("B", sized(40, 200)),
            ("C", sized(40, 300)),
        ]);
        let limits = Limits {
            entries: usize::MAX,
            bytes: 120,
        };
        assert_eq!(tokens_to_evict(&map, "FRESH", 70, limits), vec!["A", "B"]);
    }

    #[test]
    fn eviction_counts_replaced_token_at_its_new_size() {
        let map = map_of(&[("A", sized(40, 100)), ("B", sized(60, 200))]);
        let limits = Limits {
            entries: usize::MAX,
            bytes: 100,
        };
        assert!(tokens_to_evict(&map, "B", 60, limits).is_empty());
        assert_eq!(tokens_to_evict(&map, "B", 61, limits), vec!["A"]);
    }

    #[test]
    fn sized_helper_produces_exact_length() {
        assert_eq!(blob_len(&sized(40, 0)), 40);
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
        assert_eq!(resume_v(&got), "first");
    }

    #[tokio::test]
    async fn insert_overwrites_existing_token() {
        let s = SyncStore::new_in_memory("/tmp/never-used.json".into());
        s.insert_in_memory("ABC".into(), blob("first", 100)).await;
        s.insert_in_memory("ABC".into(), blob("second", 200)).await;
        let got = s.get("ABC").await.unwrap();
        assert_eq!(got.updated_at, 200);
        assert_eq!(resume_v(&got), "second");
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
        assert_eq!(resume_v(&a), "alpha");
    }

    #[tokio::test]
    async fn load_missing_file_starts_empty() {
        let dir = TempDir::new();
        let path = dir.join("does-not-exist.json");
        let s = SyncStore::load(path).await;
        assert_eq!(s.len().await, 0);
    }

    #[tokio::test]
    async fn load_corrupt_file_starts_empty_and_preserves_original() {
        let dir = TempDir::new();
        let path = dir.join("sync.json");
        tokio::fs::write(&path, b"not valid json").await.unwrap();
        let s = SyncStore::load(path.clone()).await;
        assert_eq!(s.len().await, 0);
        s.put("TOKEN".into(), blob("x", 1)).await.unwrap();

        let mut preserved = Vec::new();
        let mut entries = tokio::fs::read_dir(&dir.0).await.unwrap();
        while let Some(e) = entries.next_entry().await.unwrap() {
            let name = e.file_name().into_string().unwrap();
            if name.starts_with("sync.json.corrupt-") {
                preserved.push(tokio::fs::read(e.path()).await.unwrap());
            }
        }
        assert_eq!(preserved, vec![b"not valid json".to_vec()]);
    }

    #[tokio::test]
    async fn put_then_load_preserves_blob_bytes_verbatim() {
        let dir = TempDir::new();
        let path = dir.join("sync.json");
        let text = r#"{"resume":{"vod-1":{"t":12.5,"w":true}},"extra":[1,2]}"#;
        let s = SyncStore::new_in_memory(path.clone());
        let b = SyncBlob {
            blob: RawValue::from_string(text.to_string()).unwrap(),
            updated_at: 1,
            stored_at: 0,
        };
        s.put("TOKEN".into(), b).await.unwrap();
        let got = SyncStore::load(path).await.get("TOKEN").await.unwrap();
        assert_eq!(got.blob.get(), text);
    }

    #[tokio::test]
    async fn put_stamps_stored_at_ignoring_caller_value() {
        // `stored_at` is the eviction key; a client sending i64::MAX in the
        // PUT body must not be able to pin its blob against eviction.
        let dir = TempDir::new();
        let s = SyncStore::new_in_memory(dir.join("sync.json"));
        s.put("TOKEN".into(), blob_with_stored("x", 1, i64::MAX))
            .await
            .unwrap();
        let got = s.get("TOKEN").await.unwrap();
        assert_ne!(got.stored_at, i64::MAX);
        assert!(got.stored_at > 0);
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
        assert_eq!(resume_v(&got), "via-put");
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
