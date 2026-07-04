use super::{
    Headers, Listing, Pagination, Section, SortOption, VodDisplay, VodsGridTemplate,
    build_watch_url, find_vod_by_id, format_chapter_start, list_sort_options_grouped,
    render_template, resolve_watched_chapter,
};
use crate::SharedState;
use crate::middleware::CspNonce;
use crate::vods::Vod;
use askama::Template;
use axum::Json;
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

/// Wire contract with the client's history contract module
/// (static/lib/history-state.js); the shape is pinned on both sides by
/// tests/fixtures/history-request.json. Entries arrive most recently watched
/// first — request order is the recency contract.
#[derive(Deserialize)]
pub struct HistoryVodsRequest {
    entries: Vec<HistoryRequestedVod>,
    #[serde(default)]
    sort: Option<String>,
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

#[derive(Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum HistoryEntryState {
    InProgress,
    Watched,
}

#[derive(Deserialize)]
struct HistoryRequestedVod {
    id: String,
    state: HistoryEntryState,
    #[serde(default)]
    time: Option<i64>,
}

/// Clients legitimately send at most MAX_HISTORY_ENTRIES (1000 — see
/// static/lib/history-state.js) entries; anything beyond that is garbage
/// or abuse.
const MAX_HISTORY_ENTRIES: usize = 1000;

fn sanitize_history_requests(mut entries: Vec<HistoryRequestedVod>) -> Vec<HistoryRequestedVod> {
    entries.retain(|entry| !entry.id.is_empty());
    entries.truncate(MAX_HISTORY_ENTRIES);
    for entry in &mut entries {
        entry.time = entry.time.filter(|time| *time >= 0);
    }
    entries
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

    // Resolve each requested id to a catalog vod plus the chapter the user was
    // actually in at resume time (the series-header key), so multi-game streams
    // group by the watched game, not the first chapter. Kept in request order
    // (most recently watched first) unless the game sort reorders below.
    struct Entry<'a> {
        vod: &'a Vod,
        resume_time: Option<i64>,
        state: HistoryEntryState,
        chapter_start: Option<i64>,
        game_key: Option<String>,
    }
    let mut entries: Vec<Entry> = Vec::new();
    for requested in requested_vods {
        if let Some(&vod) = index.get(requested.id.as_str()) {
            let (game_key, chapter_start) = resolve_watched_chapter(vod, requested.time).unzip();
            entries.push(Entry {
                vod,
                resume_time: requested.time,
                state: requested.state,
                chapter_start,
                game_key,
            });
        }
    }

    // Order refs into their final display order BEFORE the listing builds them,
    // so the series headers are a plain run-length pass over contiguous games.
    if sort == Some("game") {
        entries.sort_by(|a, b| {
            let ak = a.game_key.as_deref().unwrap_or("").to_lowercase();
            let bk = b.game_key.as_deref().unwrap_or("").to_lowercase();
            ak.cmp(&bk)
                .then_with(|| b.vod.stream_time().cmp(a.vod.stream_time()))
        });
    }

    let headers = if options.show_headers {
        Headers::Series
    } else {
        Headers::None
    };

    Listing::build(&entries, Pagination::All, headers, |e| {
        let display = match e.state {
            HistoryEntryState::InProgress => {
                let link_start = if options.resume_links {
                    e.resume_time
                } else {
                    e.resume_time.or(e.chapter_start)
                };
                VodDisplay::in_progress(e.vod, link_start, e.resume_time, e.game_key.as_deref())
            }
            HistoryEntryState::Watched => {
                VodDisplay::watched(e.vod, e.chapter_start, e.game_key.as_deref())
            }
        };
        (display, e.game_key.clone())
    })
    .vods
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

    let catalog = Arc::clone(&*state.catalog.read().await);
    let vods = &catalog.vods;

    match build_continue_resume_view(vods, id, params.time) {
        Some(resume) => render_template(&ContinueResumeTemplate { resume }),
        None => StatusCode::NO_CONTENT.into_response(),
    }
}

pub async fn history_vods_grid(
    State(state): State<SharedState>,
    Json(body): Json<HistoryVodsRequest>,
) -> impl IntoResponse {
    let requested_vods = sanitize_history_requests(body.entries);

    let catalog = Arc::clone(&*state.catalog.read().await);
    let vods = &catalog.vods;

    let displays = build_history_displays(
        vods,
        &requested_vods,
        body.sort.as_deref(),
        HistoryDisplayOptions {
            resume_links: false,
            show_headers: true,
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

    fn requested(id: &str, time: Option<i64>, state: HistoryEntryState) -> HistoryRequestedVod {
        HistoryRequestedVod {
            id: id.to_string(),
            state,
            time,
        }
    }

    #[test]
    fn wire_fixture_parses_into_request() {
        let fixture = include_str!("../../tests/fixtures/history-request.json");
        let request: HistoryVodsRequest = serde_json::from_str(fixture).unwrap();

        assert_eq!(request.sort.as_deref(), Some("recent"));
        assert_eq!(request.entries.len(), 2);
        assert_eq!(request.entries[0].id, "recent");
        assert!(matches!(
            request.entries[0].state,
            HistoryEntryState::Watched
        ));
        assert_eq!(request.entries[0].time, None);
        assert_eq!(request.entries[1].id, "resume");
        assert!(matches!(
            request.entries[1].state,
            HistoryEntryState::InProgress
        ));
        assert_eq!(request.entries[1].time, Some(42));
    }

    #[test]
    fn sanitize_drops_empty_ids_negative_times_and_caps_entries() {
        let mut entries: Vec<HistoryRequestedVod> = (0..MAX_HISTORY_ENTRIES + 50)
            .map(|i| requested(&format!("v{i}"), Some(10), HistoryEntryState::InProgress))
            .collect();
        entries[0].id = String::new();
        entries[1].time = Some(-5);

        let sanitized = sanitize_history_requests(entries);

        assert_eq!(sanitized.len(), MAX_HISTORY_ENTRIES);
        assert_eq!(sanitized[0].id, "v1");
        assert_eq!(sanitized[0].time, None);
        assert_eq!(sanitized[1].time, Some(10));
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
    fn in_progress_without_resume_time_has_no_progress_or_time_suffix() {
        let vods = vec![make_history_vod()];

        let displays = build_history_displays(
            &vods,
            &[requested("v1", None, HistoryEntryState::InProgress)],
            None,
            HistoryDisplayOptions {
                resume_links: false,
                show_headers: true,
            },
        );

        assert_eq!(displays[0].status_label.as_deref(), Some("In progress"));
        assert_eq!(displays[0].progress_seconds, None);
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
