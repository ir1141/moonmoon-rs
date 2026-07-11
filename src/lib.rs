//! Server-rendered VOD browser for the MOONMOON archive (axum + Askama + htmx).
//!
//! [`run`] is the binary entry point: it boots the VOD catalog and emote
//! caches, spawns the background refresh tickers, and serves the app.
//! [`build_router`] is split out so integration tests can drive the full
//! route table (rate limiting, CSP, and tracing included) without a socket.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::{
    Router,
    routing::{get, post},
};
use tokio::sync::RwLock;
use tower_governor::{
    GovernorLayer, governor::GovernorConfigBuilder, key_extractor::SmartIpKeyExtractor,
};
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;

mod dates;
mod emotes;
mod handlers;
mod middleware;
mod sync_store;
mod vods;

pub use emotes::EmoteIndex;
pub use sync_store::SyncStore;
pub use vods::CatalogLoad;

/// One immutable catalog generation: the vods plus everything derived from
/// them. Swapped atomically behind `AppState::catalog`, so a reader can never
/// observe vods from one refresh paired with games or date bounds from
/// another, and there is no lock-ordering discipline to uphold.
pub struct Catalog {
    pub vods: Vec<vods::Vod>,
    pub games: Vec<vods::Game>,
    pub(crate) snapshot: vods::CatalogSnapshot,
    /// (min, max) `YYYY-MM-DD` stream dates across the catalog; only changes
    /// on refresh, so it's computed once per catalog swap instead of per
    /// request.
    pub date_bounds: (String, String),
}

impl Catalog {
    /// The only way to make a `Catalog` — keeps `games` and `date_bounds`
    /// derived from the same `vods` they're stored with.
    pub fn build(load: CatalogLoad) -> Self {
        let games = vods::build_games(&load.vods);
        let date_bounds = vods::archive_date_bounds(&load.vods);
        Self {
            vods: load.vods,
            games,
            snapshot: load.snapshot,
            date_bounds,
        }
    }
}

pub struct AppState {
    /// Readers `Arc::clone` the current generation and drop the guard; the
    /// only writer is `vods::refresh_in_place`, which swaps the pointer.
    pub catalog: RwLock<Arc<Catalog>>,
    pub http_client: reqwest::Client,
    pub refresh_lock: tokio::sync::Mutex<()>,
    pub sync_store: Arc<sync_store::SyncStore>,
    pub emotes: RwLock<Arc<emotes::EmoteIndex>>,
}

pub type SharedState = Arc<AppState>;

/// Boot the catalog and emote caches, spawn the refresh tickers, and serve
/// the app until shutdown.
pub async fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                "moonmoon=info,tower_http=debug"
                    .parse()
                    .expect("default env filter is valid")
            }),
        )
        .init();

    let http_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .expect("failed to build HTTP client");

    const BOOT_FETCH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);
    let catalog =
        match tokio::time::timeout(BOOT_FETCH_TIMEOUT, vods::load_catalog(&http_client)).await {
            Ok(catalog) => catalog,
            Err(_) => {
                tracing::error!(
                    "boot fetch timed out after {:?}; starting with 0 vods",
                    BOOT_FETCH_TIMEOUT
                );
                vods::CatalogLoad::empty()
            }
        };
    let catalog = Catalog::build(catalog);
    tracing::info!("ready with {} vods", catalog.vods.len());
    tracing::info!("found {} games", catalog.games.len());

    const EMOTE_BOOT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);
    let prefetched =
        match tokio::time::timeout(EMOTE_BOOT_TIMEOUT, emotes::load_prefetched(&http_client)).await
        {
            Ok(map) => map,
            Err(_) => {
                tracing::warn!(
                    "emote boot fetch timed out after {:?}; starting with empty index",
                    EMOTE_BOOT_TIMEOUT
                );
                std::collections::HashMap::new()
            }
        };
    tracing::info!("emotes: prefetched {} entries", prefetched.len());

    let sync_store_path: std::path::PathBuf = std::env::var("SYNC_STORE_PATH")
        .unwrap_or_else(|_| "sync.json".to_string())
        .into();
    let sync_store = Arc::new(sync_store::SyncStore::load(sync_store_path).await);

    let state = Arc::new(AppState {
        catalog: RwLock::new(Arc::new(catalog)),
        http_client,
        refresh_lock: tokio::sync::Mutex::new(()),
        sync_store,
        emotes: RwLock::new(Arc::new(emotes::EmoteIndex::new(prefetched))),
    });

    let refresh_state = Arc::clone(&state);
    tokio::spawn(async move {
        loop {
            let count = refresh_state.catalog.read().await.vods.len();
            tokio::time::sleep(vods::next_refresh_delay(count)).await;
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

    let emote_state = Arc::clone(&state);
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(emotes::EMOTE_REFRESH_INTERVAL);
        tick.tick().await; // skip immediate tick; boot already loaded
        loop {
            tick.tick().await;
            let prefetched = emotes::load_prefetched(&emote_state.http_client).await;
            tracing::info!(
                "emote tick refresh: {} prefetched entries",
                prefetched.len()
            );
            let new_index = Arc::new(emotes::EmoteIndex::new(prefetched));
            *emote_state.emotes.write().await = new_index;
        }
    });

    let app = build_router(state);

    let port = std::env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    let addr = format!("0.0.0.0:{port}");
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|e| panic!("failed to bind {addr}: {e}"));
    tracing::info!("listening on http://localhost:{port}");
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await
    .expect("server exited with an error");
}

/// The full route table plus the rate-limiting, CSP-nonce, and tracing
/// layers. Also spawns the limiters' bookkeeping tasks, so it must run
/// inside a tokio runtime.
pub fn build_router(state: SharedState) -> Router {
    let api_governor = Arc::new(
        GovernorConfigBuilder::default()
            .key_extractor(SmartIpKeyExtractor)
            .per_second(2)
            .burst_size(20)
            .finish()
            .expect("valid governor config"),
    );

    // Emote lookups burst hard on chat load but are cache-backed and
    // single-flighted, so they get a lenient bucket of their own — a burst
    // must not 429 or starve a viewer's sync/playback API calls.
    let emote_governor = Arc::new(
        GovernorConfigBuilder::default()
            .key_extractor(SmartIpKeyExtractor)
            .per_second(20)
            .burst_size(120)
            .finish()
            .expect("valid emote governor config"),
    );

    for limiter in [
        api_governor.limiter().clone(),
        emote_governor.limiter().clone(),
    ] {
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(std::time::Duration::from_secs(60));
            loop {
                tick.tick().await;
                limiter.retain_recent();
            }
        });
    }

    let api_routes = Router::new()
        .route("/api/chat/{vod_id}", get(handlers::chat_proxy))
        .route("/api/vod/{vod_id}", get(handlers::vod_detail))
        .route("/api/next/{vod_id}", get(handlers::next_in_period))
        .route(
            "/api/sync/{token}",
            get(handlers::sync_get).put(handlers::sync_put),
        )
        .route("/api/refresh", post(handlers::refresh_catalog))
        .route("/history/resume", get(handlers::continue_resume))
        .route("/history/vods", post(handlers::history_vods_grid))
        .layer(GovernorLayer::new(api_governor));

    let emote_routes = Router::new()
        .route("/api/emotes/channel", get(handlers::channel_emotes))
        .route("/api/emotes/lookup/{name}", get(handlers::lookup_emote))
        .route("/api/emotes/vod/{vod_id}", get(handlers::vod_emotes))
        .layer(GovernorLayer::new(emote_governor));

    Router::new()
        .route("/", get(handlers::home_page))
        .route("/games", get(handlers::games_redirect))
        .route("/game/{name}", get(handlers::game_redirect))
        .route("/streams", get(handlers::streams_redirect))
        .route("/browse", get(handlers::browse_page))
        .route("/browse/grid", get(handlers::browse_grid))
        .route("/watch/{vod_id}", get(handlers::watch_page))
        .route("/calendar", get(handlers::calendar_page))
        .route("/history", get(handlers::history_page))
        .route("/random", get(handlers::random_vod))
        .merge(api_routes)
        .merge(emote_routes)
        .nest_service("/static", ServeDir::new("static"))
        .layer(axum::middleware::from_fn(middleware::csp_nonce))
        .layer(
            TraceLayer::new_for_http()
                .on_request(|req: &axum::http::Request<_>, _span: &tracing::Span| {
                    if !req.uri().path().starts_with("/api/chat/")
                        && !req.uri().path().starts_with("/static/")
                    {
                        tracing::info!("{} {}", req.method(), req.uri());
                    }
                })
                .on_response(
                    tower_http::trace::DefaultOnResponse::new().level(tracing::Level::TRACE),
                ),
        )
        .with_state(state)
}

async fn shutdown_signal() {
    match tokio::signal::ctrl_c().await {
        Ok(_) => tracing::info!("shutdown signal received"),
        Err(e) => tracing::error!("failed to install ctrl_c handler: {e}"),
    }
}
