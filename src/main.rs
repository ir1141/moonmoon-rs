use axum::{
    Router,
    routing::{get, post},
};
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;

mod handlers;
mod vods;

pub struct AppState {
    pub vods: RwLock<Arc<Vec<vods::Vod>>>,
    pub games: RwLock<Arc<Vec<vods::Game>>>,
    pub http_client: reqwest::Client,
    pub refresh_lock: tokio::sync::Mutex<()>,
}

pub type SharedState = Arc<AppState>;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "moonmoon=info,tower_http=debug".parse().unwrap()),
        )
        .init();

    let http_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .expect("failed to build HTTP client");

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
    tracing::info!("ready with {} vods", all_vods.len());

    let games = vods::build_games(&all_vods);
    tracing::info!("found {} games", games.len());

    let state = Arc::new(AppState {
        vods: RwLock::new(Arc::new(all_vods)),
        games: RwLock::new(Arc::new(games)),
        http_client,
        refresh_lock: tokio::sync::Mutex::new(()),
    });

    let app = Router::new()
        .route("/", get(handlers::games_page))
        .route("/games", get(handlers::games_grid))
        .route("/game/{name}", get(handlers::game_vods_page))
        .route("/game/{name}/vods", get(handlers::game_vods_grid))
        .route("/streams", get(handlers::all_streams_page))
        .route("/streams/vods", get(handlers::all_streams_grid))
        .route("/watch/{vod_id}", get(handlers::watch_page))
        .route("/api/chat/{vod_id}", get(handlers::chat_proxy))
        .route("/api/vod/{vod_id}", get(handlers::vod_detail))
        .route("/api/next/{vod_id}", get(handlers::next_in_period))
        .route("/calendar", get(handlers::calendar_page))
        .route("/history", get(handlers::history_page))
        .route("/history/vods", get(handlers::history_vods_grid))
        .route("/random", get(handlers::random_vod))
        .route("/api/refresh", post(handlers::refresh_vods))
        .nest_service("/static", ServeDir::new("static"))
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
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    tracing::info!("listening on http://localhost:3000");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();
}

async fn shutdown_signal() {
    match tokio::signal::ctrl_c().await {
        Ok(_) => tracing::info!("shutdown signal received"),
        Err(e) => tracing::error!("failed to install ctrl_c handler: {e}"),
    }
}
