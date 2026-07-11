//! Router-level integration tests: drive the full route table (rate
//! limiting, CSP nonce, and tracing layers included) via `tower::oneshot`
//! without binding a socket or touching the network.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use tokio::sync::RwLock;
use tower::ServiceExt;

use moonmoon::{AppState, Catalog, CatalogLoad, EmoteIndex, SharedState, SyncStore};

fn state_with_sync_path(path: PathBuf) -> SharedState {
    Arc::new(AppState {
        catalog: RwLock::new(Arc::new(Catalog::build(CatalogLoad::empty()))),
        http_client: reqwest::Client::new(),
        refresh_lock: tokio::sync::Mutex::new(()),
        sync_store: Arc::new(SyncStore::new_in_memory(path)),
        emotes: RwLock::new(Arc::new(EmoteIndex::new(Default::default()))),
    })
}

fn empty_state() -> SharedState {
    state_with_sync_path(std::env::temp_dir().join("moonmoon-router-test-unused.json"))
}

/// The governor layers key requests by client IP, which `oneshot` requests
/// don't carry by default; stamp a loopback peer on every test request.
fn request(method: &str, uri: &str, body: Body) -> Request<Body> {
    let mut req = Request::builder()
        .method(method)
        .uri(uri)
        .body(body)
        .unwrap();
    req.extensions_mut()
        .insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 42000))));
    req
}

async fn body_string(resp: axum::response::Response) -> String {
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    String::from_utf8(bytes.to_vec()).unwrap()
}

#[tokio::test]
async fn home_page_renders_recovery_state_when_catalog_is_empty() {
    let app = moonmoon::build_router(empty_state());

    let resp = app
        .oneshot(request("GET", "/", Body::empty()))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let html = body_string(resp).await;
    assert!(html.contains("The archive isn't loaded"), "{html}");
}

#[tokio::test]
async fn watch_page_returns_not_found_for_unknown_vod() {
    let app = moonmoon::build_router(empty_state());

    let resp = app
        .oneshot(request("GET", "/watch/no-such-vod", Body::empty()))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn games_route_redirects_into_the_browse_games_lens() {
    let app = moonmoon::build_router(empty_state());

    let resp = app
        .oneshot(request("GET", "/games", Body::empty()))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::TEMPORARY_REDIRECT);
    assert_eq!(
        resp.headers().get(header::LOCATION).unwrap(),
        "/browse?lens=games"
    );
}

#[tokio::test]
async fn responses_carry_a_content_security_policy() {
    let app = moonmoon::build_router(empty_state());

    let resp = app
        .oneshot(request("GET", "/", Body::empty()))
        .await
        .unwrap();

    let csp = resp
        .headers()
        .get(header::CONTENT_SECURITY_POLICY)
        .expect("CSP header present")
        .to_str()
        .unwrap();
    assert!(csp.contains("'nonce-"), "{csp}");
}

#[tokio::test]
async fn sync_get_rejects_malformed_tokens() {
    let app = moonmoon::build_router(empty_state());

    let resp = app
        .oneshot(request("GET", "/api/sync/short", Body::empty()))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn sync_put_then_get_roundtrips_through_the_router() {
    let path = std::env::temp_dir().join(format!(
        "moonmoon-router-test-roundtrip-{}.json",
        std::process::id()
    ));
    let app = moonmoon::build_router(state_with_sync_path(path.clone()));
    let token = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";

    let put = request(
        "PUT",
        &format!("/api/sync/{token}"),
        Body::from(r#"{"blob":{"resume":{}},"updated_at":7}"#),
    );
    let resp = app.clone().oneshot(put).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    let resp = app
        .oneshot(request("GET", &format!("/api/sync/{token}"), Body::empty()))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    assert!(body.contains("\"updated_at\":7"), "{body}");

    let _ = std::fs::remove_file(&path);
}
