# moonmoon-rs

A Rust rewrite of the [moonmoon](../) VOD browser — a small web app for exploring MOONMOON's archived streams by game, date, or calendar view. The original is a vanilla-JS single-page app; this version is server-rendered with axum + Askama and uses htmx for partial updates.

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
| `/game/{name}` | VODs for a single game |
| `/streams` | All VODs |
| `/watch/{vod_id}` | Player page |
| `/calendar` | Monthly calendar view |
| `/history` | Watch history (client-side, reads `localStorage`) |
| `/random` | 302 to a random VOD |
| `/api/vod/{vod_id}` | VOD metadata as JSON |
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
    ├── watch.rs         # /watch/{id}, /random
    ├── calendar.rs      # /calendar
    ├── history.rs       # /history
    └── api.rs           # /api/chat, /api/refresh

templates/               # Askama templates (compiled into the binary)
static/                  # player.js + assets served by ServeDir
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
