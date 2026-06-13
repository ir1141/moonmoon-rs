use super::{
    Section, SortOption, VodDisplay, VodsGridTemplate, assign_series_headers, build_watch_url,
    find_vod_by_id, format_chapter_start, list_sort_options_grouped, render_template,
    resolve_watched_chapter,
};
use crate::SharedState;
use crate::middleware::CspNonce;
use crate::vods::Vod;
use askama::Template;
use axum::extract::{Extension, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Deserialize;
use std::sync::Arc;

const HISTORY_SORTS: [(&str, &str, bool); 2] = [
    ("recent", "Most recently watched", false),
    ("game", "By game", false),
];

#[derive(Template)]
#[template(path = "history.html")]
struct HistoryTemplate {
    active_section: Section,
    nonce: String,
    sort_options: Vec<SortOption>,
    selected_sort_label: &'static str,
    sort_aria_label: &'static str,
}

pub async fn history_page(Extension(nonce): Extension<CspNonce>) -> impl IntoResponse {
    render_template(&HistoryTemplate {
        active_section: Section::History,
        nonce: nonce.0,
        sort_options: list_sort_options_grouped("recent", &HISTORY_SORTS),
        selected_sort_label: "Most recently watched",
        sort_aria_label: "Sort history",
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
#[template(path = "continue_resume.html")]
struct ContinueResumeTemplate {
    resume: ContinueResumeView,
}

struct ContinueResumeView {
    title: String,
    game_name: String,
    formatted_date: String,
    duration: Option<String>,
    thumbnail_url: Option<String>,
    resume_url: String,
    start_url: String,
    progress_pct: String,
    subline: String,
}

#[derive(Deserialize)]
pub struct ContinueResumeQuery {
    pub id: Option<String>,
    pub time: Option<i64>,
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

/// Clients legitimately send at most MAX_RESUME_ENTRIES + MAX_WATCHED_ENTRIES
/// (500 + 500) ids; anything beyond that is garbage or abuse.
const MAX_HISTORY_IDS: usize = 1000;

fn parse_history_requests(
    ids: &str,
    times: &str,
    states: Option<&str>,
) -> Vec<HistoryRequestedVod> {
    let time_values: Vec<&str> = times.split(',').collect();
    let state_values: Vec<&str> = states.unwrap_or("").split(',').collect();

    ids.split(',')
        .enumerate()
        .filter(|(_, id)| !id.is_empty())
        .take(MAX_HISTORY_IDS)
        .map(|(idx, id)| {
            // idx indexes the original CSV position, so times/states stay
            // aligned even when an id slot is empty.
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
    use std::collections::HashMap;
    // One pass over the catalog instead of a linear scan per requested id.
    // First-insert-wins matches find_vod_by_id's first-match semantics.
    let mut index: HashMap<&str, &Vod> = HashMap::with_capacity(vods.len() * 2);
    for vod in vods {
        index.entry(vod.id.as_str()).or_insert(vod);
        if let Some(pid) = vod.platform_vod_id.as_deref() {
            index.entry(pid).or_insert(vod);
        }
    }

    // Build displays in the order of requested IDs (most recently watched first).
    // For each one, resolve the chapter the user was actually in at resume time
    // so multi-game streams group by the watched game, not the first chapter.
    let mut displays = Vec::new();
    let mut keys: Vec<Option<String>> = Vec::new();
    for requested in requested_vods {
        if let Some(&vod) = index.get(requested.id.as_str()) {
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
                            Some(format!("In progress · {}", format_chapter_start(time)));
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

fn format_continue_remaining(position: i64, duration: i64) -> Option<String> {
    if duration <= 0 {
        return None;
    }

    let remaining = (duration - position).max(0);
    let hours = remaining / 3600;
    let minutes = (remaining % 3600) / 60;

    if hours > 0 {
        Some(format!("{hours}h {minutes}m left"))
    } else if minutes > 0 {
        Some(format!("{minutes}m left"))
    } else {
        Some("Less than a minute left".to_string())
    }
}

fn build_continue_resume_view(
    vods: &[Vod],
    requested_id: &str,
    resume_time: Option<i64>,
) -> Option<ContinueResumeView> {
    let resume_time = resume_time.filter(|time| *time > 10)?;
    let vod = find_vod_by_id(vods, requested_id)?;
    let resolved_game = resolve_watched_chapter(vod, Some(resume_time));
    let game_name = resolved_game
        .as_ref()
        .map(|(name, _)| name.clone())
        .unwrap_or_else(|| "Stream".to_string());
    let game_hint = resolved_game.as_ref().map(|(name, _)| name.as_str());
    let display = VodDisplay::from_vod_with(vod, Some(resume_time), game_hint);
    let progress = if display.duration_seconds > 0 {
        ((resume_time as f64 / display.duration_seconds as f64) * 100.0).clamp(0.0, 100.0)
    } else {
        0.0
    };
    let resume_at = format!("resumes at {}", format_chapter_start(resume_time));
    let subline = match format_continue_remaining(resume_time, display.duration_seconds) {
        Some(remaining) => format!("{remaining} · {resume_at}"),
        None => resume_at,
    };

    Some(ContinueResumeView {
        title: display.display_title,
        game_name,
        formatted_date: display.formatted_date,
        duration: display.duration,
        thumbnail_url: display.thumbnail_url,
        resume_url: display.watch_url,
        start_url: build_watch_url(&display.id, Some(0), None),
        progress_pct: format!("{progress:.2}"),
        subline,
    })
}

pub async fn continue_resume(
    State(state): State<SharedState>,
    Query(params): Query<ContinueResumeQuery>,
) -> axum::response::Response {
    let Some(id) = params.id.as_deref().filter(|id| !id.is_empty()) else {
        return StatusCode::NO_CONTENT.into_response();
    };

    let vods = {
        let guard = state.vods.read().await;
        Arc::clone(&*guard)
    };

    match build_continue_resume_view(&vods, id, params.time) {
        Some(resume) => render_template(&ContinueResumeTemplate { resume }),
        None => StatusCode::NO_CONTENT.into_response(),
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
        show_subtitle: true,
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
    fn continue_resume_view_uses_explicit_server_fields() {
        let vods = vec![make_history_vod()];

        let resume = build_continue_resume_view(&vods, "legacy-v1", Some(5000)).unwrap();

        assert_eq!(resume.title, "Test stream");
        assert_eq!(resume.game_name, "Terraria");
        assert_eq!(resume.resume_url, "/watch/v1?t=5000&game=Terraria");
        assert_eq!(resume.start_url, "/watch/v1?t=0");
        assert_eq!(resume.progress_pct, "69.44");
        assert_eq!(resume.subline, "36m left · resumes at 1:23:20");
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
    fn parse_history_requests_keeps_times_aligned_past_empty_ids() {
        let parsed = parse_history_requests("a,,b", "10,20,30", Some("in_progress,,watched"));
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].id, "a");
        assert_eq!(parsed[0].resume_time, Some(10));
        assert_eq!(parsed[1].id, "b");
        assert_eq!(parsed[1].resume_time, Some(30));
        assert!(matches!(parsed[1].state, HistoryEntryState::Watched));
    }

    #[test]
    fn parse_history_requests_caps_requested_ids() {
        let ids = vec!["x"; MAX_HISTORY_IDS + 50].join(",");
        let parsed = parse_history_requests(&ids, "", None);
        assert_eq!(parsed.len(), MAX_HISTORY_IDS);
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
