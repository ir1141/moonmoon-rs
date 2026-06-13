# moonmoon-rs

Live at **<https://moonmoon.gamma.observer>**.

A Rust port of [OP-Archives/MOONMOON-site](https://github.com/OP-Archives/MOONMOON-site) — a web app for browsing [MOONMOON](https://www.twitch.tv/moonmoon)'s archived streams by game, date, or calendar view. The upstream is a React + Vite SPA; this version is server-rendered with axum + Askama and uses htmx for partial updates. All VOD data comes from the same `archive.overpowered.tv` API the upstream uses.

## What it does

- **Landing home page** — hero with a Continue Watching shelf (client-side, from your resume state) and quick launchers into browse, calendar, and history.
- **Unified browse** — one `/browse` surface with a games/streams lens toggle: grid of every game sorted by VOD count, or a flat list of every VOD, with full-text search, date range filter, and sort by newest / oldest / longest / shortest. Filtering streams by game splits the list into contiguous playing periods.
- **Calendar view** — weekly TV-guide timeline: one row per day with stream blocks laid out on a time axis, chapter segments color-coded by game that link straight to the right timestamp in the player, and a now-marker on today.
- **Player with resume** — picks up where you left off via `localStorage`; a progress bar overlays each thumbnail.
- **Up Next auto-continue** — when a VOD ends inside a game's playing period, an overlay offers the next VOD from that same period and auto-advances.
- **Watch history** — page listing streams saved in your resume state, entirely client-side unless optional sync is enabled.
- **Light/dark theme toggle** — stored locally per browser.
- **Cross-device sync (optional)** — generate a token in one browser, paste it in another, and your watch history follows you. Token is the only credential; no accounts, no email.
- **Synced chat replay with emotes** — chat comments scroll in time with the VOD. Twitch native emotes plus 7TV / BTTV / FFZ (global + channel sets) render inline, with hover tooltips.
- **Jump to a game inside a VOD** — if a stream covered multiple games, each chapter is a direct timestamped link.
- **Random VOD** — `/random` redirects to one at random.

## Stack

- **axum 0.8** — HTTP routing
- **Askama 0.15** — compile-time HTML templates
- **htmx** — loaded from unpkg in `base.html`; drives pagination & search without a bundler
- **reqwest** — upstream API client
- **tokio** — async runtime
- No bundler, no frontend build step; browser JavaScript is checked with TypeScript

Rust edition 2024.

## Running

```sh
cargo run
```

Serves on `http://0.0.0.0:3000` (override with `PORT`). Every boot fetches the VOD catalog from `https://archive.overpowered.tv/api/v1/moonmoon/vods` (sequential paged fetch, 50 VODs per page), then keeps playable entries. If boot fetch fails or times out after 30 seconds, the app starts with an empty catalog and can recover on a later refresh. A background task refreshes every 6 hours, gated by a cheap upstream snapshot check (`total`, latest ID, and latest `updated_at`) so idle ticks cost one tiny request.

```sh
PORT=8080 cargo run                          # custom port
RUST_LOG=moonmoon=debug cargo run            # verbose logs
SYNC_STORE_PATH=/var/lib/moonmoon/sync.json cargo run  # sync store location (default ./sync.json)
cargo test                                   # unit tests
cargo clippy --all-targets -- -D warnings    # CI-equivalent Rust lints
bun test                                     # JS helper tests
bun run check:js                             # TypeScript check for browser JS
```

`/api/*` and `/history/{resume,vods}` routes are rate-limited (2 rps, burst 20) per smart-detected client IP via `tower_governor`.

## Routes

| Path | Description |
| --- | --- |
| `/` | Landing home page (hero, Continue Watching shelf, launchers) |
| `/browse` | Unified browse: `?lens=games\|streams`, plus `game`, search, date, and sort params |
| `/browse/grid` | htmx partial for the browse grid (paginated) |
| `/games`, `/streams` | 307 redirects into `/browse` with the matching lens |
| `/game/{name}` | 307 redirect into `/browse?lens=streams&game={name}` |
| `/watch/{vod_id}` | Player page |
| `/calendar` | Weekly TV-guide view (`?week=YYYY-MM-DD`) |
| `/history` | Watch history (client-side, reads `localStorage`) |
| `/history/vods` | htmx partial for the history grid |
| `/history/resume` | htmx partial for the Continue Watching shelf |
| `/random` | 302 to a random VOD |
| `/api/vod/{vod_id}` | VOD metadata as JSON |
| `/api/next/{vod_id}` | Next VOD in the same game's playing period (powers the Up Next overlay) |
| `/api/chat/{vod_id}` | Proxies upstream chat comments |
| `GET /api/sync/{token}` | Fetch a stored sync blob (404 if unknown) |
| `PUT /api/sync/{token}` | Replace the blob for `token` (256 KiB body cap) |
| `POST /api/refresh` | Force a catalog re-fetch (no-op if the upstream snapshot is unchanged) |

## Architecture

```
src/
├── main.rs              # AppState, router, tracing
├── vods.rs              # Upstream API client, cache, Vod/Game models
├── middleware.rs        # CSP nonce generation and header injection
├── sync_store.rs        # In-memory sync blob store, atomic JSON persistence
└── handlers/
    ├── mod.rs           # Shared helpers: VodDisplay, pagination, filters, date helpers
    ├── home.rs          # / landing page
    ├── browse.rs        # /browse + /browse/grid, redirects from the legacy routes
    ├── watch.rs         # /watch/{id}, /random, /api/vod, /api/next
    ├── calendar.rs      # /calendar
    ├── history.rs       # /history, /history/vods, /history/resume
    ├── sync.rs          # /api/sync/{token} GET/PUT
    └── api.rs           # /api/chat, /api/refresh

templates/               # Askama templates (compiled into the binary)
static/
├── player.js            # Player logic, chat sync, emotes, resume, Up Next overlay
├── sync.js              # Cross-device sync: token storage, pull/push, settings dialog
├── continue-watching.js # Hydrates the landing Continue Watching shelf from resume state
├── vod-cards.js         # Resume/watched badges and chapter popovers on VOD cards
├── header.js, history.js, list-filters.js, list-feedback.js
├── lib/                 # Pure JS helpers covered by bun test (incl. storage.js localStorage guards)
├── types.d.ts           # Ambient browser/API types for TypeScript checking
└── css/                 # Split per concern: base, header, landing, browse, games, vods, calendar, player, sync, footer
```

### State

`AppState { vods, games, catalog_snapshot, date_bounds, http_client, refresh_lock, sync_store }` holds the catalog behind four `tokio::sync::RwLock`s (VODs, games, upstream snapshot, and precomputed archive date bounds) and owns the optional sync store. Handlers take a read lock just long enough to clone the cheap `Arc` and drop the guard before rendering — the guard never crosses an `.await`, and no reader ever holds two of the catalog guards at once (the refresh writer takes all four in a fixed order, so that discipline is what keeps it deadlock-free). The only writer for catalog data is the 6-hour background refresh task, which swaps all four together. `refresh_lock` serializes refreshes so concurrent ticks can't stomp each other.

### Templates

List views pair a full-page template (e.g. `browse.html`) with grid-only partials (`games_grid.html`, `vods_grid.html`) built from shared card partials (`game_card.html`, `vod_card.html`). htmx swaps selected full-page result regions for filters and grid-only partials for pagination. Watch state lives in `localStorage` (`moonmoon_resume`, `moonmoon_watched`) and is reapplied after every `htmx:afterSwap`; player-only preferences use additional local keys such as `moonmoon_part_durations`, `moonmoon_chat_size`, and `moonmoon_theatre`. All localStorage access goes through the guarded helpers in `static/lib/storage.js`, which degrade to no-ops in storage-blocking browsers. Templates are compiled into the binary by Askama, so edits to `templates/*.html` require a rebuild.

### Cross-device sync

Click the ⟳ icon in the top-right of any page to open the sync dialog.

- **Generate new token** creates a 26-character base32 token, stores it in this device's `localStorage`, and immediately uploads your current watch history.
- **Use existing token** lets a second device paste the same token and pull the history down. After that, both devices push debounced updates whenever `moonmoon_resume` changes.
- **Disconnect this device** removes the token from this browser. Your local history stays put; the remote copy is untouched.

The token is the only credential — anyone holding it can read and overwrite the history. Treat it like a password. Tokens never expire on the server. Server stores blobs (opaque JSON) at `$SYNC_STORE_PATH` (default `./sync.json`, gitignored). The endpoint pair is hosted under the existing API rate limiter (2 req/s, burst 20, per IP) and bodies are capped at 256 KiB. Per-VOD merge logic is last-write-wins by `updated` timestamp, so two devices watching the same VOD at the same time won't fully clobber each other.

### Upstream quirks

- `duration` currently arrives as numeric seconds. `VodDuration` preserves those exact seconds for calculations while still exposing a compact display string; string-format fallback remains for older payloads.
- The catalog filters out live rows and rows without VOD uploads, while keeping the raw upstream snapshot for refresh comparisons.
- VOD-level thumbnails may be absent; `backfill_thumbnails` lifts the first upload thumbnail onto `Vod.thumbnail_url`.
- Chapter images are upscaled by replacing `{width}x{height}` or `40x53` → `285x380` in the URL.
- The upstream API rate-limits aggressively; `fetch_api_response` retries 429s with numeric `Retry-After` support, exponential backoff, and jitter.
- Many fields on `Vod`/`Chapter` are `Option` because the REST payload is nullable/sparse across historical rows; deserializers also retain compatibility with older, less consistent payload shapes.

## Conventions

- No `anyhow` / `thiserror` — data-layer functions return `Result<_, reqwest::Error>` or degrade gracefully; handlers log and return HTTP errors inline.
- `vod_id` is sanitized (alphanumeric + `-_` only) before being interpolated into outbound URLs. See `chat_proxy` for the reference pattern.
- Keep handler bodies thin; push filtering, sorting, pagination, and URL-building into `handlers/mod.rs` helpers.
- Prefer cloning `Arc<Vec<Vod>>` out of the `RwLock` guard over holding the guard across `.await`.

## Credits

- **[OP-Archives/MOONMOON-site](https://github.com/OP-Archives/MOONMOON-site)** — the original React SPA this project is ported from. The API endpoints, URL shapes, and the `40x53` → `285x380` thumbnail-upscaling trick were all learned from reading their source.
- **archive.overpowered.tv** — the VOD archive and API this app reads from at runtime. All VOD metadata, thumbnails, chat, and video streams are served by them; this project is just a viewer.
- **MOONMOON** ([twitch.tv/moonmoon](https://www.twitch.tv/moonmoon)) — the streamer whose VODs this app browses.

Not affiliated with or endorsed by MOONMOON, OP-Archives, or overpowered.tv.

## License

[MIT](LICENSE)
