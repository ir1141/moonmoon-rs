//! Sync endpoints. Tokens are base32 (RFC 4648 alphabet) so they survive
//! double-click selection, copy-paste between browsers, and QR codes.

use crate::SharedState;
use crate::sync_store::{MAX_BLOB_BYTES, SyncBlob};
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;

const TOKEN_MIN_LEN: usize = 26;
const TOKEN_MAX_LEN: usize = 32;

/// Accept only uppercase base32: `A-Z` and `2-7`, length 26..=32.
/// 26 = ceil(16 bytes * 8 / 5). Upper bound leaves slack for clients that
/// emit padding or longer tokens later.
pub(crate) fn is_valid_token(token: &str) -> bool {
    let len = token.len();
    if !(TOKEN_MIN_LEN..=TOKEN_MAX_LEN).contains(&len) {
        return false;
    }
    token
        .chars()
        .all(|c| c.is_ascii_uppercase() || ('2'..='7').contains(&c))
}

pub async fn sync_get(
    State(state): State<SharedState>,
    Path(token): Path<String>,
) -> axum::response::Response {
    if !is_valid_token(&token) {
        return (StatusCode::BAD_REQUEST, "invalid token").into_response();
    }
    match state.sync_store.get(&token).await {
        Some(blob) => Json(blob).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

pub async fn sync_put(
    State(state): State<SharedState>,
    Path(token): Path<String>,
    body: axum::body::Bytes,
) -> axum::response::Response {
    if !is_valid_token(&token) {
        return (StatusCode::BAD_REQUEST, "invalid token").into_response();
    }
    if body.len() > MAX_BLOB_BYTES {
        return (StatusCode::PAYLOAD_TOO_LARGE, "blob too large").into_response();
    }
    let blob: SyncBlob = match serde_json::from_slice(&body) {
        Ok(b) => b,
        Err(_) => return (StatusCode::BAD_REQUEST, "invalid json").into_response(),
    };
    if let Err(e) = state.sync_store.put(token, blob).await {
        tracing::error!("sync put failed: {e}");
        return (StatusCode::INTERNAL_SERVER_ERROR, "store error").into_response();
    }
    StatusCode::NO_CONTENT.into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_26_char_base32() {
        assert!(is_valid_token("ABCDEFGHIJKLMNOPQRSTUVWXYZ"));
        assert!(is_valid_token("AAAAAAAAAAAAAAAAAAAAAAAA23"));
    }

    #[test]
    fn accepts_up_to_32_chars() {
        assert!(is_valid_token("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAA23"));
    }

    #[test]
    fn rejects_too_short() {
        assert!(!is_valid_token(""));
        assert!(!is_valid_token("ABC"));
        assert!(!is_valid_token(&"A".repeat(25)));
    }

    #[test]
    fn rejects_too_long() {
        assert!(!is_valid_token(&"A".repeat(33)));
    }

    #[test]
    fn rejects_lowercase() {
        assert!(!is_valid_token("abcdefghijklmnopqrstuvwxyz"));
    }

    #[test]
    fn rejects_invalid_base32_digits() {
        // base32 doesn't include 0, 1, 8, or 9
        assert!(!is_valid_token("AAAAAAAAAAAAAAAAAAAAAAAAA0"));
        assert!(!is_valid_token("AAAAAAAAAAAAAAAAAAAAAAAAA1"));
        assert!(!is_valid_token("AAAAAAAAAAAAAAAAAAAAAAAAA8"));
        assert!(!is_valid_token("AAAAAAAAAAAAAAAAAAAAAAAAA9"));
    }

    #[test]
    fn rejects_punctuation_and_path_traversal() {
        assert!(!is_valid_token("../etc/passwd"));
        assert!(!is_valid_token("AAAAAAAAAAAAAAAAAAAAAAAAA-"));
        assert!(!is_valid_token("AAAAAAAAAAAAAAAAAAAAAAAAA "));
    }
}
