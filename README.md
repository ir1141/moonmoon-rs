# moonmoon-rs

Live at **<https://moonmoon.gamma.observer>**.

A Rust port of [OP-Archives/MOONMOON-site](https://github.com/OP-Archives/MOONMOON-site) — a web app for browsing [MOONMOON](https://www.twitch.tv/moonmoon)'s archived streams by game, date, or calendar view. The upstream is a React + Vite SPA; this version is server-rendered with axum + Askama and uses htmx for partial updates. All VOD data comes from the same `archive.overpowered.tv` API the upstream uses.

## What it does

- **Browse by game** — grid of every game MOONMOON has streamed, sorted by VOD count, with search and alt sort orders.
- **Browse all streams** — flat list of every VOD with full-text search, date range filter, and sort by newest / oldest / longest / shortest.
- **Calendar view** — month-at-a-glance grid showing which days have streams.
- **Game pages grouped by playing period** — per-game VOD lists are split into contiguous playing periods so long-running games read as distinct seasons instead of one flat stream.
- **Player with resume** — picks up where you left off via `localStorage`; a progress bar overlays each thumbnail.
- **Up Next auto-continue** — when a VOD ends inside a game's playing period, an overlay offers the next VOD from that same period and auto-advances.
- **Watch history** — page listing everything you've started or finished, entirely client-side (no account, no server state).
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
- No bundler, no frontend build step

Rust edition 2024.

## Running

```sh
cargo run
```

Serves on `http://0.0.0.0:3000` (override with `PORT`). Every boot fetches the full VOD catalog from `https://archive.overpowered.tv/moonmoon/vods` (sequential paged fetch, 50 VODs per page). A background task refreshes every 6 hours, gated by a cheap upstream `total`-changed check so idle ticks cost one tiny request.

```sh
PORT=8080 cargo run                          # custom port
RUST_LOG=moonmoon=debug cargo run            # verbose logs
SYNC_STORE_PATH=/var/lib/moonmoon/sync.json cargo run  # sync store location (default ./sync.json)
cargo test                                   # unit tests
cargo clippy                                 # lints
```

`/api/*` routes are rate-limited (2 rps, burst 20) per smart-detected client IP via `tower_governor`.

## Routes

| Path | Description |
| --- | --- |
| `/` | Games grid (default landing) |
| `/games` | htmx partial for the games grid (paginated) |
| `/game/{name}` | VODs for a single game, grouped by playing period |
| `/game/{name}/vods` | htmx partial for a game's VOD grid |
| `/streams` | All VODs |
| `/streams/vods` | htmx partial for the all-streams grid |
| `/watch/{vod_id}` | Player page |
| `/calendar` | Monthly calendar view |
| `/history` | Watch history (client-side, reads `localStorage`) |
| `/history/vods` | htmx partial for the history grid |
| `/random` | 302 to a random VOD |
| `/api/vod/{vod_id}` | VOD metadata as JSON |
| `/api/next/{vod_id}` | Next VOD in the same game's playing period (powers the Up Next overlay) |
| `/api/chat/{vod_id}` | Proxies upstream chat comments |
| `GET /api/sync/{token}` | Fetch a stored sync blob (404 if unknown) |
| `PUT /api/sync/{token}` | Replace the blob for `token` (256 KiB body cap) |

## Architecture

```
src/
├── main.rs              # AppState, router, tracing
├── vods.rs              # Upstream API client, cache, Vod/Game models
├── sync_store.rs        # In-memory sync blob store, atomic JSON persistence
└── handlers/
    ├── mod.rs           # Shared helpers: VodDisplay, pagination, filters, date/duration parsing
    ├── games.rs         # /, /games
    ├── vods.rs          # /game/{name}, /streams
    ├── watch.rs         # /watch/{id}, /random, /api/vod, /api/next
    ├── calendar.rs      # /calendar
    ├── history.rs       # /history
    ├── sync.rs          # /api/sync/{token} GET/PUT
    └── api.rs           # /api/chat

templates/               # Askama templates (compiled into the binary)
static/
├── player.js            # Player logic, chat sync, emotes, resume, Up Next overlay
├── sync.js              # Cross-device sync: token storage, pull/push, settings dialog
└── css/                 # Split per concern: base, header, games, vods, calendar, player, sync
```

### State

`AppState { vods, games, http_client, refresh_lock }` holds both datasets behind `tokio::sync::RwLock<Arc<Vec<…>>>`. Handlers take a read lock just long enough to clone the cheap `Arc` and drop the guard before rendering — the guard never crosses an `.await`. The only writer is the 6-hour background refresh task, which atomically swaps both `Arc`s. `refresh_lock` serializes refreshes so concurrent ticks can't stomp each other.

### Templates

Each list view has two templates: a full-page one (e.g. `games.html`) and a grid-only partial (`games_grid.html`) that htmx swaps in for pagination and search. Resume/watched state lives in `localStorage` (`moonmoon_resume`, `moonmoon_watched`) and is reapplied after every `htmx:afterSwap`. Templates are compiled into the binary by Askama, so edits to `templates/*.html` require a rebuild.

### Cross-device sync

Click the ⟳ icon in the top-right of any page to open the sync dialog.

- **Generate new token** creates a 26-character base32 token, stores it in this device's `localStorage`, and immediately uploads your current watch history.
- **Use existing token** lets a second device paste the same token and pull the history down. After that, both devices push debounced updates whenever `moonmoon_resume` changes.
- **Disconnect this device** removes the token from this browser. Your local history stays put; the remote copy is untouched.

The token is the only credential — anyone holding it can read and overwrite the history. Treat it like a password. Tokens never expire on the server. Server stores blobs (opaque JSON) at `$SYNC_STORE_PATH` (default `./sync.json`, gitignored). The endpoint pair is hosted under the existing API rate limiter (2 req/s, burst 20, per IP) and bodies are capped at 256 KiB. Per-VOD merge logic is last-write-wins by `updated` timestamp, so two devices watching the same VOD at the same time won't fully clobber each other.

### Upstream quirks

- `duration` comes in two formats: `"3h 20m"` and `"HH:MM:SS"`. `parse_duration_minutes` / `parse_duration_seconds` in `handlers/mod.rs` handle both.
- Chapter images are upscaled by replacing `40x53` → `285x380` in the URL.
- The upstream API rate-limits aggressively; `fetch_api_response` retries 429s with exponential backoff (250ms → 500ms → 1000ms).
- Many fields on `Vod`/`Chapter` are `Option` because the upstream is inconsistent.

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

