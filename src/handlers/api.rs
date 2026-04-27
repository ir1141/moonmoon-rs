use crate::SharedState;
use axum::extract::{Path, Query, State};
use axum::response::IntoResponse;
use serde::Deserialize;

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
