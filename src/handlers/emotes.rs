use crate::SharedState;
use axum::extract::State;
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
}
