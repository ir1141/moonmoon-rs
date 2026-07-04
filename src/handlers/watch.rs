use super::{
    Section, find_vod_by_id, get_chapter_segments, get_game_tags, next_vod_in_period,
    render_template,
};
use crate::SharedState;
use crate::middleware::CspNonce;
use crate::vods::{Vod, canonical_youtube_uploads};
use askama::Template;
use axum::extract::{Extension, Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect};
use rand::prelude::IndexedRandom;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Template)]
#[template(path = "watch.html")]
struct WatchTemplate {
    vod_id: String,
    vod_title: String,
    youtube_parts_json: String,
    chapters_json: String,
    total_secs: i64,
    game_hint: String,
    active_section: Section,
    nonce: String,
}

#[derive(Serialize)]
struct YoutubePartPayload {
    id: String,
    duration: Option<i64>,
}

#[derive(Serialize)]
struct ChapterPayload {
    name: String,
    color: u8,
    start: i64,
}

#[derive(Deserialize, Default)]
pub struct GameQuery {
    pub game: Option<String>,
}

pub async fn watch_page(
    State(state): State<SharedState>,
    Extension(nonce): Extension<CspNonce>,
    Path(vod_id): Path<String>,
    Query(params): Query<GameQuery>,
) -> impl IntoResponse {
    let catalog = Arc::clone(&*state.catalog.read().await);
    let vods = &catalog.vods;
    match find_vod_by_id(vods, &vod_id) {
        Some(v) => render_template(&WatchTemplate {
            vod_id: v.id.clone(),
            vod_title: v.title.clone().unwrap_or_else(|| "Untitled".into()),
            youtube_parts_json: youtube_parts_json(v),
            chapters_json: chapters_json(v),
            total_secs: v.duration.as_ref().map_or(0, |duration| duration.seconds()),
            game_hint: params.game.unwrap_or_default(),
            active_section: Section::None,
            nonce: nonce.0,
        }),
        None => (StatusCode::NOT_FOUND, "VOD not found").into_response(),
    }
}

fn youtube_parts_json(vod: &Vod) -> String {
    let parts: Vec<YoutubePartPayload> = canonical_youtube_uploads(vod)
        .into_iter()
        .map(|upload| YoutubePartPayload {
            id: upload.id,
            duration: upload.duration.filter(|duration| *duration > 0),
        })
        .collect();
    serde_json::to_string(&parts).unwrap_or_else(|_| "[]".into())
}

fn chapters_json(vod: &Vod) -> String {
    let chapters: Vec<ChapterPayload> = get_chapter_segments(vod)
        .into_iter()
        .map(|seg| ChapterPayload {
            name: seg.name,
            color: seg.color_idx,
            start: seg.start_secs,
        })
        .collect();
    serde_json::to_string(&chapters).unwrap_or_else(|_| "[]".into())
}

pub async fn vod_detail(
    State(state): State<SharedState>,
    Path(vod_id): Path<String>,
) -> impl IntoResponse {
    let catalog = Arc::clone(&*state.catalog.read().await);
    let vods = &catalog.vods;
    if let Some(vod) = find_vod_by_id(vods, &vod_id) {
        axum::Json(vod.clone()).into_response()
    } else {
        (StatusCode::NOT_FOUND, "vod not found").into_response()
    }
}

pub async fn next_in_period(
    State(state): State<SharedState>,
    Path(vod_id): Path<String>,
    Query(params): Query<GameQuery>,
) -> impl IntoResponse {
    let catalog = Arc::clone(&*state.catalog.read().await);
    let vods = &catalog.vods;

    let Some(current) = find_vod_by_id(vods, &vod_id) else {
        return StatusCode::NOT_FOUND.into_response();
    };

    // Resolve game: explicit query param, else infer if the current VOD has exactly one distinct game.
    let game = match params.game.filter(|s| !s.is_empty()) {
        Some(g) => g,
        None => {
            let mut tags = get_game_tags(current);
            if tags.len() == 1 {
                tags.remove(0)
            } else {
                return StatusCode::NO_CONTENT.into_response();
            }
        }
    };

    match next_vod_in_period(vods, &current.id, &game) {
        Some(next) => axum::Json(serde_json::json!({
            "next_id": next.id,
            "next_title": next.title,
            "game": game,
        }))
        .into_response(),
        None => StatusCode::NO_CONTENT.into_response(),
    }
}

pub async fn random_vod(State(state): State<SharedState>) -> Redirect {
    let catalog = Arc::clone(&*state.catalog.read().await);
    let vods = &catalog.vods;
    if let Some(pick) = vods.choose(&mut rand::rng()) {
        Redirect::temporary(&format!("/watch/{}", pick.id))
    } else {
        Redirect::temporary("/")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vods::{Chapter, Vod, VodDuration, YoutubeVideo};

    #[test]
    fn test_youtube_parts_json_uses_canonical_ids_and_durations() {
        let vod = Vod {
            id: "1430".into(),
            platform: Some("twitch".into()),
            platform_vod_id: Some("2768249708".into()),
            platform_stream_id: None,
            title: Some("Playable Stream".into()),
            created_at: "2026-05-10T23:05:44.967Z".into(),
            started_at: Some("2026-05-09T22:35:39.000Z".into()),
            updated_at: None,
            duration: Some(VodDuration::from_seconds(25194)),
            thumbnail_url: None,
            chapters: Some(vec![Chapter {
                name: Some("HITMAN".into()),
                image: None,
                start: Some(0.0),
                duration: None,
                end: None,
            }]),
            youtube: Some(vec![
                YoutubeVideo {
                    row_id: None,
                    id: "live-1".into(),
                    thumbnail_url: None,
                    part: Some(1),
                    duration: Some(10800),
                    status: Some("COMPLETED".into()),
                    upload_type: Some("live".into()),
                    created_at: None,
                },
                YoutubeVideo {
                    row_id: None,
                    id: "vod-2".into(),
                    thumbnail_url: None,
                    part: Some(2),
                    duration: Some(3594),
                    status: Some("COMPLETED".into()),
                    upload_type: Some("vod".into()),
                    created_at: None,
                },
                YoutubeVideo {
                    row_id: None,
                    id: "vod-1".into(),
                    thumbnail_url: None,
                    part: Some(1),
                    duration: Some(10800),
                    status: Some("COMPLETED".into()),
                    upload_type: Some("vod".into()),
                    created_at: None,
                },
            ]),
            is_live: false,
        };

        let value: serde_json::Value = serde_json::from_str(&youtube_parts_json(&vod)).unwrap();

        assert_eq!(
            value,
            serde_json::json!([
                {"id": "vod-1", "duration": 10800},
                {"id": "vod-2", "duration": 3594}
            ])
        );
    }

    fn chapter(name: &str, start: f64) -> Chapter {
        Chapter {
            name: Some(name.into()),
            image: None,
            start: Some(start),
            duration: None,
            end: None,
        }
    }

    fn vod_with_chapters(duration_secs: Option<i64>, chapters: Vec<Chapter>) -> Vod {
        Vod {
            id: "abc".into(),
            platform: Some("twitch".into()),
            platform_vod_id: None,
            platform_stream_id: None,
            title: Some("Test".into()),
            created_at: "2026-05-10T23:05:44.967Z".into(),
            started_at: None,
            updated_at: None,
            duration: duration_secs.map(VodDuration::from_seconds),
            thumbnail_url: None,
            chapters: Some(chapters),
            youtube: None,
            is_live: false,
        }
    }

    #[test]
    fn test_chapters_json_maps_segments_to_name_color_start() {
        let vod = vod_with_chapters(
            Some(10000),
            vec![chapter("Alpha", 0.0), chapter("Bravo", 3600.0)],
        );

        let value: serde_json::Value = serde_json::from_str(&chapters_json(&vod)).unwrap();

        assert_eq!(
            value,
            serde_json::json!([
                {"name": "Alpha", "color": crate::vods::chapter_color_idx("Alpha"), "start": 0},
                {"name": "Bravo", "color": crate::vods::chapter_color_idx("Bravo"), "start": 3600},
            ])
        );
    }

    #[test]
    fn test_chapters_json_is_empty_array_without_duration() {
        // No total duration → no resolvable segments → an empty (not malformed) payload.
        let vod = vod_with_chapters(None, vec![chapter("Alpha", 0.0)]);
        assert_eq!(chapters_json(&vod), "[]");
    }
}
