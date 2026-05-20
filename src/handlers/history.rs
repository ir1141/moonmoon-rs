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
    pub states: Option<String>,
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
    is_filtered: bool,
}

#[derive(Clone, Copy)]
struct HistoryDisplayOptions {
    resume_links: bool,
    show_headers: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum HistoryEntryState {
    InProgress,
    Watched,
}

impl HistoryEntryState {
    fn from_query(value: &str) -> Option<Self> {
        match value {
            "in_progress" => Some(Self::InProgress),
            "watched" => Some(Self::Watched),
            _ => None,
        }
    }

    fn as_str(&self) -> &'static str {
        match self {
            Self::InProgress => "in_progress",
            Self::Watched => "watched",
        }
    }
}

struct HistoryRequestedVod {
    id: String,
    resume_time: Option<i64>,
    state: HistoryEntryState,
}

fn parse_history_requests(
    ids: &str,
    times: &str,
    states: Option<&str>,
) -> Vec<HistoryRequestedVod> {
    let time_values: Vec<&str> = times.split(',').collect();
    let state_values: Vec<&str> = states.unwrap_or("").split(',').collect();

    ids.split(',')
        .filter(|id| !id.is_empty())
        .enumerate()
        .map(|(idx, id)| {
            let resume_time = time_values
                .get(idx)
                .and_then(|time| time.parse::<i64>().ok())
                .filter(|time| *time >= 0);
            let state = state_values
                .get(idx)
                .and_then(|state| HistoryEntryState::from_query(state))
                .unwrap_or(HistoryEntryState::InProgress);

            HistoryRequestedVod {
                id: id.to_string(),
                resume_time,
                state,
            }
        })
        .collect()
}

fn build_history_displays(
    vods: &[Vod],
    requested_vods: &[HistoryRequestedVod],
    sort: Option<&str>,
    options: HistoryDisplayOptions,
) -> Vec<VodDisplay> {
    // Build displays in the order of requested IDs (most recently watched first).
    // For each one, resolve the chapter the user was actually in at resume time
    // so multi-game streams group by the watched game, not the first chapter.
    let mut displays = Vec::new();
    let mut keys: Vec<Option<String>> = Vec::new();
    for requested in requested_vods {
        if let Some(vod) = find_vod_by_id(vods, &requested.id) {
            let (name_opt, start_opt) = resolve_watched_chapter(vod, requested.resume_time).unzip();
            let mut display = match requested.state {
                HistoryEntryState::InProgress => {
                    let link_start = if options.resume_links {
                        requested.resume_time
                    } else {
                        requested.resume_time.or(start_opt)
                    };
                    let mut display =
                        VodDisplay::from_vod_with(vod, link_start, name_opt.as_deref());
                    if let Some(time) = requested.resume_time {
                        display.status_label =
                            Some(format!("In progress · {}", format_history_time(time)));
                        display.progress_seconds = Some(time);
                    } else {
                        display.status_label = Some("In progress".to_string());
                    }
                    display.history_state = Some(requested.state.as_str());
                    display
                }
                HistoryEntryState::Watched => {
                    let mut display =
                        VodDisplay::from_vod_with(vod, start_opt, name_opt.as_deref());
                    display.status_label = Some("Watched".to_string());
                    display.history_state = Some(requested.state.as_str());
                    display
                }
            };
            display.match_label = None;
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

fn format_history_time(seconds: i64) -> String {
    let seconds = seconds.max(0);
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{secs:02}")
    } else {
        format!("{minutes}:{secs:02}")
    }
}

pub async fn history_vods_grid(
    State(state): State<SharedState>,
    Query(params): Query<HistoryVodsQuery>,
) -> impl IntoResponse {
    let ids_str = params.ids.unwrap_or_default();
    let times_str = params.times.unwrap_or_default();
    let requested_vods = parse_history_requests(&ids_str, &times_str, params.states.as_deref());

    let vods = {
        let guard = state.vods.read().await;
        Arc::clone(&*guard)
    };

    let displays = build_history_displays(
        &vods,
        &requested_vods,
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
        is_filtered: false,
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
            &[requested("v1", Some(5000), HistoryEntryState::Watched)],
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
            &[requested(
                "legacy-v1",
                Some(5000),
                HistoryEntryState::InProgress,
            )],
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
            &[requested("v1", Some(5000), HistoryEntryState::InProgress)],
            None,
            HistoryDisplayOptions {
                resume_links: true,
                show_headers: false,
            },
        );

        assert!(displays[0].period_header.is_none());
    }

    fn requested(
        id: &str,
        resume_time: Option<i64>,
        state: HistoryEntryState,
    ) -> HistoryRequestedVod {
        HistoryRequestedVod {
            id: id.to_string(),
            resume_time,
            state,
        }
    }

    #[test]
    fn watched_only_history_entries_render_instead_of_disappearing() {
        let vods = vec![make_history_vod()];

        let displays = build_history_displays(
            &vods,
            &[requested("v1", None, HistoryEntryState::Watched)],
            None,
            HistoryDisplayOptions {
                resume_links: false,
                show_headers: true,
            },
        );

        assert_eq!(displays.len(), 1);
        assert_eq!(displays[0].status_label.as_deref(), Some("Watched"));
        assert_eq!(displays[0].watch_url, "/watch/v1?t=0&game=Just%20Chatting");
    }

    #[test]
    fn in_progress_history_entries_link_to_exact_resume_time() {
        let vods = vec![make_history_vod()];

        let displays = build_history_displays(
            &vods,
            &[requested("v1", Some(5000), HistoryEntryState::InProgress)],
            None,
            HistoryDisplayOptions {
                resume_links: false,
                show_headers: true,
            },
        );

        assert_eq!(displays[0].watch_url, "/watch/v1?t=5000&game=Terraria");
        assert_eq!(
            displays[0].status_label.as_deref(),
            Some("In progress · 1:23:20")
        );
        assert_eq!(displays[0].progress_seconds, Some(5000));
    }

    #[test]
    fn recent_history_preserves_mixed_order_from_client() {
        let mut second = make_history_vod();
        second.id = "v2".into();
        second.platform_vod_id = None;
        second.title = Some("Second stream".into());
        let vods = vec![make_history_vod(), second];

        let displays = build_history_displays(
            &vods,
            &[
                requested("v2", None, HistoryEntryState::Watched),
                requested("v1", Some(5000), HistoryEntryState::InProgress),
            ],
            Some("recent"),
            HistoryDisplayOptions {
                resume_links: false,
                show_headers: false,
            },
        );

        assert_eq!(
            displays
                .iter()
                .map(|display| display.id.as_str())
                .collect::<Vec<_>>(),
            vec!["v2", "v1"]
        );
    }

    #[test]
    fn game_sorted_history_groups_by_resolved_chapter() {
        let mut first = make_history_vod();
        first.id = "first".into();
        let mut second = make_history_vod();
        second.id = "second".into();
        let vods = vec![first, second];

        let displays = build_history_displays(
            &vods,
            &[
                requested("first", Some(5000), HistoryEntryState::InProgress),
                requested("second", Some(100), HistoryEntryState::InProgress),
            ],
            Some("game"),
            HistoryDisplayOptions {
                resume_links: false,
                show_headers: true,
            },
        );

        assert_eq!(displays[0].id, "second");
        assert_eq!(
            displays[0].period_header.as_deref(),
            Some("Just Chatting · 1 stream")
        );
        assert_eq!(displays[1].id, "first");
        assert_eq!(
            displays[1].period_header.as_deref(),
            Some("Terraria · 1 stream")
        );
    }
}
