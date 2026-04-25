use crate::SharedState;
use axum::Json;
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

// ─── Manual Refresh ───

pub async fn refresh_vods(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let outcome = crate::vods::refresh_in_place(&state).await;
    Json(outcome_to_json(outcome))
}

fn outcome_to_json(outcome: crate::vods::RefreshOutcome) -> serde_json::Value {
    use crate::vods::RefreshOutcome;
    match outcome {
        RefreshOutcome::Busy => serde_json::json!({ "status": "busy" }),
        RefreshOutcome::Unchanged(count) => {
            serde_json::json!({ "status": "unchanged", "count": count })
        }
        RefreshOutcome::Refreshed(count) => {
            serde_json::json!({ "status": "refreshed", "count": count })
        }
        RefreshOutcome::Error(message) => {
            serde_json::json!({ "status": "error", "message": message })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vods::RefreshOutcome;

    #[test]
    fn test_outcome_to_json_busy() {
        let v = outcome_to_json(RefreshOutcome::Busy);
        assert_eq!(v, serde_json::json!({ "status": "busy" }));
    }

    #[test]
    fn test_outcome_to_json_unchanged() {
        let v = outcome_to_json(RefreshOutcome::Unchanged(1419));
        assert_eq!(v, serde_json::json!({ "status": "unchanged", "count": 1419 }));
    }

    #[test]
    fn test_outcome_to_json_refreshed() {
        let v = outcome_to_json(RefreshOutcome::Refreshed(1420));
        assert_eq!(v, serde_json::json!({ "status": "refreshed", "count": 1420 }));
    }

    #[test]
    fn test_outcome_to_json_error() {
        let v = outcome_to_json(RefreshOutcome::Error("boom".into()));
        assert_eq!(v, serde_json::json!({ "status": "error", "message": "boom" }));
    }
}
