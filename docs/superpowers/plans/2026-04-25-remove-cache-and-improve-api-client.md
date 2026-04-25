# Remove disk cache & improve upstream API client — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Drop `data/vods.json` and the disk-cache layer; rewrite `fetch_all_vods` as a 4-way concurrent fan-out with `$select` projection; add a 6h in-process refresh ticker so the site stays fresh without on-disk state.

**Architecture:** In-memory `AppState` is unchanged. Boot does one direct upstream fetch (with a 30s timeout, degrades to empty on failure). A `tokio::spawn`'d task fires every 6h, calling a shared `vods::refresh_in_place` that's also reused by `POST /api/refresh`. `fetch_all_vods` switches from a sequential 50/page loop with 200ms sleeps (~17s) to bounded concurrent fan-out (~2-3s).

**Tech Stack:** Rust 2024, axum 0.8, tokio (full features), reqwest 0.12, Askama 0.15, htmx. No new dependencies.

**Spec:** [`docs/superpowers/specs/2026-04-25-remove-cache-and-improve-api-client-design.md`](../specs/2026-04-25-remove-cache-and-improve-api-client-design.md)

---

## File Map

| File | Action | Responsibility |
|---|---|---|
| `src/vods.rs` | Heavy edit | Add `page_url`, `pages`, `RefreshOutcome`, `refresh_in_place`; rewrite `fetch_all_vods` for concurrency; delete `read_cache`/`write_cache`/cache constants; drop `Serialize` derives; simplify `load_vods`. |
| `src/main.rs` | Small additions | Wrap `load_vods` in 30s `tokio::time::timeout`; spawn 6h refresh ticker after `AppState` is built. |
| `src/handlers/api.rs` | Small simplification | `refresh_vods` becomes a thin wrapper that calls `vods::refresh_in_place` and maps `RefreshOutcome` → JSON via a small helper. |
| `Cargo.toml` | No change | `serde_json` stays — `handlers/api.rs` and `handlers/watch.rs` use it for JSON responses independent of the cache. |
| `.gitignore` | One-line removal | Drop the `/data` entry. |
| `data/` | Delete | Whole directory removed; was only ever cache storage. |
| `README.md` | Paragraph edit | Replace "first launch fetches and writes `data/vods.json`" with the new behaviour. |
| `CLAUDE.md` | Paragraph edit | Same; remove `CACHE_MAX_AGE_SECS` references. |

---

## Task 1: Add `page_url` builder + tests (TDD)

**Files:**
- Modify: `src/vods.rs` (test then function)

- [ ] **Step 1: Write the failing test**

Add to `src/vods.rs` inside the existing `#[cfg(test)] mod tests { ... }` block:

```rust
#[test]
fn test_page_url_includes_required_params() {
    let url = page_url(100);
    assert!(url.starts_with("https://archive.overpowered.tv/moonmoon/vods?"));
    assert!(url.contains("$limit=50"), "missing $limit=50: {url}");
    assert!(url.contains("$skip=100"), "missing $skip=100: {url}");
    assert!(url.contains("$sort[createdAt]=-1"), "missing $sort: {url}");
    for field in [
        "id", "title", "createdAt", "duration", "thumbnail_url", "chapters", "youtube",
    ] {
        assert!(
            url.contains(&format!("$select[]={field}")),
            "missing $select[]={field} in: {url}"
        );
    }
}

#[test]
fn test_page_url_skip_zero() {
    let url = page_url(0);
    assert!(url.contains("$skip=0"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib test_page_url -- --nocapture`
Expected: `error[E0425]: cannot find function 'page_url' in this scope` (compile failure).

- [ ] **Step 3: Write minimal implementation**

In `src/vods.rs`, *just below* the existing `const INITIAL_429_BACKOFF_MS: u64 = 250;` line (around line 52), add:

```rust
fn page_url(skip: usize) -> String {
    format!(
        "{API}?$limit={PAGE_SIZE}&$skip={skip}&$sort[createdAt]=-1\
         &$select[]=id&$select[]=title&$select[]=createdAt\
         &$select[]=duration&$select[]=thumbnail_url\
         &$select[]=chapters&$select[]=youtube"
    )
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib test_page_url`
Expected: 2 passed.

- [ ] **Step 5: Commit**

```bash
git add src/vods.rs
git commit -m "Add page_url builder for paginated upstream fetches"
```

---

## Task 2: Add `pages(total)` helper + tests (TDD)

**Files:**
- Modify: `src/vods.rs` (test then function)

- [ ] **Step 1: Write the failing test**

Add to the existing `#[cfg(test)] mod tests` block in `src/vods.rs`:

```rust
#[test]
fn test_pages_handles_edges() {
    assert_eq!(pages(0), 0);
    assert_eq!(pages(1), 1);
    assert_eq!(pages(50), 1);
    assert_eq!(pages(51), 2);
    assert_eq!(pages(100), 2);
    assert_eq!(pages(1419), 29);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib test_pages_handles_edges`
Expected: `error[E0425]: cannot find function 'pages' in this scope`.

- [ ] **Step 3: Write minimal implementation**

In `src/vods.rs`, immediately after the `page_url` function added in Task 1, add:

```rust
fn pages(total: usize) -> usize {
    total.div_ceil(PAGE_SIZE)
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib test_pages_handles_edges`
Expected: 1 passed.

- [ ] **Step 5: Commit**

```bash
git add src/vods.rs
git commit -m "Add pages() helper for ceiling page count"
```

---

## Task 3: Add `RefreshOutcome` enum + JSON-mapping test + extract `refresh_in_place`

This task moves the body of `handlers::api::refresh_vods` into a new `vods::refresh_in_place` and introduces the typed `RefreshOutcome`. The HTTP handler shrinks to a thin wrapper. Behaviour is unchanged — same JSON shapes, same `refresh_lock`, same `total`-check no-op, same `write_cache` call (we delete that in Task 6, not here).

**Files:**
- Modify: `src/vods.rs` (add enum and `refresh_in_place`)
- Modify: `src/handlers/api.rs` (delegate, add JSON mapping helper + tests)

- [ ] **Step 1: Add the `RefreshOutcome` enum to `src/vods.rs`**

Near the top of `src/vods.rs`, after the existing `pub struct Game { ... }` block, add:

```rust
#[must_use]
#[derive(Debug, Clone)]
pub enum RefreshOutcome {
    Busy,
    Unchanged(usize),
    Refreshed(usize),
    Error(String),
}
```

- [ ] **Step 2: Add `refresh_in_place` to `src/vods.rs`**

Append to `src/vods.rs` (above the `#[cfg(test)] mod tests` block):

```rust
pub async fn refresh_in_place(state: &crate::SharedState) -> RefreshOutcome {
    let _refresh_guard = match state.refresh_lock.try_lock() {
        Ok(g) => g,
        Err(_) => {
            tracing::info!("refresh: already in progress, skipping");
            return RefreshOutcome::Busy;
        }
    };

    let cached_count = state.vods.read().await.len();

    let remote_count = match fetch_vod_count(&state.http_client).await {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("refresh: failed to check vod count: {e}");
            return RefreshOutcome::Error(format!("failed to check vod count: {e}"));
        }
    };

    if remote_count == cached_count {
        tracing::info!("refresh: vod count unchanged ({cached_count})");
        return RefreshOutcome::Unchanged(cached_count);
    }

    tracing::info!("refresh: vod count changed ({cached_count} -> {remote_count}), fetching...");
    let new_vods = match fetch_all_vods(&state.http_client).await {
        Ok(v) => v,
        Err(e) => {
            tracing::error!("refresh: failed to fetch vods: {e}");
            return RefreshOutcome::Error(format!("failed to fetch vods: {e}"));
        }
    };

    let new_vods = std::sync::Arc::new(new_vods);
    let new_games = std::sync::Arc::new(build_games(&new_vods));
    let count = new_vods.len();

    let vods_for_cache = std::sync::Arc::clone(&new_vods);
    if let Err(e) =
        tokio::task::spawn_blocking(move || write_cache(&vods_for_cache)).await
    {
        tracing::warn!("refresh: cache write task join error: {e}");
    }

    {
        let mut vods_w = state.vods.write().await;
        let mut games_w = state.games.write().await;
        *vods_w = new_vods;
        *games_w = new_games;
    }

    tracing::info!("refresh: complete ({count} vods)");
    RefreshOutcome::Refreshed(count)
}
```

(The `write_cache` call stays here for now; Task 6 deletes it together with the rest of the cache layer. Keeping it here means the codebase still compiles and behaves identically after this task.)

- [ ] **Step 3: Add JSON-mapping helper + tests to `src/handlers/api.rs`**

Replace the body of `pub async fn refresh_vods(...)` in `src/handlers/api.rs` (lines 72-134) with:

```rust
pub async fn refresh_vods(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let outcome = crate::vods::refresh_in_place(&state).await;
    Json(outcome_to_json(outcome))
}

fn outcome_to_json(outcome: crate::vods::RefreshOutcome) -> serde_json::Value {
    use crate::vods::RefreshOutcome;
    match outcome {
        RefreshOutcome::Busy => serde_json::json!({ "status": "busy" }),
        RefreshOutcome::Unchanged(count) => {
            serde_json::json!({ "status": "unchanged", "count": count })
        }
        RefreshOutcome::Refreshed(count) => {
            serde_json::json!({ "status": "refreshed", "count": count })
        }
        RefreshOutcome::Error(message) => {
            serde_json::json!({ "status": "error", "message": message })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vods::RefreshOutcome;

    #[test]
    fn test_outcome_to_json_busy() {
        let v = outcome_to_json(RefreshOutcome::Busy);
        assert_eq!(v, serde_json::json!({ "status": "busy" }));
    }

    #[test]
    fn test_outcome_to_json_unchanged() {
        let v = outcome_to_json(RefreshOutcome::Unchanged(1419));
        assert_eq!(v, serde_json::json!({ "status": "unchanged", "count": 1419 }));
    }

    #[test]
    fn test_outcome_to_json_refreshed() {
        let v = outcome_to_json(RefreshOutcome::Refreshed(1420));
        assert_eq!(v, serde_json::json!({ "status": "refreshed", "count": 1420 }));
    }

    #[test]
    fn test_outcome_to_json_error() {
        let v = outcome_to_json(RefreshOutcome::Error("boom".into()));
        assert_eq!(v, serde_json::json!({ "status": "error", "message": "boom" }));
    }
}
```

Also remove the now-unused `use std::sync::Arc;` line in `src/handlers/api.rs` if rustc warns about it after the edit. (It's used by the chat proxy section as well — leave it if `chat_proxy` still references `Arc`. If clippy/build complains in Step 5, drop it.)

- [ ] **Step 4: Run tests + build**

Run: `cargo build && cargo test --lib`
Expected: clean build, all existing tests still pass, four new `outcome_to_json_*` tests pass.

- [ ] **Step 5: Manual smoke test**

Run in one terminal: `cargo run`
Run in another: `curl -XPOST localhost:3000/api/refresh`
Expected: response JSON has the same shape as before — one of `{"status":"unchanged","count":N}`, `{"status":"refreshed","count":N}`, `{"status":"busy"}`, or `{"status":"error","message":"..."}`. Stop the server.

- [ ] **Step 6: Commit**

```bash
git add src/vods.rs src/handlers/api.rs
git commit -m "Extract refresh_in_place to vods.rs with typed RefreshOutcome

The HTTP handler now delegates to a shared async function so the upcoming
6h refresh ticker can call the same code path. JSON output shapes are
unchanged."
```

---

## Task 4: Rewrite `fetch_all_vods` for concurrent fan-out

This is the performance change. Same input/output as today, faster.

**Files:**
- Modify: `src/vods.rs`

- [ ] **Step 1: Add the concurrency constant**

In `src/vods.rs`, immediately after `const PAGE_SIZE: usize = 50;`, add:

```rust
const MAX_CONCURRENT_PAGES: usize = 4;
```

- [ ] **Step 2: Replace the body of `fetch_all_vods`**

Replace the entire current `fetch_all_vods` function (currently lines ~88-115) with:

```rust
pub async fn fetch_all_vods(client: &reqwest::Client) -> Result<Vec<Vod>, reqwest::Error> {
    use std::sync::Arc;
    use tokio::sync::Semaphore;
    use tokio::task::JoinSet;

    let first = fetch_api_response(client, &page_url(0)).await?;
    let total = first.total;
    tracing::info!("fetching {total} vods...");

    let total_pages = pages(total);
    if total_pages == 0 {
        return Ok(Vec::new());
    }

    let mut buckets: Vec<Option<Vec<Vod>>> = (0..total_pages).map(|_| None).collect();
    buckets[0] = Some(first.data);

    if total_pages > 1 {
        let sem = Arc::new(Semaphore::new(MAX_CONCURRENT_PAGES));
        let mut joins: JoinSet<Result<(usize, Vec<Vod>), reqwest::Error>> = JoinSet::new();

        for page_idx in 1..total_pages {
            let permit = Arc::clone(&sem)
                .acquire_owned()
                .await
                .expect("semaphore not closed");
            let client = client.clone();
            joins.spawn(async move {
                let _permit = permit;
                let resp = fetch_api_response(&client, &page_url(page_idx * PAGE_SIZE)).await?;
                Ok((page_idx, resp.data))
            });
        }

        while let Some(res) = joins.join_next().await {
            let (idx, data) = res.expect("page-fetch task panicked")?;
            buckets[idx] = Some(data);
            tracing::debug!("page {} of {} done", idx + 1, total_pages);
        }
    }

    let result: Vec<Vod> = buckets
        .into_iter()
        .collect::<Option<Vec<Vec<Vod>>>>()
        .expect("all page slots filled before flatten")
        .into_iter()
        .flatten()
        .collect();

    tracing::info!("{} / {} vods fetched", result.len(), total);
    Ok(result)
}
```

(The `collect::<Option<Vec<_>>>()` step turns `Vec<Option<Vec<Vod>>>` into `Option<Vec<Vec<Vod>>>` and panics with a clear message if any slot is `None` — guarding against a logic bug rather than silently dropping pages.)

- [ ] **Step 3: Build + run all tests**

Run: `cargo build && cargo test`
Expected: clean build, all tests pass (we haven't added a unit test for `fetch_all_vods` itself — this is verified by the manual smoke test below since it hits the live API).

- [ ] **Step 4: Manual smoke test — full fetch**

Delete `data/vods.json` if it exists, then run:

```bash
rm -f data/vods.json
RUST_LOG=moonmoon=debug,tower_http=debug cargo run
```

Expected log output includes:
- `fetching 1419 vods...` (or whatever the current total is)
- Multiple concurrent `GET … vods?$limit=50&$skip=…` lines interleaved (4 in flight, not strictly sequential)
- Final line: `1419 / 1419 vods fetched` (or matching count) within ~3 seconds of the first request log

Visit `http://localhost:3000` — confirm games grid renders normally. Stop the server.

- [ ] **Step 5: Commit**

```bash
git add src/vods.rs
git commit -m "Fan out upstream paginated fetch concurrently

Replaces the sequential 50/page loop with a JoinSet bounded by a
4-permit semaphore. Drops the priming \$limit=1 call and the unconditional
200ms sleep between pages. Adds \$select projection to skip fields we
never deserialize. Cold-fetch wall time goes from ~17s to ~2-3s."
```

---

## Task 5: Drop disk cache layer

After this task `data/vods.json` is no longer read or written. The directory and `.gitignore` entry are cleaned up in Task 8.

**Files:**
- Modify: `src/vods.rs`

- [ ] **Step 1: Delete cache constants**

In `src/vods.rs`, delete these two lines (currently lines 5-6):

```rust
const CACHE_PATH: &str = "data/vods.json";
const CACHE_MAX_AGE_SECS: u64 = 86400;
```

Also delete the `use std::path::Path;` and `use std::time::{Duration, SystemTime};` imports at the top of the file *if* nothing else uses them after the next steps. Check before deleting; `Duration` may still be referenced. If unsure, leave them — `cargo clippy --all-targets -- -D warnings` will flag any unused import in Step 6.

- [ ] **Step 2: Delete `read_cache` and `write_cache`**

In `src/vods.rs`, delete the entire `fn read_cache() -> Option<Vec<Vod>> { ... }` function (currently lines ~117-162) and the entire `pub fn write_cache(vods: &[Vod]) { ... }` function (currently lines ~164-180).

- [ ] **Step 3: Drop `Serialize` derive from data types**

In `src/vods.rs`, change:

```rust
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Vod { ... }

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Chapter { ... }

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct YoutubeVideo { ... }
```

to:

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct Vod { ... }

#[derive(Debug, Clone, Deserialize)]
pub struct Chapter { ... }

#[derive(Debug, Clone, Deserialize)]
pub struct YoutubeVideo { ... }
```

Then in the same file remove `Serialize` from the import line at the top:

```rust
// before
use serde::{Deserialize, Serialize};
// after
use serde::Deserialize;
```

- [ ] **Step 4: Simplify `load_vods`**

Replace the existing `pub async fn load_vods(client: &reqwest::Client) -> Vec<Vod> { ... }` (currently lines ~182-198) with:

```rust
pub async fn load_vods(client: &reqwest::Client) -> Vec<Vod> {
    match fetch_all_vods(client).await {
        Ok(vods) => vods,
        Err(e) => {
            tracing::error!("failed to fetch vods: {e}");
            tracing::error!("starting with 0 vods — site will be empty until next refresh");
            Vec::new()
        }
    }
}
```

- [ ] **Step 5: Remove the `write_cache` call from `refresh_in_place`**

In the `refresh_in_place` function added in Task 3, delete this block:

```rust
let vods_for_cache = std::sync::Arc::clone(&new_vods);
if let Err(e) =
    tokio::task::spawn_blocking(move || write_cache(&vods_for_cache)).await
{
    tracing::warn!("refresh: cache write task join error: {e}");
}
```

- [ ] **Step 6: Build, lint, and run all tests**

Run: `cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test`
Expected: clean build, no warnings, all tests pass.

If clippy flags any unused imports (e.g., `std::path::Path`, `std::time::SystemTime`, `Duration`, `Serialize`), delete those import lines.

- [ ] **Step 7: Manual smoke test**

```bash
rm -rf data/
cargo run
```

Expected: server boots, fetches all VODs from upstream, serves the site at `http://localhost:3000`. Confirm `data/` directory is **not** recreated. Stop the server.

```bash
ls data/ 2>&1
```

Expected: `ls: cannot access 'data/': No such file or directory`.

- [ ] **Step 8: Commit**

```bash
git add src/vods.rs
git commit -m "Remove disk cache layer

Boots now fetch directly from upstream every time; degrade-to-empty
on failure (logged) is unchanged. The data/ directory and .gitignore
entry are cleaned up in a follow-up commit."
```

---

## Task 6: Add 30s boot fetch timeout in `main.rs`

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Replace the boot fetch line**

In `src/main.rs`, replace this line (currently line 36):

```rust
    let all_vods = vods::load_vods(&http_client).await;
```

with:

```rust
    const BOOT_FETCH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);
    let all_vods = match tokio::time::timeout(BOOT_FETCH_TIMEOUT, vods::load_vods(&http_client))
        .await
    {
        Ok(v) => v,
        Err(_) => {
            tracing::error!(
                "boot fetch timed out after {:?}; starting with 0 vods",
                BOOT_FETCH_TIMEOUT
            );
            Vec::new()
        }
    };
```

- [ ] **Step 2: Build and run**

Run: `cargo build && cargo run`
Expected: server boots normally; the timeout has no visible effect on the happy path.

Stop the server.

- [ ] **Step 3: Commit**

```bash
git add src/main.rs
git commit -m "Time-box the boot upstream fetch at 30 seconds

Without a disk cache, a stuck upstream would otherwise stall the deploy.
The 6h refresh ticker (added next) heals the empty state."
```

---

## Task 7: Spawn the 6h refresh ticker in `main.rs`

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Add the ticker spawn**

In `src/main.rs`, immediately after the `let state = Arc::new(AppState { ... });` block (currently around line 47), add:

```rust
    const REFRESH_INTERVAL: std::time::Duration = std::time::Duration::from_secs(6 * 60 * 60);
    let refresh_state = Arc::clone(&state);
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(REFRESH_INTERVAL);
        tick.tick().await; // swallow the immediate first tick — boot just fetched
        loop {
            tick.tick().await;
            match vods::refresh_in_place(&refresh_state).await {
                vods::RefreshOutcome::Refreshed(n) => {
                    tracing::info!("tick refresh: refreshed {n} vods")
                }
                vods::RefreshOutcome::Unchanged(n) => {
                    tracing::debug!("tick refresh: unchanged ({n})")
                }
                vods::RefreshOutcome::Busy => {
                    tracing::debug!("tick refresh: skipped (busy)")
                }
                vods::RefreshOutcome::Error(e) => {
                    tracing::warn!("tick refresh: {e}")
                }
            }
        }
    });
```

- [ ] **Step 2: Build and run all tests**

Run: `cargo build && cargo test && cargo clippy --all-targets -- -D warnings`
Expected: clean build, no warnings, all tests pass.

- [ ] **Step 3: Manual smoke test — ticker logs at startup**

Run: `RUST_LOG=moonmoon=debug cargo run`
Expected: server boots, `ready with N vods` log appears, no immediate `tick refresh:` log (because we swallow the first tick). Stop after a few seconds — the next tick is in 6h.

- [ ] **Step 4: Commit**

```bash
git add src/main.rs
git commit -m "Spawn 6-hour background refresh ticker

Replaces the disk cache's TTL-based staleness check. The ticker shares
the existing refresh_lock with POST /api/refresh, so manual and
automatic refreshes can't collide."
```

---

## Task 8: Clean up `data/` directory and `.gitignore`

**Files:**
- Delete: `data/` (directory)
- Modify: `.gitignore`

- [ ] **Step 1: Remove the directory**

Run:

```bash
rm -rf data
```

- [ ] **Step 2: Remove the `/data` line from `.gitignore`**

Edit `.gitignore` and delete the line containing exactly `/data`. The remaining lines (`/target`, `CLAUDE.md`, etc.) stay.

- [ ] **Step 3: Confirm clean working tree picks up the change**

Run: `git status`
Expected: `.gitignore` modified, no other changes (the `data/` directory was already gitignored, so its deletion is invisible to git).

- [ ] **Step 4: Commit**

```bash
git add .gitignore
git commit -m "Drop /data from .gitignore — the directory is no longer used"
```

---

## Task 9: Update README and CLAUDE.md

**Files:**
- Modify: `README.md`
- Modify: `CLAUDE.md`

- [ ] **Step 1: Update `README.md`**

In `README.md`, find the "Running" section (lines ~32-52). Replace this paragraph:

```markdown
Serves on `http://0.0.0.0:3000`. On first launch it fetches every VOD from `https://archive.overpowered.tv/moonmoon/vods` and writes `data/vods.json`. Subsequent launches reuse that cache if it's younger than 24 hours.
```

with:

```markdown
Serves on `http://0.0.0.0:3000`. Every boot fetches the full VOD catalog from `https://archive.overpowered.tv/moonmoon/vods` (concurrent paged fetch, ~2-3 seconds). A background task refreshes every 6 hours, gated by a cheap upstream `total`-changed check so idle ticks cost one tiny request.
```

In `README.md`, find the file tree section (around line 76-94) and update the line:

```
data/vods.json           # Cached upstream payload (gitignored)
```

— delete that line entirely.

- [ ] **Step 2: Update `CLAUDE.md`**

In `CLAUDE.md`, find the **Commands** section. Replace this bullet:

```markdown
- `cargo run` — starts the server on `http://0.0.0.0:3000`. On first launch it fetches all VODs from `https://archive.overpowered.tv/moonmoon/vods` and writes `data/vods.json`; subsequent launches load from that cache if younger than 24h.
```

with:

```markdown
- `cargo run` — starts the server on `http://0.0.0.0:3000`. Every boot fetches all VODs from `https://archive.overpowered.tv/moonmoon/vods` directly (~2-3s, concurrent paged); a 6h background ticker refreshes thereafter.
```

Also in `CLAUDE.md`, find the **Data layer (`src/vods.rs`)** subsection and replace this bullet:

```markdown
- `load_vods()` → checks `data/vods.json` cache (`CACHE_MAX_AGE_SECS = 86400`), falls back to `fetch_all_vods()` which paginates the `archive.overpowered.tv` API at 50/page with a 200ms sleep between pages.
```

with:

```markdown
- `load_vods()` → calls `fetch_all_vods()` directly (no cache). `fetch_all_vods` paginates the `archive.overpowered.tv` API at 50/page with a 4-way concurrent fan-out (`MAX_CONCURRENT_PAGES`) bounded by a `Semaphore`, all-or-nothing on partial failure.
```

In the **Conventions** section of `CLAUDE.md`, the existing bullet about no-anyhow/no-thiserror stays as-is.

- [ ] **Step 3: Verify the documentation builds and renders sensibly**

Run: `git diff README.md CLAUDE.md`
Read the diff and confirm: no leftover references to `data/vods.json`, `CACHE_MAX_AGE_SECS`, or the 200ms sleep.

Run: `git grep -i 'data/vods.json\|CACHE_MAX_AGE_SECS\|read_cache\|write_cache\|CACHE_PATH'`
Expected: no matches anywhere in the repo (other than possibly in `docs/superpowers/` which is fine — historical).

- [ ] **Step 4: Commit**

```bash
git add README.md CLAUDE.md
git commit -m "Update README and CLAUDE.md for cache-less boot model"
```

---

## Task 10: Final verification

**Files:** none modified.

- [ ] **Step 1: Run the full check suite**

Run, in order:

```bash
cargo fmt -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Expected: all three pass with no warnings.

- [ ] **Step 2: Cold-boot wall-clock check**

```bash
rm -rf data/
time cargo run --release &  # let it boot, then ctrl-c
```

Watch the logs. Expected: from `fetching N vods...` to `ready with N vods` is **under 5 seconds**, ideally ~2-3s in `--release`. Stop the server with Ctrl-C.

- [ ] **Step 3: Refresh-while-idle check**

In one terminal: `cargo run`
In another:

```bash
curl -sS -XPOST localhost:3000/api/refresh | python3 -m json.tool
```

Expected: `{"count": <N>, "status": "unchanged"}`.

- [ ] **Step 4: Concurrent-refresh-busy check**

While the server is still running, run two refreshes back-to-back:

```bash
(curl -sS -XPOST localhost:3000/api/refresh & curl -sS -XPOST localhost:3000/api/refresh & wait) | tee /tmp/refresh.log
```

Expected: at least one response is `{"status":"busy"}` *if* the upstream `total`-check is slow enough for them to overlap. If both come back `unchanged` (because each finished in <1ms before the other started), this is fine — the lock is wired correctly even if it didn't fire here. Stop the server.

- [ ] **Step 5: No leftover cache references**

Run:

```bash
git grep -i 'data/vods.json\|CACHE_MAX_AGE_SECS\|read_cache\|write_cache\|CACHE_PATH' -- ':!docs/superpowers/'
```

Expected: no output.

- [ ] **Step 6: Concurrency check via debug logs**

```bash
rm -rf data/
RUST_LOG=moonmoon=debug cargo run 2>&1 | head -60
```

Expected: at least 4 `page N of M done` lines appear with non-monotonic ordering (because they complete out of order). Stop the server.

- [ ] **Step 7: No commit needed — verification only.**

If all six checks pass, the implementation is complete.

---

## Self-Review Notes

**Spec coverage:**
- "Drop disk cache layer" → Task 5
- "Boot fetches all VODs directly" → Task 5 + Task 6 (timeout)
- "6h background timer" → Task 7
- "`POST /api/refresh` JSON shapes unchanged" → Task 3 (with explicit JSON-mapping tests)
- "Cold-fetch ~2-3s" → Task 4 + Task 10 verification
- "Boot succeeds when upstream is down" → Task 5 (degrade in `load_vods`) + Task 6 (timeout)
- "`refresh_in_place` shared between handler and ticker" → Task 3 + Task 7
- "All-or-nothing fan-out" → Task 4
- "`#[must_use]` on `RefreshOutcome`" → Task 3
- "`usize::div_ceil` for `pages`" → Task 2
- "Descriptive `.expect()` messages" → Task 4
- "`$select` projection" → Task 1 (`page_url`)
- "`MAX_CONCURRENT_PAGES = 4`" → Task 4
- "Delete `data/` + `.gitignore` cleanup" → Task 8
- "README + CLAUDE.md updates" → Task 9
- "Tests: `page_url`, `pages`, `RefreshOutcome` JSON" → Tasks 1, 2, 3

**Type/name consistency:**
- `RefreshOutcome` variants: `Busy | Unchanged(usize) | Refreshed(usize) | Error(String)` — used identically in `vods.rs`, the JSON helper, and the ticker.
- `MAX_CONCURRENT_PAGES`, `PAGE_SIZE`, `BOOT_FETCH_TIMEOUT`, `REFRESH_INTERVAL` — referenced consistently.
- `crate::SharedState` — used in `refresh_in_place`'s signature; matches the existing `pub type SharedState = Arc<AppState>` in `main.rs`.
- `vods::refresh_in_place` — same name in spec, plan, and call sites.

**Placeholder scan:** none. All code blocks are complete; all paths are exact.

**One spec deviation, intentional:** the spec said "drop `serde_json` if unused after `Serialize` removal." Verified: `serde_json` is independently used by `handlers/api.rs` (for the JSON response macros) and `handlers/watch.rs` (for `Json` and `to_string`). It stays.
