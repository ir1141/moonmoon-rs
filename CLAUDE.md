# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What This Is

A Rust rewrite of the `moonmoon` VOD browser. Server-rendered via axum + Askama templates, with htmx driving partial updates. The parent `moonmoon/` directory contains the original vanilla JS SPA — see `../CLAUDE.md` for the legacy context; this Rust version is the active architecture.

## Commands

- `cargo run` — starts the server on `http://0.0.0.0:3000`. On first launch it fetches all VODs from `https://archive.overpowered.tv/moonmoon/vods` and writes `data/vods.json`; subsequent launches load from that cache if younger than 24h.
- `cargo test` — runs unit tests (currently in `src/vods.rs` and `src/handlers/mod.rs`). No integration tests yet.
- `cargo test <name>` — single test, e.g. `cargo test test_parse_duration_minutes`.
- `cargo clippy` / `cargo build` — standard.
- `RUST_LOG=moonmoon=debug cargo run` — verbose logs. Default filter is `moonmoon=info,tower_http=debug`.
- `curl -XPOST localhost:3000/api/refresh` — force re-fetch. It first checks remote `total` cheaply; if unchanged, it does nothing.

There is no dev server / hot reload. Templates are compiled into the binary by Askama, so template edits require `cargo run` again.

## Architecture

### Startup & State

`main.rs` loads VODs once at boot into `AppState { vods, games, http_client }` behind `tokio::sync::RwLock<Arc<Vec<…>>>`. Handlers `.read().await` to get a cheap `Arc` clone; the only writer is `/api/refresh` which swaps both `Arc`s atomically. This means handlers never re-parse or re-fetch — the entire dataset lives in memory.

### Data layer (`src/vods.rs`)

- `Vod` / `Chapter` / `YoutubeVideo` mirror the upstream API shape. Many fields are `Option` because the upstream is inconsistent.
- `load_vods()` → checks `data/vods.json` cache (`CACHE_MAX_AGE_SECS = 86400`), falls back to `fetch_all_vods()` which paginates the `archive.overpowered.tv` API at 50/page with a 200ms sleep between pages.
- `build_games()` folds all chapter names across all VODs into a deduped `Vec<Game>`, case-insensitive by key but preserving first-seen casing for display. Chapter images are upscaled by replacing `40x53` in the URL with `285x380`.
- Failures during initial fetch are logged and degrade to an empty VOD list rather than panicking.

### Handlers (`src/handlers/`)

Handlers are split by concern and re-exported from `mod.rs`:

- `games.rs` — `/` (full page) and `/games` (htmx grid partial, paginated via `GAME_BATCH_SIZE = 60`).
- `vods.rs` — `/game/{name}` + `/game/{name}/vods`, `/streams` + `/streams/vods`. Uses `VOD_BATCH_SIZE = 36`.
- `watch.rs` — `/watch/{vod_id}` (player page), `/api/vod/{vod_id}` (JSON), `/random` (302 to a random VOD).
- `calendar.rs` — `/calendar` calendar view.
- `history.rs` — `/history` + `/history/vods` (reads `moonmoon_resume` / `moonmoon_watched` from localStorage client-side, so the server just renders all VODs and htmx/JS filters).
- `api.rs` — `/api/chat/{vod_id}` (proxies `archive.overpowered.tv/.../comments`, validates `vod_id` as alphanumeric + `-_`) and `POST /api/refresh`.

Shared helpers live in `handlers/mod.rs`: `VodDisplay::from_vod`, `filter_games`, `filter_vod_displays`, `paginate`, `get_chapter_start`, `build_*_next_url`, date/duration parsing, and `render_template` (the error-to-500 wrapper all handlers should use).

### Templates (`templates/*.html`)

Askama (file-based, inheritance via `base.html`). Each list view has a full-page template (e.g. `games.html`) and a grid-only partial (`games_grid.html`) that htmx swaps in for pagination & search. When adding a list view, follow the same two-template pattern and hook it up in `main.rs` with both `/page` and `/page/grid`-style routes.

htmx is loaded from unpkg in `base.html`; there is no bundler. The small bits of JS in `base.html` and `static/player.js` are hand-written and talk to `localStorage` (`moonmoon_resume`, `moonmoon_watched`). Resume progress bars are applied after every `htmx:afterSwap`.

### Duration parsing

Upstream `duration` comes in two formats: `"3h 20m"` and `"HH:MM:SS"`. `parse_duration_minutes` / `parse_duration_seconds` in `handlers/mod.rs` handle both. If you touch sorting-by-length or timestamp math, use these — tests in `handlers::tests` lock in the expected outputs.

### Conventions

- Rust edition 2024.
- No `anyhow`/`thiserror` — handlers log and return HTTP errors inline, data-layer errors return `Result<_, reqwest::Error>` or degrade gracefully.
- `vod_id` must be sanitized before being put into outbound URLs (`chat_proxy` is the reference).
- Prefer cloning out of the `RwLock` guard (`Arc<Vec<Vod>>` is cheap) over holding the guard across `.await` boundaries.
- Keep handler bodies thin — push filtering, sorting, pagination, and URL-building into `handlers/mod.rs` helpers so the template structs stay simple.
