use super::{Section, next_vod_in_period, render_template};
use crate::SharedState;
use askama::Template;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect};
use rand::prelude::IndexedRandom;
use serde::Deserialize;
use std::sync::Arc;

#[derive(Template)]
#[template(path = "watch.html")]
struct WatchTemplate {
    vod_id: String,
    vod_title: String,
    youtube_ids_json: String,
    game_hint: String,
    active_section: Section,
}

#[derive(Deserialize, Default)]
pub struct GameQuery {
    pub game: Option<String>,
}

pub async fn watch_page(
    State(state): State<SharedState>,
    Path(vod_id): Path<String>,
    Query(params): Query<GameQuery>,
) -> impl IntoResponse {
    let vods = {
        let guard = state.vods.read().await;
        Arc::clone(&*guard)
    };
    match vods.iter().find(|v| v.id == vod_id) {
        Some(v) => {
            let youtube_ids: Vec<String> = v
                .youtube
                .as_ref()
                .map(|yt| yt.iter().map(|y| y.id.clone()).collect())
                .unwrap_or_default();
            let youtube_ids_json =
                serde_json::to_string(&youtube_ids).unwrap_or_else(|_| "[]".into());
            render_template(&WatchTemplate {
                vod_id: v.id.clone(),
                vod_title: v.title.clone().unwrap_or_else(|| "Untitled".into()),
                youtube_ids_json,
                game_hint: params.game.unwrap_or_default(),
                active_section: Section::None,
            })
        }
        None => (StatusCode::NOT_FOUND, "VOD not found").into_response(),
    }
}

pub async fn vod_detail(
    State(state): State<SharedState>,
    Path(vod_id): Path<String>,
) -> impl IntoResponse {
    let vods = {
        let guard = state.vods.read().await;
        Arc::clone(&*guard)
    };
    if let Some(vod) = vods.iter().find(|v| v.id == vod_id) {
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
    let vods = {
        let guard = state.vods.read().await;
        Arc::clone(&*guard)
    };

    let Some(current) = vods.iter().find(|v| v.id == vod_id) else {
        return StatusCode::NOT_FOUND.into_response();
    };

    // Resolve game: explicit query param, else infer if the current VOD has exactly one distinct game.
    let game = match params.game.filter(|s| !s.is_empty()) {
        Some(g) => g,
        None => {
            let mut names: Vec<String> = Vec::new();
            if let Some(ref chapters) = current.chapters {
                for ch in chapters {
                    if let Some(ref n) = ch.name
                        && !n.is_empty()
                        && !names.iter().any(|x| x.eq_ignore_ascii_case(n))
                    {
                        names.push(n.clone());
                    }
                }
            }
            if names.len() == 1 {
                names.remove(0)
            } else {
                return StatusCode::NO_CONTENT.into_response();
            }
        }
    };

    match next_vod_in_period(&vods, &vod_id, &game) {
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
    let vods = {
        let guard = state.vods.read().await;
        Arc::clone(&*guard)
    };
    if let Some(pick) = vods.choose(&mut rand::rng()) {
        Redirect::temporary(&format!("/watch/{}", pick.id))
    } else {
        Redirect::temporary("/")
    }
}
