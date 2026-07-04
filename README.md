# moonmoon-rs

Live at **<https://moonmoon.gamma.observer>**.

A Rust port of [OP-Archives/MOONMOON-site](https://github.com/OP-Archives/MOONMOON-site) — a web app for browsing [MOONMOON](https://www.twitch.tv/moonmoon)'s archived streams by game, date, or calendar view. The upstream is a React + Vite SPA; this version is server-rendered with axum + Askama and uses htmx for partial updates. All VOD data comes from the same `archive.overpowered.tv` API the upstream uses.

## What it does

- **Landing home page** — launch hero with live archive stats, a Continue Watching shelf (client-side, from your resume state), an "On this day" callback, a recently-archived grid, and quick launchers into browse, calendar, and history.
- **Unified browse** — one `/browse` surface with a games/streams lens toggle: a grid of every game sorted by VOD count, or a flat list of every VOD, with full-text search, date range filter, and sort by newest / oldest / longest / shortest. Filtering streams by game splits the list into contiguous playing periods.
- **Calendar view** — weekly TV-guide timeline: one row per day with stream blocks laid out on a time axis, chapter segments color-coded by game that link straight to the right timestamp in the player, and a now-marker on today.
- **Player with resume** — picks up where you left off via `localStorage`; a progress bar overlays each thumbnail.
- **Up Next auto-continue** — when a VOD ends inside a game's playing period, an overlay offers the next VOD from that same period and auto-advances.
- **In-player game selector** — for multi-game streams, jump between chapters from a selector inside the player without leaving the page.
- **Watch history** — page listing streams saved in your resume state, entirely client-side unless optional sync is enabled.
- **Light/dark theme toggle** — stored locally per browser.
- **Cross-device sync (optional)** — generate a token in one browser, paste it in another, and your watch history follows you. Token is the only credential; no accounts, no email.
- **Synced chat replay with emotes** — chat comments scroll in time with the VOD, with alternating messages faintly shaded for readability. Twitch native emotes plus 7TV / BTTV / FFZ (global + channel sets) render inline, with hover tooltips. Emotes are resolved server-side, so the browser never calls the provider APIs directly. Newer VODs load a frozen per-VOD emote snapshot up front, so chat renders with the exact set (and stream-time ids) that was live during that stream.
- **Jump to a game inside a VOD** — if a stream covered multiple games, each chapter is a direct timestamped link.
- **Random VOD** — `/random` redirects to one at random.

## Stack

- **axum 0.8** — HTTP routing
- **Askama 0.15** — compile-time HTML templates
- **htmx** — pinned (`htmx.org@2.0.4`) and loaded from unpkg in `base.html`; drives pagination & search without a bundler
- **reqwest 0.12** — upstream API client (VODs, chat, emote providers)
- **tokio** — async runtime
- **tower_governor 0.8** — per-IP rate limiting
- No bundler, no frontend build step; browser JavaScript is checked with TypeScript (`tsc --noEmit`)

Rust edition 2024.

## Running

```sh
cargo run
```

Serves on `http://0.0.0.0:3000` (override with `PORT`). Every boot fetches the VOD catalog from `https://archive.overpowered.tv/api/v1/moonmoon/vods` (sequential paged fetch, 50 VODs per page), keeps the playable entries, then prefetches MOONMOON's channel + global emote sets from 7TV / BTTV / FFZ. If the catalog boot fetch fails or times out after 120 seconds, the app starts with an empty catalog and self-heals on a later refresh (the emote prefetch has its own 15-second budget). The background catalog task refreshes hourly — gated by a cheap upstream snapshot check (`total`, latest ID, and latest `updated_at`) so idle ticks cost one tiny request — and drops to a 60-second retry while the catalog is empty. A separate ticker re-fetches the emote sets every 24 hours.

```sh
PORT=8080 cargo run                          # custom port
RUST_LOG=moonmoon=debug cargo run            # verbose logs (default: moonmoon=info,tower_http=debug)
SYNC_STORE_PATH=/var/lib/moonmoon/sync.json cargo run  # sync store location (default ./sync.json)
cargo test                                   # unit tests
cargo clippy --all-targets -- -D warnings    # CI-equivalent Rust lints
cargo fmt                                    # format
bun test                                     # JS helper tests (static/lib/*)
bun run check:js                             # TypeScript check for browser JS
```

Most `/api/*` and `/history/{resume,vods}` routes are rate-limited (2 rps, burst 20) per smart-detected client IP via `tower_governor`. The high-volume `/api/emotes/*` lookups have their own more lenient bucket (20 rps, burst 120) so an emote burst on chat load can't 429 a viewer's sync/playback calls.

## Routes

| Path | Description |
| --- | --- |
| `/` | Landing home page (launch hero, archive stats, Continue Watching shelf, "On this day", recent grid) |
| `/browse` | Unified browse: `?lens=games\|streams`, plus `game`, search, date, and sort params |
| `/browse/grid` | htmx partial for the browse grid (paginated) |
| `/games`, `/streams` | 307 redirects into `/browse` with the matching lens |
| `/game/{name}` | 307 redirect into `/browse?lens=streams&game={name}` |
| `/watch/{vod_id}` | Player page |
| `/calendar` | Weekly TV-guide view (`?week=YYYY-MM-DD`) |
| `/history` | Watch history (client-side, reads `localStorage`) |
| `POST /history/vods` | Renders the history grid from a POSTed list of watched / in-progress VOD ids (the client's unified history store) |
| `/history/resume` | HTML fragment for one Continue Watching card (fetched per id by `continue-watching.js`) |
| `/random` | 307 to a random VOD |
| `/api/vod/{vod_id}` | VOD metadata as JSON |
| `/api/next/{vod_id}` | Next VOD in the same game's playing period (powers the Up Next overlay) |
| `/api/chat/{vod_id}` | Proxies upstream chat comments |
| `/api/emotes/channel` | Prefetched channel + global emote set as JSON |
| `/api/emotes/lookup/{name}` | On-demand cross-provider emote search (cached) |
| `/api/emotes/vod/{vod_id}` | Proxies the archive's frozen per-VOD emote snapshot (empty map for old VODs / misses) |
| `GET /api/sync/{token}` | Fetch a stored sync blob (404 if unknown) |
| `PUT /api/sync/{token}` | Replace the blob for `token` (256 KiB body cap) |
| `POST /api/refresh` | Force a catalog re-fetch (no-op if the upstream snapshot is unchanged) |

## Architecture

```
src/
├── main.rs              # AppState, Catalog generation, router, background tickers, rate limiters
├── vods/
│   ├── mod.rs           # Vod/Chapter/YoutubeVideo/VodDuration models, playable filtering, re-exports
│   ├── catalog.rs       # Upstream API client, load/refresh, CatalogSnapshot change detection
│   └── games.rs         # Game model, build_games/build_dominant_games, chapter color + date helpers
├── middleware.rs        # Per-request CSP nonce generation and header injection
├── sync_store.rs        # Token-keyed sync blob store, atomic JSON persistence
├── emotes/
│   ├── mod.rs           # EmoteProvider/EmoteRecord, EMOTE_REFRESH_INTERVAL
│   ├── fetch.rs         # Prefetch MOONMOON channel + global sets from 7TV/BTTV/FFZ
│   ├── store.rs         # EmoteIndex: prefetched map + resolved-lookup cache
│   └── parse.rs         # Normalize each provider's payload into EmoteRecord
└── handlers/
    ├── mod.rs           # Shared helpers: VodDisplay, pagination, filters, date/duration helpers
    ├── listing.rs       # Shared VOD listing pipeline (Listing::build): pagination + period/series headers
    ├── home.rs          # / landing page
    ├── browse.rs        # /browse + /browse/grid, 307 redirects from the legacy routes
    ├── watch.rs         # /watch/{id}, /random, /api/vod, /api/next
    ├── calendar.rs      # /calendar
    ├── history.rs       # /history, /history/vods, /history/resume
    ├── sync.rs          # /api/sync/{token} GET/PUT
    ├── emotes.rs        # /api/emotes/channel, /api/emotes/lookup/{name}, /api/emotes/vod/{id}
    └── api.rs           # /api/chat, /api/refresh

templates/               # Askama templates (compiled into the binary)
static/
├── player.js            # Player logic, chat sync, emotes, resume, Up Next overlay, game selector
├── sync.js              # Cross-device sync: token storage, pull/push, settings dialog
├── continue-watching.js # Hydrates the landing Continue Watching shelf from resume state
├── vod-cards.js         # Resume/watched badges and chapter popovers on VOD cards
├── header.js, history.js, list-filters.js, list-feedback.js
├── lib/                 # Pure JS helpers covered by bun test (emote client/cache/heuristic,
│                        #   history state/sort, storage/token, player parts, chat autoscroll/stripe/timestamps, …)
├── types.d.ts           # Ambient browser/API types for TypeScript checking
└── css/                 # Split per concern: base, header, landing, browse, games, vods, calendar, player, sync, footer
```

### State

The router's shared state is `AppState { catalog, http_client, refresh_lock, sync_store, emotes }`. The catalog lives behind a single `RwLock<Arc<Catalog>>`, where a `Catalog` is one immutable generation — its `vods` plus everything derived from them (`games`, the `CatalogSnapshot` used for change detection, and the precomputed archive `date_bounds`), all built together by `Catalog::build`. A reader just `Arc::clone`s the current generation and drops the guard before rendering, so the guard never crosses an `.await` and a reader can never observe vods from one refresh paired with games or bounds from another. The only writer is `vods::refresh_in_place`, which swaps the whole `Arc<Catalog>` atomically; `refresh_lock` (a `Mutex<()>`) serializes refreshes so concurrent ticks can't stack. Emotes sit behind their own `RwLock<Arc<EmoteIndex>>`, swapped by the 24-hour emote ticker.

### Templates

List views pair a full-page template (e.g. `browse.html`) with grid-only partials (`games_grid.html`, `vods_grid.html`) built from shared card partials (`game_card.html`, `vod_card.html`) and control includes (`list_filters.html`, `_sort_control.html`, `_footer.html`, `continue_resume.html`, `continue_watching_block.html`). htmx swaps selected full-page result regions for filters and grid-only partials for pagination. Watch state lives in `localStorage` as a single unified history store (`moonmoon_history`; older `moonmoon_resume` / `moonmoon_watched` stores are migrated into it on first read) and is reapplied to VOD cards after every `htmx:afterSwap` and on the `moonmoon:historyChanged` event; player-only preferences use additional local keys such as `moonmoon_part_durations`, `moonmoon_chat_size`, `moonmoon_chat_timestamps`, `moonmoon_theatre`, and `moonmoon_history_sort`. The whole history contract - store shape, normalization, legacy migration, merging, the sync-blob shape, and the resume-noise threshold - lives in `static/lib/history-state.js`; `player.js`, `sync.js`, `history.js`, and `vod-cards.js` are thin adapters over it. All localStorage access goes through the guarded helpers in `static/lib/storage.js`, which degrade to no-ops in storage-blocking browsers. Templates are compiled into the binary by Askama, so edits to `templates/*.html` require a rebuild.

### Emotes

Emote resolution is server-side so the client never hits provider APIs directly. At boot (and every 24 hours after, via `EMOTE_REFRESH_INTERVAL`) `emotes::fetch` prefetches MOONMOON's channel and global sets from 7TV, BTTV, and FFZ into an `EmoteIndex`; `/api/emotes/channel` serves that map. Names the prefetch didn't cover are resolved on demand by `/api/emotes/lookup/{name}`, which searches all three providers concurrently and caches the result (hit *or* clean miss) in the index. `valid_emote_name` gates input (3–25 chars, alphanumeric + `_`) before any provider is called, and `EmoteProvider` serializes as the canonical `7TV` / `BTTV` / `FFZ` strings.

Newer VODs also carry a frozen per-VOD snapshot — the exact 7TV / BTTV / FFZ set that was live during that stream. `/api/emotes/vod/{vod_id}` (charset-gated like `chat_proxy`, then resolved against the in-memory catalog) proxies the archive's `/vods/{id}/emotes` and normalizes it with `parse::parse_vod_emote_snapshot`, which builds CDN URLs directly from each emote id and absorbs 7TV first so it wins any cross-provider name collision. Unknown ids, upstream 404s (older VODs predate snapshot capture, roughly id < 310), and transport errors all return an empty map (`cache-control` 300s), so the client falls back cleanly to the prefetch + live lookup above. On the player page `loadVodEmotes` (`static/player.js`) fetches the snapshot up front and lets its entries overwrite prefetched channel emotes (snapshot-wins), so since-removed / collision-prone emotes render with their stream-time ids.

### Cross-device sync

Click the ⟳ icon in the top-right of any page to open the sync dialog.

- **Generate new token** creates a 26-character base32 token, stores it in this device's `localStorage`, and immediately uploads your current watch history.
- **Use existing token** lets a second device paste the same token and pull the history down. After that, both devices push debounced updates whenever `moonmoon_history` changes.
- **Disconnect this device** removes the token from this browser. Your local history stays put; the remote copy is untouched.

The token is the only credential — anyone holding it can read and overwrite the history. Treat it like a password. Tokens (26–32 char uppercase base32) never expire on the server. The server stores opaque JSON blobs at `$SYNC_STORE_PATH` (default `./sync.json`, gitignored), behind the existing API rate limiter (2 req/s, burst 20, per IP), with bodies capped at 256 KiB. Whole-blob conflict resolution is last-write-wins by the client-supplied `updated_at`; the server stamps its own `stored_at` and uses it as the eviction key, capping the store at 10,000 tokens (oldest evicted first). The blob itself is a versioned `{ v: 2, history: { <id>: entry } }` payload (reads still accept the old `{ resume, watched }` split shape), and per-VOD merging is the client's job (`static/lib/history-state.js`), so two devices watching different VODs won't clobber each other.

### Upstream quirks

- `duration` currently arrives as numeric seconds. `VodDuration` preserves those exact seconds for calculations while still exposing a compact display string; string-format fallback remains for older payloads.
- The catalog filters out live rows and rows without VOD uploads, while keeping the raw upstream snapshot for refresh comparisons.
- VOD-level thumbnails may be absent; `backfill_thumbnails` lifts the first upload thumbnail onto `Vod.thumbnail_url`.
- Chapter images are upscaled by replacing `{width}x{height}` or `40x53` → `285x380` in the URL.
- The upstream API rate-limits aggressively; `fetch_api_response` retries 429s with numeric `Retry-After` support, exponential backoff, and jitter. Catalog loads are all-or-nothing per page: a single malformed VOD row is skipped with a warning, but a page where every row fails aborts the refresh rather than swapping in a degraded catalog.
- Many fields on `Vod`/`Chapter` are `Option` because the REST payload is nullable/sparse across historical rows; deserializers also retain compatibility with older, less consistent payload shapes.

## Conventions

- No `anyhow` / `thiserror` — data-layer functions return `Result<_, reqwest::Error>` or degrade gracefully; handlers log and return HTTP errors inline.
- `vod_id` is sanitized (alphanumeric + `-_` only) before being interpolated into outbound URLs. See `chat_proxy` for the reference pattern.
- Keep handler bodies thin; push filtering, sorting, pagination, and URL-building into `handlers/mod.rs` helpers.
- Prefer `Arc::clone`ing the current `Catalog` out of the `RwLock` guard over holding the guard across `.await`.

## Credits

- **[OP-Archives/MOONMOON-site](https://github.com/OP-Archives/MOONMOON-site)** — the original React SPA this project is ported from. The API endpoints, URL shapes, and the `40x53` → `285x380` thumbnail-upscaling trick were all learned from reading their source.
- **archive.overpowered.tv** — the VOD archive and API this app reads from at runtime. All VOD metadata, thumbnails, chat, and video streams are served by them; this project is just a viewer.
- **MOONMOON** ([twitch.tv/moonmoon](https://www.twitch.tv/moonmoon)) — the streamer whose VODs this app browses.

Not affiliated with or endorsed by MOONMOON, OP-Archives, or overpowered.tv.

## License

[MIT](LICENSE)
