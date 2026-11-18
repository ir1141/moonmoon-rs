use axum::extract::{Path, State};
use axum::response::IntoResponse;
use axum::response::Response;
use serde::Serialize;

use super::valid_vod_id;

use crate::SharedState;
use crate::emotes::parse;
use crate::emotes::{Lookup, ResolvedEntry};

#[derive(Serialize)]
pub struct ChannelEmotesResponse<'a> {
    pub emotes: &'a std::collections::HashMap<String, crate::emotes::EmoteRecord>,
}

pub async fn channel_emotes(State(state): State<SharedState>) -> impl IntoResponse {
    let index = state.emotes.read().await.clone();
    let body = serde_json::to_string(&ChannelEmotesResponse {
        emotes: &index.prefetched,
    })
    .expect("EmoteRecord is always serializable");
    (
        axum::http::StatusCode::OK,
        [
            ("content-type", "application/json"),
            ("cache-control", "public, max-age=300"),
        ],
        body,
    )
}

#[derive(Serialize)]
#[serde(untagged)]
pub enum LookupResponse {
    Hit {
        hit: bool, // always true
        #[serde(flatten)]
        record: crate::emotes::EmoteRecord,
    },
    Miss {
        hit: bool, // always false
    },
}

pub async fn lookup_emote(
    State(state): State<SharedState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    if !valid_emote_name(&name) {
        return (
            axum::http::StatusCode::BAD_REQUEST,
            [("content-type", "application/json")],
            r#"{"error":"invalid name"}"#.to_string(),
        );
    }

    let index = state.emotes.read().await.clone();
    let client = state.http_client.clone();
    let search_name = name.clone();
    let result = index
        .lookup_or_resolve(&name, || async move {
            search_all_providers(&client, &search_name).await
        })
        .await;

    let body = match result {
        Lookup::Hit(record) => serde_json::to_string(&LookupResponse::Hit { hit: true, record })
            .expect("LookupResponse is always serializable"),
        Lookup::Miss => serde_json::to_string(&LookupResponse::Miss { hit: false })
            .expect("LookupResponse is always serializable"),
        Lookup::Unknown => {
            return (
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                [("content-type", "application/json")],
                r#"{"error":"lookup unavailable, retry later"}"#.to_string(),
            );
        }
    };
    (
        axum::http::StatusCode::OK,
        [("content-type", "application/json")],
        body,
    )
}

// Mirrors the JS isEmoteCandidate length/charset rules so we never call
// providers with garbage. The client also gates with emote-heuristic.js,
// but the server validates independently because the endpoint is public.
fn valid_emote_name(name: &str) -> bool {
    let len = name.chars().count();
    if !(3..=25).contains(&len) {
        return false;
    }
    name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Proxy the archive's per-VOD emote snapshot, returning the same `{emotes}`
/// map shape as `channel_emotes`. Unknown ids, upstream 404s (old VODs predate
/// snapshot capture, roughly id < 310), and transport errors all return an
/// empty map so the client falls back cleanly to prefetch + live search.
pub async fn vod_emotes(State(state): State<SharedState>, Path(vod_id): Path<String>) -> Response {
    use axum::http::StatusCode;

    if !valid_vod_id(&vod_id) {
        return (
            StatusCode::BAD_REQUEST,
            [("content-type", "application/json")],
            r#"{"error":"invalid vod_id"}"#,
        )
            .into_response();
    }

    let canonical = {
        let catalog = state.catalog.read().await;
        super::find_vod_by_id(&catalog.vods, &vod_id).map(|v| v.id.clone())
    };
    let empty = || {
        (
            StatusCode::OK,
            [
                ("content-type", "application/json"),
                ("cache-control", "public, max-age=300"),
            ],
            r#"{"emotes":{}}"#.to_string(),
        )
            .into_response()
    };
    let Some(canonical) = canonical else {
        return empty();
    };

    let body = match state
        .vod_emotes
        .get(&canonical, crate::emotes::VOD_EMOTES_TTL)
    {
        Some(body) => body,
        None => {
            let url =
                format!("https://archive.overpowered.tv/api/v1/moonmoon/vods/{canonical}/emotes");
            let fetched = match state.http_client.get(&url).send().await {
                Ok(resp) => {
                    let status = resp.status();
                    let json = resp.json::<serde_json::Value>().await.ok();
                    let outcome = vod_snapshot_outcome(status, json);
                    if outcome.is_none() {
                        tracing::warn!(
                            "vod emote snapshot for {canonical}: unusable upstream {status}"
                        );
                    }
                    outcome
                }
                Err(e) => {
                    tracing::warn!("vod emote snapshot fetch failed for {canonical}: {e}");
                    None
                }
            };
            let cacheable = fetched.is_some();
            let body: std::sync::Arc<str> =
                serde_json::json!({ "emotes": fetched.unwrap_or_default() })
                    .to_string()
                    .into();
            if cacheable {
                state
                    .vod_emotes
                    .put(canonical, std::sync::Arc::clone(&body));
            }
            body
        }
    };
    (
        StatusCode::OK,
        [
            ("content-type", "application/json"),
            ("cache-control", "public, max-age=300"),
        ],
        body.to_string(),
    )
        .into_response()
}

/// The emote map to serve for an archive snapshot response, or `None` when the
/// answer is transient (rate limit, server error, unexpected body) and must not
/// be cached. A 404 is definitive: old VODs have no snapshot.
fn vod_snapshot_outcome(
    status: axum::http::StatusCode,
    json: Option<serde_json::Value>,
) -> Option<std::collections::HashMap<String, crate::emotes::EmoteRecord>> {
    if status == axum::http::StatusCode::NOT_FOUND {
        return Some(Default::default());
    }
    let json = json.filter(|_| status.is_success())?;
    (json.get("success").and_then(|v| v.as_bool()) == Some(true)).then(|| {
        json.get("data")
            .map(parse::parse_vod_emote_snapshot)
            .unwrap_or_default()
    })
}

/// Query all three providers concurrently and combine the answers via
/// [`ResolvedEntry::from_search_results`]; only the HTTP glue lives here.
async fn search_all_providers(client: &reqwest::Client, name: &str) -> Option<ResolvedEntry> {
    let encoded = urlencoding::encode(name);
    let seventv_body = serde_json::json!({
        "operationName": "SearchEmotes",
        "variables": {
            "query": name,
            "limit": 4,
            "page": 1,
            "sort": { "value": "popularity", "order": "DESCENDING" },
            "filter": {
                "category": "TOP",
                "exact_match": true,
                "case_sensitive": true,
                "ignore_tags": false,
                "zero_width": false,
                "animated": false,
                "aspect_ratio": ""
            }
        },
        "query": include_str!("../emotes/seventv_search.graphql")
    });

    let seventv = async {
        let resp = client
            .post("https://7tv.io/v3/gql")
            .json(&seventv_body)
            .send()
            .await
            .ok()?
            .error_for_status()
            .ok()?
            .json::<serde_json::Value>()
            .await
            .ok()?;
        Some(parse::parse_seventv_search(&resp, name))
    };
    let bttv = async {
        let resp = client
            .get(format!(
                "https://api.betterttv.net/3/emotes/shared/search?query={encoded}&offset=0&limit=10"
            ))
            .send()
            .await
            .ok()?
            .error_for_status()
            .ok()?
            .json::<serde_json::Value>()
            .await
            .ok()?;
        Some(parse::parse_bttv_search(&resp, name))
    };
    let ffz = async {
        let resp = client
            .get(format!(
                "https://api.frankerfacez.com/v1/emotes?q={encoded}&sensitive=false&sort=count-desc&page=1"
            ))
            .send()
            .await
            .ok()?
            .error_for_status()
            .ok()?
            .json::<serde_json::Value>()
            .await
            .ok()?;
        Some(parse::parse_ffz_search(&resp, name))
    };

    let (s, b, f) = tokio::join!(seventv, bttv, ffz);

    ResolvedEntry::from_search_results([s, b, f])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vod_snapshot_caches_only_definitive_answers() {
        use axum::http::StatusCode;
        let ok = serde_json::json!({ "success": true, "data": {} });
        assert!(vod_snapshot_outcome(StatusCode::OK, Some(ok.clone())).is_some());
        assert!(vod_snapshot_outcome(StatusCode::NOT_FOUND, None).is_some());
        assert!(vod_snapshot_outcome(StatusCode::TOO_MANY_REQUESTS, Some(ok.clone())).is_none());
        assert!(vod_snapshot_outcome(StatusCode::BAD_GATEWAY, None).is_none());
        assert!(vod_snapshot_outcome(StatusCode::FORBIDDEN, Some(ok)).is_none());
        assert!(vod_snapshot_outcome(StatusCode::OK, None).is_none());
        let refused = serde_json::json!({ "success": false });
        assert!(vod_snapshot_outcome(StatusCode::OK, Some(refused)).is_none());
    }
    use crate::emotes::{EmoteIndex, EmoteProvider, EmoteRecord};
    use std::collections::HashMap;

    #[test]
    fn channel_response_serializes_prefetched_map() {
        let mut pre = HashMap::new();
        pre.insert(
            "PogU".to_string(),
            EmoteRecord {
                url: "https://x/1".into(),
                provider: EmoteProvider::SevenTv,
                owner: None,
            },
        );
        let idx = EmoteIndex::new(pre);
        let body = serde_json::to_string(&ChannelEmotesResponse {
            emotes: &idx.prefetched,
        })
        .unwrap();
        assert!(body.contains("\"PogU\""));
        assert!(body.contains("\"7TV\""));
    }

    #[test]
    fn valid_emote_name_enforces_length_and_charset() {
        assert!(super::valid_emote_name("PogU"));
        assert!(super::valid_emote_name("moon2A"));
        assert!(super::valid_emote_name("Pog_U"));
        assert!(!super::valid_emote_name("ab")); // too short
        assert!(!super::valid_emote_name(&"a".repeat(26))); // too long
        assert!(!super::valid_emote_name("hi there")); // space
        assert!(!super::valid_emote_name("emo.te")); // dot
        assert!(!super::valid_emote_name("emoté")); // non-ascii
    }

    #[test]
    fn lookup_response_serializes_flat_hit() {
        let body = serde_json::to_string(&LookupResponse::Hit {
            hit: true,
            record: crate::emotes::EmoteRecord {
                url: "https://x/1".into(),
                provider: crate::emotes::EmoteProvider::Bttv,
                owner: Some("o".into()),
            },
        })
        .unwrap();
        assert!(body.contains("\"hit\":true"));
        assert!(body.contains("\"url\":\"https://x/1\""));
        assert!(body.contains("\"provider\":\"BTTV\""));
        assert!(body.contains("\"owner\":\"o\""));
    }

    #[test]
    fn lookup_response_serializes_miss() {
        let body = serde_json::to_string(&LookupResponse::Miss { hit: false }).unwrap();
        assert_eq!(body, "{\"hit\":false}");
    }
}
