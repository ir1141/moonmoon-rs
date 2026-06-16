use crate::SharedState;
use crate::emotes::parse;
use crate::emotes::{Lookup, ResolvedEntry};
use axum::extract::{Path, State};
use axum::response::IntoResponse;
use serde::Serialize;

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
            match search_all_providers(&client, &search_name).await {
                SearchOutcome::Hit(record) => Some(ResolvedEntry::Hit(record)),
                SearchOutcome::CleanMiss => Some(ResolvedEntry::Miss),
                SearchOutcome::AllFailed => None,
            }
        })
        .await;

    let body = match result {
        Lookup::Hit(record) => {
            serde_json::to_string(&LookupResponse::Hit { hit: true, record }).unwrap()
        }
        Lookup::Miss | Lookup::Unknown => {
            serde_json::to_string(&LookupResponse::Miss { hit: false }).unwrap()
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

enum SearchOutcome {
    Hit(crate::emotes::EmoteRecord),
    CleanMiss,
    AllFailed,
}

async fn search_all_providers(client: &reqwest::Client, name: &str) -> SearchOutcome {
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

    let outcomes = [s, b, f];
    if let Some(hit) = outcomes
        .iter()
        .find_map(|o| o.as_ref().and_then(|inner| inner.clone()))
    {
        return SearchOutcome::Hit(hit);
    }
    // Any provider that returned Some(None) is a clean miss. Treat at-least-one
    // clean miss as cacheable; treat all-None as transient.
    if outcomes.iter().any(|o| o.is_some()) {
        SearchOutcome::CleanMiss
    } else {
        SearchOutcome::AllFailed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
