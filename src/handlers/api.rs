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
    let mut url = format!("https://archive.overpowered.tv/api/v1/moonmoon/vods/{vod_id}/comments");
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
            // Upstream wraps payloads as {success, data: {comments, cursor}};
            // forward only `data` so the frontend keeps its existing shape.
            let forwarded = if status.is_success() {
                match serde_json::from_str::<serde_json::Value>(&body) {
                    Ok(serde_json::Value::Object(mut obj)) => {
                        obj.remove("data").map(|d| d.to_string()).unwrap_or(body)
                    }
                    _ => body,
                }
            } else {
                body
            };
            (
                axum::http::StatusCode::from_u16(status.as_u16())
                    .unwrap_or(axum::http::StatusCode::INTERNAL_SERVER_ERROR),
                [("content-type", "application/json")],
                forwarded,
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
