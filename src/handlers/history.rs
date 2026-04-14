use super::{
    Section, VodDisplay, assign_series_headers, render_template, resolve_watched_chapter,
};
use crate::SharedState;
use askama::Template;
use axum::extract::{Query, State};
use axum::response::IntoResponse;
use serde::Deserialize;
use std::sync::Arc;

#[derive(Template)]
#[template(path = "history.html")]
struct HistoryTemplate {
    active_section: Section,
}

pub async fn history_page() -> impl IntoResponse {
    render_template(&HistoryTemplate {
        active_section: Section::History,
    })
}

#[derive(Deserialize)]
pub struct HistoryVodsQuery {
    pub ids: Option<String>,
    pub times: Option<String>,
}

#[derive(Template)]
#[template(path = "vods_grid.html")]
struct VodsGridTemplate {
    vods: Vec<VodDisplay>,
    has_more: bool,
    next_url: String,
    show_game_tags: bool,
}

pub async fn history_vods_grid(
    State(state): State<SharedState>,
    Query(params): Query<HistoryVodsQuery>,
) -> impl IntoResponse {
    let ids_str = params.ids.unwrap_or_default();
    let requested_ids: Vec<&str> = ids_str.split(',').filter(|s| !s.is_empty()).collect();

    let times_str = params.times.unwrap_or_default();
    let resume_times: Vec<Option<i64>> = times_str
        .split(',')
        .map(|s| s.parse::<i64>().ok())
        .collect();

    let vods = {
        let guard = state.vods.read().await;
        Arc::clone(&*guard)
    };

    // Build displays in the order of requested IDs (most recently watched first).
    // For each one, resolve the chapter the user was actually in at resume time
    // so multi-game streams group by the watched game, not the first chapter.
    let mut displays = Vec::new();
    let mut keys: Vec<Option<String>> = Vec::new();
    for (i, id) in requested_ids.iter().enumerate() {
        if let Some(vod) = vods.iter().find(|v| v.id == *id) {
            let time = resume_times.get(i).copied().flatten();
            let (name_opt, start_opt) = match (time, resolve_watched_chapter(vod, time)) {
                (Some(_), Some((name, start))) => (Some(name), Some(start)),
                _ => (None, None),
            };
            let display = VodDisplay::from_vod_with(vod, start_opt, name_opt.as_deref());
            displays.push(display);
            keys.push(name_opt);
        }
    }

    assign_series_headers(&mut displays, &keys);

    render_template(&VodsGridTemplate {
        vods: displays,
        has_more: false,
        next_url: String::new(),
        show_game_tags: true,
    })
}
