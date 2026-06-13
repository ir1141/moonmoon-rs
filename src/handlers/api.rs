use super::find_vod_by_id;
use crate::{SharedState, vods::Vod};
use axum::extract::{Path, Query, State};
use axum::response::IntoResponse;
use serde::Deserialize;

// ─── Chat proxy ───

#[derive(Deserialize)]
pub struct ChatQuery {
    pub content_offset_seconds: Option<f64>,
    pub cursor: Option<String>,
}

/// Upstream cursors are short opaque strings; anything huge is garbage and
/// just burns an upstream connection on a guaranteed-failing URL.
const MAX_CURSOR_LEN: usize = 2048;

fn canonical_chat_vod_id(vods: &[Vod], requested_id: &str) -> Option<String> {
    find_vod_by_id(vods, requested_id).map(|vod| vod.id.clone())
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
    let canonical_vod_id = {
        let catalog = state.catalog.read().await;
        canonical_chat_vod_id(&catalog.vods, &vod_id)
    };
    let Some(canonical_vod_id) = canonical_vod_id else {
        return (axum::http::StatusCode::NOT_FOUND, "unknown vod_id").into_response();
    };

    let mut url =
        format!("https://archive.overpowered.tv/api/v1/moonmoon/vods/{canonical_vod_id}/comments");
    let mut qparts = vec![];
    if let Some(offset) = params.content_offset_seconds
        && offset.is_finite()
        && offset >= 0.0
    {
        qparts.push(format!("content_offset_seconds={offset}"));
    }
    if let Some(ref cursor) = params.cursor {
        if cursor.len() > MAX_CURSOR_LEN {
            return (axum::http::StatusCode::BAD_REQUEST, "cursor too long").into_response();
        }
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

// ─── Manual refresh ───

pub async fn refresh_catalog(State(state): State<SharedState>) -> impl IntoResponse {
    use crate::vods::RefreshOutcome;
    match crate::vods::refresh_in_place(&state).await {
        RefreshOutcome::Refreshed(n) => {
            (axum::http::StatusCode::OK, format!("refreshed {n} vods")).into_response()
        }
        RefreshOutcome::Unchanged(n) => {
            (axum::http::StatusCode::OK, format!("unchanged ({n} vods)")).into_response()
        }
        RefreshOutcome::Busy => (
            axum::http::StatusCode::ACCEPTED,
            "refresh already in progress".to_string(),
        )
            .into_response(),
        RefreshOutcome::Error(e) => (axum::http::StatusCode::BAD_GATEWAY, e).into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_vod(id: &str, platform_vod_id: Option<&str>) -> Vod {
        Vod {
            id: id.into(),
            platform: None,
            platform_vod_id: platform_vod_id.map(str::to_string),
            platform_stream_id: None,
            title: Some(format!("vod {id}")),
            created_at: "2026-05-09T22:35:39.000Z".into(),
            started_at: None,
            updated_at: Some("2026-05-10T00:00:00.000Z".into()),
            duration: Some("1h".into()),
            thumbnail_url: None,
            chapters: None,
            youtube: None,
            is_live: false,
        }
    }

    #[test]
    fn test_canonical_chat_vod_id_accepts_internal_id() {
        let vods = vec![make_vod("1430", Some("2768249708"))];

        assert_eq!(
            canonical_chat_vod_id(&vods, "1430").as_deref(),
            Some("1430")
        );
    }

    #[test]
    fn test_canonical_chat_vod_id_accepts_platform_vod_id() {
        let vods = vec![make_vod("1430", Some("2768249708"))];

        assert_eq!(
            canonical_chat_vod_id(&vods, "2768249708").as_deref(),
            Some("1430")
        );
    }

    #[test]
    fn test_canonical_chat_vod_id_rejects_unknown_id() {
        let vods = vec![make_vod("1430", Some("2768249708"))];

        assert_eq!(canonical_chat_vod_id(&vods, "missing"), None);
    }
}
