# moonmoon-rs

A Rust port of [OP-Archives/MOONMOON-site](https://github.com/OP-Archives/MOONMOON-site) — a small web app for browsing [MOONMOON](https://www.twitch.tv/moonmoon)'s archived streams by game, date, or calendar view. The upstream is a React + Vite SPA; this version is server-rendered with axum + Askama and uses htmx for partial updates. All VOD data comes from the same `archive.overpowered.tv` API the upstream uses.

## What it does

- **Browse by game** — landing page is a grid of every game MOONMOON has streamed, sorted by VOD count, with search and alt sort orders.
- **Browse all streams** — flat list of every VOD with full-text search, date range filter, and sort by newest / oldest / longest / shortest.
- **Calendar view** — month-at-a-glance grid showing which days have streams.
- **Game pages grouped by playing period** — per-game VOD lists are split into contiguous playing periods so long-running games read as distinct seasons instead of one flat stream.
- **Persistent top nav** — every page shares a header with the active section highlighted.
- **Player with resume** — picks up where you left off via `localStorage`; a progress bar overlays each thumbnail so you can see what you've already watched.
- **Up Next auto-continue** — when a VOD ends inside a game's playing period, an overlay offers the next VOD from that same period and auto-advances.
- **Watch history** — a dedicated page listing everything you've started or finished, entirely client-side (no account, no server state).
- **Synced chat replay with full emote support** — the player fetches upstream chat comments and scrolls them in time with the VOD. Twitch native emotes plus third-party emotes from **7TV**, **BTTV**, and **FFZ** are rendered inline (both global sets and MOONMOON's channel sets), with hover tooltips showing the emote name and provider.
- **Jump to a game inside a VOD** — if a stream covered multiple games, each chapter is a direct timestamped link into the player.
- **Random VOD** — `/random` sends you to one at random.

Everything is server-rendered with htmx partials for pagination and search, so it's fast on cold loads and works without JavaScript for the read-only views.

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

Serves on `http://0.0.0.0:3000`. On first launch it fetches every VOD from `https://archive.overpowered.tv/moonmoon/vods` and writes `data/vods.json`. Subsequent launches reuse that cache if it's younger than 24 hours.

```sh
RUST_LOG=moonmoon=debug cargo run   # verbose logs
cargo test                          # unit tests
cargo clippy                        # lints
```

Force a refresh without restarting:

```sh
curl -XPOST localhost:3000/api/refresh
```

It cheaply re-checks the upstream `total` first and no-ops if nothing changed.

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
| `POST /api/refresh` | Re-fetches from upstream |

## Architecture

```
src/
├── main.rs              # AppState, router, tracing
├── vods.rs              # Upstream API client, cache, Vod/Game models
└── handlers/
    ├── mod.rs           # Shared helpers: VodDisplay, pagination, filters, date/duration parsing
    ├── games.rs         # /, /games
    ├── vods.rs          # /game/{name}, /streams
    ├── watch.rs         # /watch/{id}, /random, /api/vod, /api/next
    ├── calendar.rs      # /calendar
    ├── history.rs       # /history
    └── api.rs           # /api/chat, /api/refresh

templates/               # Askama templates (compiled into the binary)
static/
├── player.js            # Player logic, chat sync, emotes, resume, Up Next overlay
└── css/                 # Split per concern: base, header, games, vods, calendar, player
data/vods.json           # Cached upstream payload (gitignored)
```

### State

`AppState { vods, games, http_client }` holds both datasets behind `tokio::sync::RwLock<Arc<Vec<…>>>`. Handlers take a read lock just long enough to clone the cheap `Arc` and drop the guard before rendering — the guard never crosses an `.await`. The only writer is `/api/refresh`, which atomically swaps both `Arc`s.

### Templates

Each list view has two templates: a full-page one (e.g. `games.html`) and a grid-only partial (`games_grid.html`) that htmx swaps in for pagination and search. Resume/watched state lives in `localStorage` (`moonmoon_resume`, `moonmoon_watched`) and is reapplied after every `htmx:afterSwap`.

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

## No hot reload

Templates are compiled into the binary by Askama, so edits to `templates/*.html` require `cargo run` again.

## Credits

- **[OP-Archives/MOONMOON-site](https://github.com/OP-Archives/MOONMOON-site)** — the original React SPA this project is ported from. The API endpoints, URL shapes, and the `40x53` → `285x380` thumbnail-upscaling trick were all learned from reading their source.
- **archive.overpowered.tv** — the VOD archive and API this app reads from at runtime. All VOD metadata, thumbnails, chat, and video streams are served by them; this project is just a viewer.
- **MOONMOON** ([twitch.tv/moonmoon](https://www.twitch.tv/moonmoon)) — the streamer whose VODs this app browses.

Not affiliated with or endorsed by MOONMOON, OP-Archives, or overpowered.tv.

## License

[MIT](LICENSE)

