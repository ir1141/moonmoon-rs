use crate::SharedState;
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::response::IntoResponse;
use serde::Deserialize;
use std::sync::Arc;

// ─── Chat proxy ───

#[derive(Deserialize)]
pub struct ChatQuery {
    pub content_offset_seconds: Option<f64>,
    pub cursor: Option<String>,
}

pub async fn chat_proxy(
    State(state): State<SharedState>,
    Path(vod_id): Path<String>,
    Query(params): Query<ChatQuery>,
) -> impl IntoResponse {
    if !vod_id
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        return (axum::http::StatusCode::BAD_REQUEST, "invalid vod_id").into_response();
    }
    let mut url = format!("https://archive.overpowered.tv/moonmoon/v1/vods/{vod_id}/comments");
    let mut qparts = vec![];
    if let Some(offset) = params.content_offset_seconds
        && offset.is_finite()
        && offset >= 0.0
    {
        qparts.push(format!("content_offset_seconds={offset}"));
    }
    if let Some(ref cursor) = params.cursor {
        let encoded = urlencoding::encode(cursor);
        qparts.push(format!("cursor={encoded}"));
    }
    if !qparts.is_empty() {
        url = format!("{url}?{}", qparts.join("&"));
    }

    match state.http_client.get(&url).send().await {
        Ok(resp) => {
            let status = resp.status();
            let body = match resp.text().await {
                Ok(b) => b,
                Err(e) => {
                    tracing::warn!("chat proxy: failed to read upstream body: {e}");
                    return (axum::http::StatusCode::BAD_GATEWAY, "proxy body read error")
                        .into_response();
                }
            };
            (
                axum::http::StatusCode::from_u16(status.as_u16())
                    .unwrap_or(axum::http::StatusCode::INTERNAL_SERVER_ERROR),
                [("content-type", "application/json")],
                body,
            )
                .into_response()
        }
        Err(e) => (
            axum::http::StatusCode::BAD_GATEWAY,
            format!("proxy error: {e}"),
        )
            .into_response(),
    }
}

// ─── Manual Refresh ───

pub async fn refresh_vods(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let _refresh_guard = match state.refresh_lock.try_lock() {
        Ok(g) => g,
        Err(_) => {
            tracing::info!("refresh: already in progress, skipping");
            return Json(serde_json::json!({ "status": "busy" }));
        }
    };

    let cached_count = state.vods.read().await.len();

    let remote_count = match crate::vods::fetch_vod_count(&state.http_client).await {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("refresh: failed to check vod count: {e}");
            return Json(serde_json::json!({
                "status": "error",
                "message": format!("failed to check vod count: {e}")
            }));
        }
    };

    if remote_count == cached_count {
        tracing::info!("refresh: vod count unchanged ({cached_count})");
        return Json(serde_json::json!({
            "status": "unchanged",
            "count": cached_count
        }));
    }

    tracing::info!("refresh: vod count changed ({cached_count} -> {remote_count}), fetching...");
    let new_vods = match crate::vods::fetch_all_vods(&state.http_client).await {
        Ok(v) => v,
        Err(e) => {
            tracing::error!("refresh: failed to fetch vods: {e}");
            return Json(serde_json::json!({
                "status": "error",
                "message": format!("failed to fetch vods: {e}")
            }));
        }
    };

    let new_vods = Arc::new(new_vods);
    let new_games = Arc::new(crate::vods::build_games(&new_vods));
    let count = new_vods.len();

    let vods_for_cache = Arc::clone(&new_vods);
    if let Err(e) =
        tokio::task::spawn_blocking(move || crate::vods::write_cache(&vods_for_cache)).await
    {
        tracing::warn!("refresh: cache write task join error: {e}");
    }

    {
        let mut vods_w = state.vods.write().await;
        let mut games_w = state.games.write().await;
        *vods_w = new_vods;
        *games_w = new_games;
    }

    tracing::info!("refresh: complete ({count} vods)");
    Json(serde_json::json!({ "status": "refreshed", "count": count }))
}
