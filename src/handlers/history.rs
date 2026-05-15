use super::{
    Section, VodDisplay, assign_series_headers, find_vod_by_id, render_template,
    resolve_watched_chapter,
};
use crate::SharedState;
use crate::middleware::CspNonce;
use crate::vods::Vod;
use askama::Template;
use axum::extract::{Extension, Query, State};
use axum::response::IntoResponse;
use serde::Deserialize;
use std::sync::Arc;

#[derive(Template)]
#[template(path = "history.html")]
struct HistoryTemplate {
    active_section: Section,
    nonce: String,
}

pub async fn history_page(Extension(nonce): Extension<CspNonce>) -> impl IntoResponse {
    render_template(&HistoryTemplate {
        active_section: Section::History,
        nonce: nonce.0,
    })
}

#[derive(Deserialize)]
pub struct HistoryVodsQuery {
    pub ids: Option<String>,
    pub times: Option<String>,
    pub sort: Option<String>,
    pub resume_links: Option<bool>,
    pub headers: Option<bool>,
}

#[derive(Template)]
#[template(path = "vods_grid.html")]
struct VodsGridTemplate {
    vods: Vec<VodDisplay>,
    has_more: bool,
    next_url: String,
    show_game_tags: bool,
}

#[derive(Clone, Copy)]
struct HistoryDisplayOptions {
    resume_links: bool,
    show_headers: bool,
}

fn build_history_displays(
    vods: &[Vod],
    requested_ids: &[&str],
    resume_times: &[Option<i64>],
    sort: Option<&str>,
    options: HistoryDisplayOptions,
) -> Vec<VodDisplay> {
    // Build displays in the order of requested IDs (most recently watched first).
    // For each one, resolve the chapter the user was actually in at resume time
    // so multi-game streams group by the watched game, not the first chapter.
    let mut displays = Vec::new();
    let mut keys: Vec<Option<String>> = Vec::new();
    for (i, id) in requested_ids.iter().enumerate() {
        if let Some(vod) = find_vod_by_id(vods, id) {
            let time = resume_times.get(i).copied().flatten();
            let (name_opt, start_opt) = time
                .and_then(|t| resolve_watched_chapter(vod, Some(t)))
                .unzip();
            let link_start = if options.resume_links {
                time
            } else {
                start_opt
            };
            let display = VodDisplay::from_vod_with(vod, link_start, name_opt.as_deref());
            displays.push(display);
            keys.push(name_opt);
        }
    }

    if sort == Some("game") {
        let mut paired: Vec<(VodDisplay, Option<String>)> =
            displays.into_iter().zip(keys).collect();
        paired.sort_by(|a, b| {
            let ak = a.1.as_deref().unwrap_or("").to_lowercase();
            let bk = b.1.as_deref().unwrap_or("").to_lowercase();
            ak.cmp(&bk)
                .then_with(|| b.0.created_at.cmp(&a.0.created_at))
        });
        (displays, keys) = paired.into_iter().unzip();
    }

    if options.show_headers {
        assign_series_headers(&mut displays, &keys);
    }

    displays
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

    let displays = build_history_displays(
        &vods,
        &requested_ids,
        &resume_times,
        params.sort.as_deref(),
        HistoryDisplayOptions {
            resume_links: params.resume_links.unwrap_or(false),
            show_headers: params.headers.unwrap_or(true),
        },
    );

    render_template(&VodsGridTemplate {
        vods: displays,
        has_more: false,
        next_url: String::new(),
        show_game_tags: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vods::{Chapter, Vod};

    fn make_history_vod() -> Vod {
        Vod {
            id: "v1".into(),
            platform: None,
            platform_vod_id: Some("legacy-v1".into()),
            platform_stream_id: None,
            title: Some("Test stream".into()),
            created_at: "2026-05-01T00:00:00Z".into(),
            started_at: None,
            updated_at: None,
            duration: Some("2h".into()),
            thumbnail_url: None,
            chapters: Some(vec![
                Chapter {
                    name: Some("Just Chatting".into()),
                    image: None,
                    start: Some(0.0),
                    duration: None,
                    end: None,
                },
                Chapter {
                    name: Some("Terraria".into()),
                    image: None,
                    start: Some(3600.0),
                    duration: None,
                    end: None,
                },
            ]),
            youtube: None,
            is_live: false,
        }
    }

    #[test]
    fn history_displays_default_to_watched_chapter_start_links() {
        let vods = vec![make_history_vod()];

        let displays = build_history_displays(
            &vods,
            &["v1"],
            &[Some(5000)],
            None,
            HistoryDisplayOptions {
                resume_links: false,
                show_headers: true,
            },
        );

        assert_eq!(displays[0].watch_url, "/watch/v1?t=3600&game=Terraria");
        assert_eq!(
            displays[0].period_header.as_deref(),
            Some("Terraria · 1 stream")
        );
    }

    #[test]
    fn history_displays_can_link_to_exact_resume_time() {
        let vods = vec![make_history_vod()];

        let displays = build_history_displays(
            &vods,
            &["legacy-v1"],
            &[Some(5000)],
            None,
            HistoryDisplayOptions {
                resume_links: true,
                show_headers: true,
            },
        );

        assert_eq!(displays[0].id, "v1");
        assert_eq!(displays[0].watch_url, "/watch/v1?t=5000&game=Terraria");
    }

    #[test]
    fn history_displays_can_suppress_series_headers() {
        let vods = vec![make_history_vod()];

        let displays = build_history_displays(
            &vods,
            &["v1"],
            &[Some(5000)],
            None,
            HistoryDisplayOptions {
                resume_links: true,
                show_headers: false,
            },
        );

        assert!(displays[0].period_header.is_none());
    }
}
