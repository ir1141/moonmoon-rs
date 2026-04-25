# Remove disk cache & improve upstream API client — Design

**Date:** 2026-04-25
**Status:** Proposed
**Owner:** ir1141

## Motivation

Two pressures push the same direction:

1. **Stale data.** `data/vods.json` lives up to 24h. moonmoon streams roughly daily; if the cache lands at the wrong time, fresh VODs don't appear until the next rebuild.
2. **Online-host readiness.** Most cheap PaaS targets (Fly.io, Railway, generic containers) have ephemeral filesystems where a JSON cache is at best useless and at worst surprising. We want a deploy artefact that has no on-disk state.

Removing the cache also nudges us to fix latent inefficiency in `fetch_all_vods` — once every boot is a cold boot, the ~17s sequential paginate-with-sleeps loop stops being acceptable.

## Goals

- Drop `data/vods.json` and the entire disk-cache layer.
- Boot fetches all VODs from upstream directly, into the existing in-memory `AppState`.
- A background timer refreshes every 6 hours, gated by the cheap "is `total` unchanged?" check that already exists.
- Cold-fetch wall time falls from ~17s to ~2–3s by replacing sequential pagination with bounded concurrent fan-out.
- HTTP API surface is unchanged. `POST /api/refresh` returns the same JSON shapes.
- Boot still succeeds when upstream is down (degrade to empty `vods`); the 6h tick or a manual refresh self-heals.

## Non-goals

- Per-instance shared cache (Redis/DB). Multi-instance deploys will each fetch independently — fine for the foreseeable scale.
- Per-request upstream queries (the original SPA model). Considered and rejected — would force a handler rewrite and lose the in-memory benefits.
- Anyhow/thiserror introduction. Existing convention (`Result<_, reqwest::Error>` + log-and-degrade) still fits.
- Graceful shutdown of the refresh ticker.
- Health endpoint that flips on `vods.is_empty()`.

## Design

### Architecture

```
                ┌─────────────────────────────────┐
                │  archive.overpowered.tv API     │
                └────────────────┬────────────────┘
                                 │ concurrent paged GETs (4 in flight)
                                 │ with $select projection + 429 backoff
                                 ▼
                ┌─────────────────────────────────┐
                │  vods::fetch_all_vods()         │
                └────────────────┬────────────────┘
                                 │ Vec<Vod>
                                 ▼
                  ┌──────────────────────────────┐
   boot ─────────►│  AppState                    │◄─── 6h refresh ticker
   POST /refresh ►│   vods:  RwLock<Arc<Vec<…>>> │     (tokio::spawn task)
                  │   games: RwLock<Arc<Vec<…>>> │
                  └──────────────┬───────────────┘
                                 │ Arc clones (cheap)
                                 ▼
                  ┌──────────────────────────────┐
                  │  axum handlers (unchanged)   │
                  └──────────────────────────────┘
```

### Components

#### `src/vods.rs`

**Delete:**
- `CACHE_PATH`, `CACHE_MAX_AGE_SECS` constants.
- `read_cache()`, `write_cache()`.
- `Serialize` derive on `Vod` / `Chapter` / `YoutubeVideo` (only used by the cache; `Deserialize` stays).
- The `read_cache()` short-circuit inside `load_vods()`.

**Add:**
- `MAX_CONCURRENT_PAGES: usize = 4`.
- A small `page_url(skip: usize) -> String` helper that produces:
  ```
  https://archive.overpowered.tv/moonmoon/vods
    ?$limit=50
    &$skip={skip}
    &$sort[createdAt]=-1
    &$select[]=id&$select[]=title&$select[]=createdAt
    &$select[]=duration&$select[]=thumbnail_url
    &$select[]=chapters&$select[]=youtube
  ```
- `pages(total: usize) -> usize` (ceiling division).
- `pub async fn refresh_in_place(state: &SharedState) -> RefreshOutcome` containing the body of today's `handlers::api::refresh_vods` minus the cache-write step. Used by both the HTTP handler and the 6h ticker.
- `pub enum RefreshOutcome { Busy, Unchanged(usize), Refreshed(usize), Error(String) }`.

**Rewrite `fetch_all_vods`:**
1. Fetch page 0 synchronously; learn `total` from its response (no priming `$limit=1` request).
2. Pre-allocate `Vec<Option<Vec<Vod>>>` of length `pages(total)`; set slot 0 from step 1.
3. Spawn the remaining pages onto a `JoinSet`, gated by `Arc<Semaphore::new(MAX_CONCURRENT_PAGES)>`. Each task acquires an owned permit, calls `fetch_api_response`, and returns `(idx, Vec<Vod>)`.
4. Drain the `JoinSet`. Any task error → return `Err` immediately; partial results are discarded (all-or-nothing).
5. Flatten the index-ordered slots into a single `Vec<Vod>`.

`fetch_api_response`, `fetch_vod_count`, `should_retry`, `backoff_delay`, `build_games`, `upscale_chapter_image`: unchanged.

`load_vods` becomes: call `fetch_all_vods`, log+degrade to `vec![]` on error.

#### `src/main.rs`

Wrap the boot fetch in a 30-second timeout:

```rust
const BOOT_FETCH_TIMEOUT: Duration = Duration::from_secs(30);
let vods = tokio::time::timeout(BOOT_FETCH_TIMEOUT, vods::load_vods(&client))
    .await
    .unwrap_or_else(|_| {
        tracing::error!("boot fetch timed out after 30s; starting empty");
        vec![]
    });
```

After `AppState` is built, spawn a 6h refresh ticker:

```rust
const REFRESH_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);
let refresh_state = state.clone();
tokio::spawn(async move {
    let mut tick = tokio::time::interval(REFRESH_INTERVAL);
    tick.tick().await; // swallow immediate fire (just booted)
    loop {
        tick.tick().await;
        match vods::refresh_in_place(&refresh_state).await {
            RefreshOutcome::Refreshed(n)  => tracing::info!("tick refresh: {n} vods"),
            RefreshOutcome::Unchanged(n)  => tracing::debug!("tick refresh: unchanged ({n})"),
            RefreshOutcome::Busy          => tracing::debug!("tick refresh: busy, skipping"),
            RefreshOutcome::Error(e)      => tracing::warn!("tick refresh: {e}"),
        }
    }
});
```

#### `src/handlers/api.rs`

`refresh_vods` shrinks to: call `vods::refresh_in_place(&state)`, map `RefreshOutcome` → JSON (same shape as today: `{status: "busy"|"unchanged"|"refreshed"|"error", count?, message?}`).

#### `Cargo.toml`

Drop `serde_json` if it ends up unused after removing the cache. Verified via `cargo build` + `git grep serde_json::` post-edit.

#### Filesystem

- Delete `data/` directory.
- Remove `data/` and `data/vods.json` from `.gitignore` if present.

#### Documentation

- `README.md` — replace the "first launch fetches and writes `data/vods.json`" paragraph with a one-liner: "Every boot fetches the full catalog from upstream (~2–3s); a 6h background refresh keeps it current."
- `CLAUDE.md` — same edit; also remove references to `CACHE_MAX_AGE_SECS`.

### Data flow

**Boot:**

```
main.rs
  ├─ build http_client
  ├─ tokio::time::timeout(30s, vods::load_vods(&client))
  │     ├─ on Ok(Vec<Vod>) → use it
  │     ├─ on Err (timeout) → vec![], log error
  │     └─ load_vods on internal Err → vec![], log error
  ├─ vods::build_games(&vods) (sync)
  ├─ AppState { vods: Arc, games: Arc, http_client, refresh_lock }
  ├─ tokio::spawn(refresh_ticker(state))
  └─ axum::serve  starts immediately, even if vods is empty
```

**Handler request (unchanged):**

```
handler
  ├─ state.vods.read().await           (microseconds)
  ├─ Arc::clone(&*guard)                (pointer bump)
  ├─ drop(guard)                         (released BEFORE any .await)
  └─ filter / paginate / render Askama template
```

**Refresh path (shared by HTTP and tick):**

```
refresh_in_place(state)
  ├─ try_lock(refresh_lock)             ─► Busy ⇒ return
  ├─ cached_count = state.vods.read().await.len()
  ├─ remote_count = fetch_vod_count(&client).await?     ($limit=1, ~1 request)
  ├─ if remote_count == cached_count    ─► Unchanged(cached_count)
  ├─ new_vods  = fetch_all_vods(&client).await?         (concurrent fan-out)
  ├─ new_games = build_games(&new_vods)
  ├─ atomic-ish: write_lock vods+games, swap both Arcs
  └─ Refreshed(count)
```

### Concurrency invariants

1. **Ticker and `/api/refresh` cannot collide.** Both go through `refresh_in_place`, which `try_lock`s `refresh_lock`. A tick during a manual refresh becomes `Busy` and is silently skipped; the next tick (6h later) picks up.
2. **Read locks never cross `.await`.** Same convention as today.
3. **Tiny `Arc`-swap window** between writing `vods` and writing `games`. Acceptable: a request landing in the window sees a freshly-counted VOD list with games from the previous generation. Worst case is a single new game appearing milliseconds late.
4. **No graceful-shutdown wiring** for the ticker; it dies with the runtime.

### Error handling

| Source | Reaction | User-visible effect |
|---|---|---|
| Boot fetch network/5xx fail | Log `error!`, `vods = vec![]` | Empty site until next refresh |
| Boot fetch exceeds 30s timeout | Log `error!`, `vods = vec![]` | Same as above |
| Tick `total` check fails | Log `warn!`, return `RefreshOutcome::Error`, ticker logs and continues | Site keeps stale data |
| Tick `fetch_all_vods` fails | Same | Site keeps stale data |
| Manual `/api/refresh` fails | HTTP `200 OK` with `{status:"error", message}` (matches today) | Caller sees error JSON |
| Upstream 429 mid-fetch | Existing `MAX_429_RETRIES = 3` exponential backoff (250→500→1000ms) | Slower fetch |
| Partial fan-out failure | All-or-nothing — discard, return `Err` | `vods` unchanged |
| `refresh_lock` already held | `RefreshOutcome::Busy` | Tick: silent. Manual: `{status:"busy"}` |
| Empty `vods` at request time | Templates render empty grids cleanly | Empty site, no error |

Tick failures log at `warn!`. Manual `/api/refresh` failures log at `error!` (a human asked).

### Testing

**Keep:** all existing tests (`test_vod_deserialize`, `test_build_games_*`, `test_backoff_delay_grows`, `handlers::tests::*`).

**Add:**
1. `page_url` smoke test — assert URL contains `$limit=50`, `$skip=…`, `$sort[createdAt]=-1`, and all 7 `$select[]=…` field names. Catches typos in the projection.
2. `pages(total)` — covers `0`, `1`, `50`, `51`, `1419`. Off-by-one insurance for the `total == 0` boot case.
3. `RefreshOutcome` → JSON mapping — locks the public HTTP contract without standing up an HTTP test.
4. (Optional) Boot timeout — if we extract `load_vods_with_timeout(client, dur)`, write a `tokio::time::pause` test that stubs `fetch_all_vods` with a `pending()` future and asserts the wrapper returns `vec![]` after 30s of virtual time. Skip if it forces awkward refactors.

**Deliberately not added:** live API hits in tests, mock HTTP server (`wiremock`/`httpmock`), 6h ticker timing tests.

**Manual verification checklist (pre-merge):**
- [ ] Fresh checkout (no `data/`) — `cargo run` → site loads in <5s, all VODs visible.
- [ ] `curl -XPOST localhost:3000/api/refresh` while idle → `{"status":"unchanged",...}`.
- [ ] Two rapid `curl -XPOST localhost:3000/api/refresh` → second returns `{"status":"busy"}`.
- [ ] `cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test` all clean.
- [ ] `git grep -i 'data/vods.json\|read_cache\|write_cache\|CACHE_PATH'` returns nothing in `src/`.
- [ ] `RUST_LOG=moonmoon=debug cargo run` shows 4 concurrent `GET … vods?$limit=50&$skip=…` log lines, not sequential.

## Alternatives considered

- **Cache removal only, leave `fetch_all_vods` sequential.** Rejected: ~17s cold boot is unacceptable once every boot is cold.
- **Per-request upstream queries (no in-memory cache).** Mirrors the original React SPA. Rejected: forces a handler rewrite, loses the cheap shared-cache property, and the games landing page has no upstream-native equivalent.
- **`anyhow` / `thiserror`.** Rejected: error chains are ~2 hops deep, all underlying errors are `reqwest::Error`, and the current convention already produces useful `tracing::error!` lines. Adding either library would be ceremony, not signal.
- **Best-effort fan-out (keep partial pages on failure).** Rejected: harder to reason about freshness and ordering than all-or-nothing; the next tick fixes it anyway.
- **`MAX_CONCURRENT_PAGES = 8`.** Probed safe but went with 4 to be conservative on upstream load.

## Open questions

None remaining.

## Touch surface summary

- `src/vods.rs` — heavy edit (the bulk of the change)
- `src/main.rs` — small additions (boot timeout + refresh ticker)
- `src/handlers/api.rs` — small simplification (delegate to `refresh_in_place`)
- `Cargo.toml` — possible single-line removal of `serde_json`
- `data/` — directory deletion + `.gitignore` cleanup
- `README.md`, `CLAUDE.md` — paragraph updates

## Performance expectation

- Cold boot fetch: ~17s → ~2–3s.
- Idle refresh tick (no new VODs): 1 cheap `$limit=1` request, ~400ms wall-clock, no swap.
- Refresh tick on new VODs: full fan-out, ~2–3s.
- Per-request handler latency: unchanged (in-memory `Arc<Vec<Vod>>` clone + filter).
