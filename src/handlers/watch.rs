use super::render_template;
use crate::SharedState;
use askama::Template;
use axum::extract::{Path, State};
use axum::response::{IntoResponse, Redirect};
use rand::prelude::IndexedRandom;
use std::sync::Arc;

#[derive(Template)]
#[template(path = "watch.html")]
struct WatchTemplate {
    vod_id: String,
    vod_title: String,
    youtube_ids_json: String,
}

pub async fn watch_page(
    State(state): State<SharedState>,
    Path(vod_id): Path<String>,
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
            })
        }
        None => (axum::http::StatusCode::NOT_FOUND, "VOD not found").into_response(),
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
        (axum::http::StatusCode::NOT_FOUND, "vod not found").into_response()
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
