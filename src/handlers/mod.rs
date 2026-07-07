mod api;
mod browse;
mod calendar;
mod emotes;
mod history;
mod home;
mod listing;
mod sync;
mod watch;

pub use api::{chat_proxy, refresh_catalog};
pub use browse::{browse_grid, browse_page, game_redirect, games_redirect, streams_redirect};
pub use calendar::calendar_page;
pub use emotes::{channel_emotes, lookup_emote, vod_emotes};
pub use history::{continue_resume, history_page, history_vods_grid};
pub use home::home_page;
pub(crate) use listing::{Headers, Listing, Pagination};
pub use sync::{sync_get, sync_put};
pub use watch::{next_in_period, random_vod, vod_detail, watch_page};

use crate::dates::{
    current_utc_days, date_query_for_days, days_in_month, days_to_civil, parse_ymd_to_days,
};
use crate::vods::month_abbr;
use crate::vods::{Game, Vod};
use askama::Template;
use axum::response::{Html, IntoResponse};
use serde::Deserialize;

pub(crate) const VOD_BATCH_SIZE: usize = 36;
pub(crate) const GAME_BATCH_SIZE: usize = 60;
pub(crate) const PERIOD_GAP_DAYS: i64 = 14;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Section {
    None,
    Home,
    Browse,
    History,
    Calendar,
}

impl Section {
    pub(crate) fn slug(&self) -> &'static str {
        match self {
            Section::None => "",
            Section::Home => "home",
            Section::Browse => "browse",
            Section::History => "history",
            Section::Calendar => "calendar",
        }
    }
}

pub(crate) fn vod_matches_id(vod: &Vod, requested_id: &str) -> bool {
    vod.id == requested_id || vod.platform_vod_id.as_deref() == Some(requested_id)
}

pub(crate) fn find_vod_by_id<'a>(vods: &'a [Vod], requested_id: &str) -> Option<&'a Vod> {
    vods.iter().find(|vod| vod_matches_id(vod, requested_id))
}

// ─── Query types ───

#[derive(Deserialize, Default)]
pub struct ListQuery {
    pub search: Option<String>,
    pub sort: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub page: Option<usize>,
    pub lens: Option<String>,
    pub game: Option<String>,
}

// ─── Display types ───

pub(crate) struct ActiveFilter {
    pub label: String,
}

pub(crate) struct ListMetadata {
    pub unfiltered_count: usize,
    pub filtered_count: usize,
    pub is_filtered: bool,
    pub active_filters: Vec<ActiveFilter>,
    pub clear_url: String,
    pub result_label: String,
}

pub(crate) struct SortOption {
    pub value: &'static str,
    pub label: &'static str,
    pub selected: bool,
    pub separator_before: bool,
}

pub(crate) struct ListFilterConfig {
    pub form_id: &'static str,
    pub toolbar_class: &'static str,
    pub action: String,
    pub title: &'static str,
    pub search_placeholder: &'static str,
    pub search_label: String,
    pub sort_label: &'static str,
    pub hx_get: String,
    pub results_id: &'static str,
    pub loading_id: &'static str,
    pub sort_options: Vec<SortOption>,
    pub selected_sort_value: &'static str,
    pub selected_sort_label: &'static str,
    pub archive_min_date: String,
    pub archive_max_date: String,
    pub date_preset: &'static str,
    pub show_custom_dates: bool,
}

pub(crate) fn list_sort_options_grouped(
    selected: &str,
    options: &[(&'static str, &'static str, bool)],
) -> Vec<SortOption> {
    options
        .iter()
        .map(|(value, label, separator_before)| SortOption {
            value,
            label,
            selected: *value == selected,
            separator_before: *separator_before,
        })
        .collect()
}

pub(crate) fn selected_sort_option(
    selected: &str,
    options: &[(&'static str, &'static str, bool)],
) -> (&'static str, &'static str) {
    options
        .iter()
        .find(|(value, _, _)| *value == selected)
        .or_else(|| options.first())
        .map(|(value, label, _)| (*value, *label))
        .unwrap_or(("", ""))
}

pub(crate) struct DatePresetState {
    pub active: &'static str,
    pub show_custom: bool,
}

pub(crate) struct FilteredGames {
    pub games: Vec<Game>,
    pub metadata: ListMetadata,
}

/// Grid-only partial for the streams/history lists. Rendered standalone for
/// htmx "load more" swaps and `{% include %}`d by the full-page templates.
/// Shared by `browse` and `history` so the field set stays in lockstep.
#[derive(Template)]
#[template(path = "vods_grid.html")]
pub(crate) struct VodsGridTemplate {
    pub vods: Vec<VodDisplay>,
    pub has_more: bool,
    pub next_url: String,
    pub show_game_tags: bool,
    pub show_subtitle: bool,
    pub is_filtered: bool,
}

/// Grid-only partial for the games lens (counterpart to [`VodsGridTemplate`]).
#[derive(Template)]
#[template(path = "games_grid.html")]
pub(crate) struct GamesGridTemplate {
    pub games: Vec<Game>,
    pub has_more: bool,
    pub next_url: String,
    pub show_recency: bool,
    pub show_oldest_recency: bool,
    pub is_filtered: bool,
}

pub(crate) struct ChapterSegment {
    pub name: String,
    pub width_pct: f64,
    pub watch_url: String,
    pub color_idx: u8,
    pub start_label: String,
    /// Clamped start offset (seconds) of this segment within the stream.
    pub start_secs: i64,
    pub duration_secs: i64,
}

pub(crate) struct VodDisplay {
    pub id: String,
    pub display_title: String,
    pub formatted_date: String,
    /// Year-less short date for the streams-lens subtitle (e.g. "Mar 14");
    /// under date sorts the year is carried by the month-group header above
    /// the card (other sorts render no year).
    pub formatted_date_short: String,
    pub duration: Option<String>,
    pub thumbnail_url: Option<String>,
    pub chapter_segments: Vec<ChapterSegment>,
    pub created_at: String,
    pub match_label: Option<String>,
    pub status_label: Option<String>,
    pub progress_seconds: Option<i64>,
    pub history_state: Option<&'static str>,
    pub chapter_names: Vec<String>,
    pub duration_seconds: i64,
    /// Set by the listing module's header pass ([`super::listing`]): a Period
    /// (month) or Series (game) header, never both — the [`Headers`] mode picks
    /// exactly one.
    pub period_header: Option<String>,
    pub watch_url: String,
}

impl VodDisplay {
    pub(crate) fn from_vod(vod: &Vod) -> Self {
        Self::from_vod_with(vod, None, None)
    }

    pub(crate) fn from_vod_with(
        vod: &Vod,
        chapter_start: Option<i64>,
        game_name_hint: Option<&str>,
    ) -> Self {
        let display_title = vod
            .title
            .clone()
            .unwrap_or_else(|| "Untitled Stream".to_string());
        let stream_time = vod.stream_time();
        let formatted_date = format_date(stream_time);
        let formatted_date_short = format_date_short(stream_time);
        let duration_seconds = vod
            .duration
            .as_ref()
            .map_or(0, |duration| duration.seconds());
        let watch_url = build_watch_url(&vod.id, chapter_start, game_name_hint);
        let chapter_segments = get_chapter_segments(vod);
        let chapter_names = get_game_tags(vod);
        VodDisplay {
            id: vod.id.clone(),
            display_title,
            formatted_date,
            formatted_date_short,
            duration: vod
                .duration
                .as_ref()
                .map(|duration| duration.display().to_string()),
            thumbnail_url: vod.thumbnail_url.clone(),
            chapter_segments,
            created_at: stream_time.to_string(),
            match_label: None,
            status_label: None,
            progress_seconds: None,
            history_state: None,
            chapter_names,
            duration_seconds,
            period_header: None,
            watch_url,
        }
    }

    /// In-progress history card: `progress_seconds` and the label's `· {time}`
    /// suffix are present together exactly when `resume_time` is `Some`.
    /// `link_start` is the already-resolved watch-url start; the
    /// resume-vs-chapter choice belongs to the caller.
    pub(crate) fn in_progress(
        vod: &Vod,
        link_start: Option<i64>,
        resume_time: Option<i64>,
        game_name_hint: Option<&str>,
    ) -> Self {
        let mut display = Self::from_vod_with(vod, link_start, game_name_hint);
        display.status_label = Some(match resume_time {
            Some(time) => format!("In progress · {}", format_chapter_start(time)),
            None => "In progress".to_string(),
        });
        display.progress_seconds = resume_time;
        display.history_state = Some("in_progress");
        display
    }

    /// Watched history card.
    pub(crate) fn watched(
        vod: &Vod,
        chapter_start: Option<i64>,
        game_name_hint: Option<&str>,
    ) -> Self {
        let mut display = Self::from_vod_with(vod, chapter_start, game_name_hint);
        display.status_label = Some("Watched".to_string());
        display.history_state = Some("watched");
        display
    }
}

pub(crate) fn build_watch_url(
    vod_id: &str,
    chapter_start: Option<i64>,
    game_name_hint: Option<&str>,
) -> String {
    let mut url = format!("/watch/{vod_id}");
    let mut parts: Vec<String> = Vec::new();
    if let Some(t) = chapter_start {
        parts.push(format!("t={t}"));
    }
    if let Some(g) = game_name_hint
        && !g.is_empty()
    {
        parts.push(format!("game={}", urlencoding::encode(g)));
    }
    if !parts.is_empty() {
        url.push('?');
        url.push_str(&parts.join("&"));
    }
    url
}

// ─── Render helper ───

pub(crate) fn render_template(tmpl: &impl Template) -> axum::response::Response {
    match tmpl.render() {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
            tracing::error!("template render failed: {e}");
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Html("Internal server error".to_string()),
            )
                .into_response()
        }
    }
}

// ─── Helpers ───

pub(crate) fn filter_games_with_metadata(
    games: &[Game],
    vods: &[Vod],
    params: &ListQuery,
    clear_base_url: &str,
    sort: &str,
) -> FilteredGames {
    let unfiltered_count = games.len();
    let filtered = if list_date_filter_is_active(params) {
        filter_and_sort_games(
            crate::vods::build_dominant_games(
                vods.iter()
                    .filter(|vod| vod_matches_date_filter(vod, params)),
            ),
            &params.search,
            sort,
        )
    } else {
        filter_and_sort_games(games.to_vec(), &params.search, sort)
    };

    let filtered_count = filtered.len();
    let metadata = build_list_metadata_for_kind(
        unfiltered_count,
        filtered_count,
        params,
        clear_base_url,
        "game",
        "games",
    );

    FilteredGames {
        games: filtered,
        metadata,
    }
}

fn filter_and_sort_games(
    mut filtered: Vec<Game>,
    search: &Option<String>,
    sort: &str,
) -> Vec<Game> {
    if let Some(search) = normalized_filter_value(search) {
        let search_lower = search.to_lowercase();
        filtered.retain(|g| g.name.to_lowercase().contains(&search_lower));
    }

    match sort {
        "fewest" | "streams_asc" => filtered.sort_by_key(|a| a.vod_count),
        "most" | "streams_desc" => filtered.sort_by_key(|a| std::cmp::Reverse(a.vod_count)),
        "az" => filtered.sort_by_key(|a| a.name.to_lowercase()),
        "za" => filtered.sort_by_key(|a| std::cmp::Reverse(a.name.to_lowercase())),
        "oldest" => sort_games_by_first_streamed(&mut filtered),
        _ => sort_games_by_latest_streamed(&mut filtered),
    }

    filtered
}

fn sort_games_by_latest_streamed(games: &mut [Game]) {
    games.sort_by(
        |a, b| match (a.last_streamed.as_ref(), b.last_streamed.as_ref()) {
            (Some(left), Some(right)) => right
                .cmp(left)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        },
    );
}

fn sort_games_by_first_streamed(games: &mut [Game]) {
    games.sort_by(
        |a, b| match (a.first_streamed.as_ref(), b.first_streamed.as_ref()) {
            (Some(left), Some(right)) => left
                .cmp(right)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        },
    );
}

fn list_date_filter_is_active(params: &ListQuery) -> bool {
    normalized_filter_value(&params.from).is_some() || normalized_filter_value(&params.to).is_some()
}

pub(crate) fn date_preset_state(
    from: &Option<String>,
    to: &Option<String>,
    archive_min_date: &str,
    archive_max_date: &str,
) -> DatePresetState {
    let from = normalized_filter_value(from);
    let to = normalized_filter_value(to);
    if from.is_none() && to.is_none() {
        return DatePresetState {
            active: "all",
            show_custom: false,
        };
    }

    let today = bounded_preset_today(archive_min_date, archive_max_date);
    for preset in ["30", "90", "year"] {
        let (preset_from, preset_to) =
            preset_date_range(preset, today, archive_min_date, archive_max_date);
        if from.as_deref() == Some(preset_from.as_str())
            && to.as_deref() == Some(preset_to.as_str())
        {
            return DatePresetState {
                active: preset,
                show_custom: false,
            };
        }
    }

    DatePresetState {
        active: "custom",
        show_custom: true,
    }
}

fn bounded_preset_today(archive_min_date: &str, archive_max_date: &str) -> i64 {
    let today = current_utc_days();
    let min_days = parse_ymd_to_days(archive_min_date).unwrap_or(today);
    let max_days = parse_ymd_to_days(archive_max_date).unwrap_or(today);
    today.clamp(min_days, max_days)
}

fn preset_date_range(
    preset: &str,
    today: i64,
    archive_min_date: &str,
    archive_max_date: &str,
) -> (String, String) {
    let min_days = parse_ymd_to_days(archive_min_date).unwrap_or(today);
    let max_days = parse_ymd_to_days(archive_max_date).unwrap_or(today);
    let start = match preset {
        "30" => today - 30,
        "90" => today - 90,
        "year" => {
            let (year, _, _) = days_to_civil(today);
            parse_ymd_to_days(&format!("{year:04}-01-01")).unwrap_or(today)
        }
        _ => today,
    }
    .clamp(min_days, max_days);
    let end = today.clamp(min_days, max_days);
    (date_query_for_days(start), date_query_for_days(end))
}

fn vod_matches_date_filter(vod: &Vod, params: &ListQuery) -> bool {
    let stream_time = vod.stream_time();
    let date = stream_time.get(..10).unwrap_or(stream_time);

    if let Some(from) = date_filter_lower_bound(&params.from)
        && date < from.as_str()
    {
        return false;
    }

    if let Some(to) = date_filter_upper_bound(&params.to)
        && date > to.as_str()
    {
        return false;
    }

    true
}

/// A catalog VOD that survived filtering, plus its search-match label.
/// Replaces the old build-every-VodDisplay-then-filter flow: displays (chapter
/// segments, tags, formatted labels) are only built for the paginated slice.
pub(crate) struct VodRefMatch<'a> {
    pub vod: &'a Vod,
    pub match_label: Option<String>,
}

pub(crate) fn filter_vods_with_metadata<'a>(
    vods: impl Iterator<Item = &'a Vod>,
    params: &ListQuery,
    sort: &str,
    clear_base_url: &str,
) -> (Vec<VodRefMatch<'a>>, ListMetadata) {
    let mut refs: Vec<VodRefMatch<'a>> = vods
        .map(|vod| VodRefMatch {
            vod,
            match_label: None,
        })
        .collect();
    let unfiltered_count = refs.len();

    if let Some(search) = normalized_filter_value(&params.search) {
        let search_lower = search.to_lowercase();
        refs.retain_mut(|r| {
            let title = r.vod.title.as_deref().unwrap_or("Untitled Stream");
            if title.to_lowercase().contains(&search_lower) {
                return true;
            }
            match vod_matching_chapter_name(r.vod, &search_lower) {
                Some(name) => {
                    r.match_label = Some(format!("Matched chapter: {name}"));
                    true
                }
                None => false,
            }
        });
    }

    if let Some(from) = date_filter_lower_bound(&params.from) {
        refs.retain(|r| vod_date(r.vod) >= from.as_str());
    }
    if let Some(to) = date_filter_upper_bound(&params.to) {
        refs.retain(|r| vod_date(r.vod) <= to.as_str());
    }

    match sort {
        "oldest" => refs.sort_by(|a, b| a.vod.stream_time().cmp(b.vod.stream_time())),
        "longest" => refs.sort_by_key(|r| std::cmp::Reverse(vod_duration_minutes(r.vod))),
        "shortest" => refs.sort_by_key(|r| vod_duration_minutes(r.vod)),
        _ => refs.sort_by(|a, b| b.vod.stream_time().cmp(a.vod.stream_time())),
    }

    let metadata = build_list_metadata(unfiltered_count, refs.len(), params, clear_base_url);
    (refs, metadata)
}

fn vod_date(vod: &Vod) -> &str {
    let t = vod.stream_time();
    t.get(..10).unwrap_or(t)
}

fn vod_duration_minutes(vod: &Vod) -> i64 {
    vod.duration.as_ref().map_or(0, |d| d.seconds()) / 60
}

fn vod_matching_chapter_name<'a>(vod: &'a Vod, search_lower: &str) -> Option<&'a str> {
    vod.chapters
        .as_ref()?
        .iter()
        .filter_map(|ch| ch.name.as_deref())
        .find(|name| !name.is_empty() && name.to_lowercase().contains(search_lower))
}

fn normalized_filter_value(value: &Option<String>) -> Option<String> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn date_filter_lower_bound(value: &Option<String>) -> Option<String> {
    normalized_date_filter_value(value, false)
}

fn date_filter_upper_bound(value: &Option<String>) -> Option<String> {
    normalized_date_filter_value(value, true)
}

fn normalized_date_filter_value(value: &Option<String>, upper_bound: bool) -> Option<String> {
    let value = normalized_filter_value(value)?;
    legacy_month_filter_bound(&value, upper_bound).or(Some(value))
}

fn legacy_month_filter_bound(value: &str, upper_bound: bool) -> Option<String> {
    let bytes = value.as_bytes();
    if bytes.len() != 7 || bytes[4] != b'-' {
        return None;
    }
    if !bytes[..4].iter().all(u8::is_ascii_digit) || !bytes[5..].iter().all(u8::is_ascii_digit) {
        return None;
    }

    let year: i32 = value[..4].parse().ok()?;
    let month: u32 = value[5..].parse().ok()?;
    if !(1..=12).contains(&month) {
        return None;
    }

    let day = if upper_bound {
        days_in_month(year, month)
    } else {
        1
    };
    Some(format!("{year:04}-{month:02}-{day:02}"))
}

fn build_list_metadata(
    unfiltered_count: usize,
    filtered_count: usize,
    params: &ListQuery,
    clear_base_url: &str,
) -> ListMetadata {
    build_list_metadata_for_kind(
        unfiltered_count,
        filtered_count,
        params,
        clear_base_url,
        "stream",
        "streams",
    )
}

fn build_list_metadata_for_kind(
    unfiltered_count: usize,
    filtered_count: usize,
    params: &ListQuery,
    clear_base_url: &str,
    singular: &str,
    plural: &str,
) -> ListMetadata {
    let search = normalized_filter_value(&params.search);
    let from = normalized_filter_value(&params.from);
    let to = normalized_filter_value(&params.to);
    let mut active_filters = Vec::new();

    if let Some(search) = search.as_ref() {
        active_filters.push(ActiveFilter {
            label: format!("Search: {search}"),
        });
    }

    match (from.as_ref(), to.as_ref()) {
        (Some(from), Some(to)) if from == to => active_filters.push(ActiveFilter {
            label: format!("Date: {from}"),
        }),
        (Some(from), Some(to)) => active_filters.push(ActiveFilter {
            label: format!("Dates: {from} to {to}"),
        }),
        (Some(from), None) => active_filters.push(ActiveFilter {
            label: format!("From {from}"),
        }),
        (None, Some(to)) => active_filters.push(ActiveFilter {
            label: format!("Through {to}"),
        }),
        (None, None) => {}
    }

    let is_filtered = !active_filters.is_empty();
    let result_label = count_label(filtered_count, singular, plural);
    let clear_url = build_clear_url(clear_base_url, params);

    ListMetadata {
        unfiltered_count,
        filtered_count,
        is_filtered,
        active_filters,
        clear_url,
        result_label,
    }
}

fn count_label(count: usize, singular: &str, plural: &str) -> String {
    let noun = if count == 1 { singular } else { plural };
    format!("{count} {noun}")
}

fn build_clear_url(base_url: &str, params: &ListQuery) -> String {
    let Some(sort) = normalized_filter_value(&params.sort) else {
        return base_url.to_string();
    };
    let sep = if base_url.contains('?') { '&' } else { '?' };
    format!("{base_url}{sep}sort={}", urlencoding::encode(&sort))
}

/// Picks the chapter containing `time_secs` (the last chapter whose start ≤ time).
/// Falls back to the first chapter if no time is given or none match. Returns
/// (chapter_name, chapter_start_seconds) or None if the VOD has no chapters.
pub(crate) fn resolve_watched_chapter(vod: &Vod, time_secs: Option<i64>) -> Option<(String, i64)> {
    let named: Vec<(&str, i64)> = vod
        .chapters
        .as_ref()?
        .iter()
        .filter_map(|ch| {
            let name = ch.name.as_deref().filter(|n| !n.is_empty())?;
            Some((name, ch.start.map(|s| s as i64).unwrap_or(0)))
        })
        .collect();

    let pick = match time_secs {
        Some(t) => named
            .iter()
            .rfind(|&&(_, s)| s <= t)
            .or_else(|| named.first())
            .copied(),
        None => named.first().copied(),
    };
    pick.map(|(n, s)| (n.to_string(), s))
}

pub(crate) struct NextVod {
    pub id: String,
    pub title: String,
}

pub(crate) fn next_vod_in_period(
    vods: &[Vod],
    current_id: &str,
    game_name: &str,
) -> Option<NextVod> {
    let current = vods
        .iter()
        .find(|v| v.id == current_id && v.has_game(game_name))?;
    let current_time = current.stream_time();
    // Earliest playable stream of the same game strictly after the current one.
    let next = vods
        .iter()
        .filter(|v| v.id != current_id && v.is_playable() && v.has_game(game_name))
        .filter(|v| v.stream_time() > current_time)
        .min_by(|a, b| a.stream_time().cmp(b.stream_time()))?;
    let curr_days = parse_ymd_to_days(current_time)?;
    let next_days = parse_ymd_to_days(next.stream_time())?;
    if (next_days - curr_days).abs() <= PERIOD_GAP_DAYS {
        Some(NextVod {
            id: next.id.clone(),
            title: next.title.clone().unwrap_or_else(|| "Untitled".into()),
        })
    } else {
        None
    }
}

pub(crate) fn get_chapter_segments(vod: &Vod) -> Vec<ChapterSegment> {
    let total_duration_secs = vod
        .duration
        .as_ref()
        .map_or(0, |duration| duration.seconds());
    if total_duration_secs <= 0 {
        return Vec::new();
    }

    vod.chapter_spans()
        .into_iter()
        .map(|span| {
            let len = span.end - span.start;
            let width_pct = (len as f64 / total_duration_secs as f64) * 100.0;
            ChapterSegment {
                name: span.name.to_string(),
                width_pct,
                watch_url: build_watch_url(&vod.id, Some(span.start), None),
                color_idx: crate::vods::chapter_color_idx(span.name),
                start_label: format_chapter_start(span.start),
                start_secs: span.start,
                duration_secs: len,
            }
        })
        .collect()
}

pub(crate) fn format_chapter_start(seconds: i64) -> String {
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

pub(crate) fn get_game_tags(vod: &Vod) -> Vec<String> {
    let mut tags = Vec::new();
    let mut seen = std::collections::HashSet::new();
    if let Some(ref chapters) = vod.chapters {
        for ch in chapters {
            if let Some(ref name) = ch.name
                && !name.is_empty()
                && seen.insert(name.to_lowercase())
            {
                tags.push(name.clone());
            }
        }
    }
    tags
}

pub(crate) fn format_date(created_at: &str) -> String {
    let Some(date_part) = created_at.get(..10) else {
        return created_at.to_string();
    };
    if !date_part.is_ascii() {
        return created_at.to_string();
    }
    let parts: Vec<&str> = date_part.split('-').collect();
    if parts.len() != 3 {
        return date_part.to_string();
    }
    let day = parts[2].trim_start_matches('0');
    format!("{} {day}, {}", month_abbr(parts[1]), parts[0])
}

/// Year-less date for the streams subtitle, e.g. "Mar 14". Falls back to the
/// raw input — or its first 10 characters — when it isn't a parseable
/// `YYYY-MM-DD…` string.
pub(crate) fn format_date_short(created_at: &str) -> String {
    let Some(date_part) = created_at.get(..10) else {
        return created_at.to_string();
    };
    if !date_part.is_ascii() {
        return created_at.to_string();
    }
    let parts: Vec<&str> = date_part.split('-').collect();
    if parts.len() != 3 {
        return date_part.to_string();
    }
    let day = parts[2].trim_start_matches('0');
    format!("{} {day}", month_abbr(parts[1]))
}

pub(crate) fn paginate_with_nav<T>(
    items: Vec<T>,
    base_url: &str,
    batch: usize,
    params: &ListQuery,
) -> (Vec<T>, bool, String) {
    let page = params.page.unwrap_or(0);
    let total = items.len();
    let paged = paginate(items, page, batch);
    let has_more = page.saturating_add(1).saturating_mul(batch) < total;
    let next_url = build_next_url(base_url, page.saturating_add(1), params);
    (paged, has_more, next_url)
}

pub(crate) fn paginate<T>(items: Vec<T>, page: usize, batch: usize) -> Vec<T> {
    let start = page.saturating_mul(batch);
    if start >= items.len() {
        return vec![];
    }
    let end = start.saturating_add(batch).min(items.len());
    items.into_iter().skip(start).take(end - start).collect()
}

pub(crate) fn build_next_url(base: &str, page: usize, params: &ListQuery) -> String {
    let mut parts = vec![format!("page={page}")];
    if let Some(ref s) = params.lens {
        parts.push(format!("lens={}", urlencoding::encode(s)));
    }
    if let Some(ref s) = params.game
        && !s.is_empty()
    {
        parts.push(format!("game={}", urlencoding::encode(s)));
    }
    if let Some(ref s) = params.search
        && !s.is_empty()
    {
        parts.push(format!("search={}", urlencoding::encode(s)));
    }
    if let Some(ref s) = params.sort {
        parts.push(format!("sort={}", urlencoding::encode(s)));
    }
    if let Some(ref s) = params.from
        && !s.is_empty()
    {
        parts.push(format!("from={}", urlencoding::encode(s)));
    }
    if let Some(ref s) = params.to
        && !s.is_empty()
    {
        parts.push(format!("to={}", urlencoding::encode(s)));
    }
    format!("{base}?{}", parts.join("&"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vod_display_preserves_numeric_api_duration_seconds() {
        let vod: Vod = serde_json::from_str(
            r#"{
                "id": 1430,
                "platform_vod_id": "2768249708",
                "title": "Test Stream",
                "created_at": "2026-05-09T22:35:39.000Z",
                "duration": 25194,
                "vod_uploads": [
                    {"upload_id": "M1giB9QeXNM"}
                ],
                "chapters": [
                    {"name": "HITMAN", "start": 0}
                ]
            }"#,
        )
        .unwrap();

        let display = VodDisplay::from_vod(&vod);

        assert_eq!(display.duration.as_deref(), Some("6h 59m"));
        assert_eq!(display.duration_seconds, 25194);
    }

    #[test]
    fn test_vod_matches_platform_vod_id() {
        let mut vod = make_vod("1430", "2024-01-01T00:00:00Z", &["HITMAN"]);
        vod.platform_vod_id = Some("2768249708".into());

        assert!(vod_matches_id(&vod, "1430"));
        assert!(vod_matches_id(&vod, "2768249708"));
        assert!(!vod_matches_id(&vod, "nope"));
    }

    #[test]
    fn test_format_date() {
        assert_eq!(format_date("2025-01-15T00:00:00Z"), "Jan 15, 2025");
        assert_eq!(format_date("2025-12-01T12:30:00Z"), "Dec 1, 2025");
    }

    #[test]
    fn test_format_date_short_drops_year() {
        assert_eq!(format_date_short("2025-01-15T00:00:00Z"), "Jan 15");
        assert_eq!(format_date_short("2025-12-01T12:30:00Z"), "Dec 1");
        assert_eq!(format_date_short("éééééé"), "éééééé");
    }

    #[test]
    fn test_format_date_handles_non_ascii_without_panicking() {
        assert_eq!(format_date("éééééé"), "éééééé");
    }

    #[test]
    fn test_get_game_tags() {
        let vod = Vod {
            id: "1".into(),
            platform: None,
            platform_vod_id: None,
            platform_stream_id: None,
            title: Some("Test".into()),
            created_at: "2025-01-01T00:00:00Z".into(),
            started_at: None,
            updated_at: None,
            duration: Some("2h".into()),
            thumbnail_url: None,
            chapters: Some(vec![
                crate::vods::Chapter {
                    name: Some("Game A".into()),
                    image: None,
                    start: Some(0.0),
                    duration: None,
                    end: None,
                },
                crate::vods::Chapter {
                    name: Some("Game A".into()),
                    image: None,
                    start: Some(100.0),
                    duration: None,
                    end: None,
                },
                crate::vods::Chapter {
                    name: Some("Game B".into()),
                    image: None,
                    start: Some(200.0),
                    duration: None,
                    end: None,
                },
            ]),
            youtube: None,
            is_live: false,
        };
        let tags = get_game_tags(&vod);
        assert_eq!(tags, vec!["Game A".to_string(), "Game B".to_string()]);
    }

    #[test]
    fn test_get_game_tags_case_insensitive() {
        let vod = Vod {
            id: "1".into(),
            platform: None,
            platform_vod_id: None,
            platform_stream_id: None,
            title: Some("Test".into()),
            created_at: "2025-01-01T00:00:00Z".into(),
            started_at: None,
            updated_at: None,
            duration: Some("2h".into()),
            thumbnail_url: None,
            chapters: Some(vec![
                crate::vods::Chapter {
                    name: Some("Elden Ring".into()),
                    image: None,
                    start: Some(0.0),
                    duration: None,
                    end: None,
                },
                crate::vods::Chapter {
                    name: Some("ELDEN RING".into()),
                    image: None,
                    start: Some(500.0),
                    duration: None,
                    end: None,
                },
            ]),
            youtube: None,
            is_live: false,
        };
        let tags = get_game_tags(&vod);
        assert_eq!(tags, vec!["Elden Ring".to_string()]);
    }

    #[test]
    fn test_vod_has_game() {
        let vod = Vod {
            id: "1".into(),
            platform: None,
            platform_vod_id: None,
            platform_stream_id: None,
            title: Some("Test".into()),
            created_at: "2025-01-01T00:00:00Z".into(),
            started_at: None,
            updated_at: None,
            duration: Some("2h".into()),
            thumbnail_url: None,
            chapters: Some(vec![crate::vods::Chapter {
                name: Some("Elden Ring".into()),
                image: None,
                start: None,
                duration: None,
                end: None,
            }]),
            youtube: None,
            is_live: false,
        };
        assert!(vod.has_game("Elden Ring"));
        assert!(vod.has_game("elden ring"));
        assert!(vod.has_game("ELDEN RING"));
        assert!(!vod.has_game("Dark Souls"));
    }

    #[test]
    fn test_paginate() {
        let items: Vec<i32> = (0..100).collect();
        let page0 = paginate(items.clone(), 0, 36);
        assert_eq!(page0.len(), 36);
        assert_eq!(page0[0], 0);

        let page2 = paginate(items.clone(), 2, 36);
        assert_eq!(page2.len(), 28);

        let page3 = paginate(items, 3, 36);
        assert!(page3.is_empty());
    }

    #[test]
    fn test_paginate_handles_huge_page_without_overflow() {
        let items: Vec<i32> = (0..10).collect();
        assert!(paginate(items.clone(), usize::MAX, 36).is_empty());
        let params = ListQuery {
            page: Some(usize::MAX),
            ..Default::default()
        };
        let (paged, has_more, _) = paginate_with_nav(items, "/x", 36, &params);
        assert!(paged.is_empty());
        assert!(!has_more);
    }

    #[test]
    fn filter_vods_with_metadata_matches_title_and_chapters() {
        let mut titled = make_vod("title", "2026-05-20T00:00:00Z", &[]);
        titled.title = Some("Late night HITMAN".into());
        let vods = [
            titled,
            make_vod("chapter", "2026-05-19T00:00:00Z", &["HITMAN"]),
            make_vod("neither", "2026-05-18T00:00:00Z", &["Terraria"]),
        ];
        let params = ListQuery {
            search: Some("hitman".into()),
            ..Default::default()
        };
        let (refs, metadata) =
            filter_vods_with_metadata(vods.iter(), &params, "newest", "/browse?lens=streams");
        assert_eq!(refs.len(), 2);
        assert_eq!(refs[0].vod.id, "title");
        assert!(refs[0].match_label.is_none());
        assert_eq!(refs[1].vod.id, "chapter");
        assert_eq!(
            refs[1].match_label.as_deref(),
            Some("Matched chapter: HITMAN")
        );
        assert_eq!(metadata.unfiltered_count, 3);
        assert_eq!(metadata.filtered_count, 2);
    }

    #[test]
    fn filter_vods_with_metadata_filters_dates_inclusive_and_sorts() {
        let vods = [
            make_vod("after", "2026-05-20T00:00:00Z", &["A"]),
            make_vod("same-day", "2026-05-19T23:59:59Z", &["A"]),
            make_vod("before", "2026-05-18T23:59:59Z", &["A"]),
        ];
        let params = ListQuery {
            sort: Some("oldest".into()),
            from: Some("2026-05-19".into()),
            to: Some("2026-05-19".into()),
            ..Default::default()
        };
        let (refs, _) = filter_vods_with_metadata(vods.iter(), &params, "oldest", "/x");
        assert_eq!(
            refs.iter().map(|r| r.vod.id.as_str()).collect::<Vec<_>>(),
            vec!["same-day"]
        );
    }

    #[test]
    fn filter_vods_with_metadata_ignores_blank_filters() {
        let vods = [
            make_vod("newer", "2026-05-20T00:00:00Z", &["A"]),
            make_vod("older", "2026-05-19T00:00:00Z", &["A"]),
        ];
        let params = ListQuery {
            search: Some("   ".into()),
            from: Some("".into()),
            to: Some("".into()),
            ..Default::default()
        };
        let (refs, metadata) = filter_vods_with_metadata(vods.iter(), &params, "newest", "/x");
        assert_eq!(
            refs.iter().map(|r| r.vod.id.as_str()).collect::<Vec<_>>(),
            vec!["newer", "older"]
        );
        assert!(!metadata.is_filtered);
    }

    #[test]
    fn test_vod_display_uses_started_at_for_stream_date() {
        let mut vod = make_vod("started", "2026-05-10T23:05:44.967Z", &["HITMAN"]);
        vod.started_at = Some("2026-05-09T22:35:39.000Z".into());

        let display = VodDisplay::from_vod(&vod);

        assert_eq!(display.formatted_date, "May 9, 2026");
        assert_eq!(display.created_at, "2026-05-09T22:35:39.000Z");
    }

    #[test]
    fn games_date_filter_recomputes_visible_counts() {
        let vods = vec![
            make_vod("elden-1", "2026-05-20T10:00:00Z", &["Elden Ring"]),
            make_vod("elden-2", "2026-05-20T11:00:00Z", &["Elden Ring"]),
            make_vod("hitman-in-range", "2026-05-20T12:00:00Z", &["HITMAN"]),
            make_vod("hitman-old-1", "2026-05-18T12:00:00Z", &["HITMAN"]),
            make_vod("hitman-old-2", "2026-05-17T12:00:00Z", &["HITMAN"]),
            make_vod("terraria-old", "2026-05-18T12:00:00Z", &["Terraria"]),
        ];
        let all_games = crate::vods::build_games(&vods);
        let params = ListQuery {
            search: None,
            sort: Some("most".into()),
            from: Some("2026-05-20".into()),
            to: Some("2026-05-20".into()),
            page: None,
            ..Default::default()
        };

        let filtered = filter_games_with_metadata(&all_games, &vods, &params, "/games", "most");

        assert_eq!(filtered.metadata.unfiltered_count, 3);
        assert_eq!(filtered.metadata.filtered_count, 2);
        assert_eq!(
            filtered
                .games
                .iter()
                .map(|game| (game.name.as_str(), game.vod_count))
                .collect::<Vec<_>>(),
            vec![("Elden Ring", 2), ("HITMAN", 1)]
        );
    }

    #[test]
    fn games_search_and_date_filters_compose() {
        let vods = vec![
            make_vod("alpha-quest-1", "2026-05-20T10:00:00Z", &["Alpha Quest"]),
            make_vod("alpha-quest-2", "2026-05-20T11:00:00Z", &["Alpha Quest"]),
            make_vod("alpha-zero", "2026-05-20T12:00:00Z", &["Alpha Zero"]),
            make_vod("alpha-old", "2026-05-18T12:00:00Z", &["Alpha Classic"]),
            make_vod("beta", "2026-05-20T13:00:00Z", &["Beta Quest"]),
        ];
        let all_games = crate::vods::build_games(&vods);
        let params = ListQuery {
            search: Some("alpha".into()),
            sort: Some("fewest".into()),
            from: Some("2026-05-20".into()),
            to: Some("2026-05-20".into()),
            page: None,
            ..Default::default()
        };

        let filtered = filter_games_with_metadata(&all_games, &vods, &params, "/games", "fewest");

        assert_eq!(filtered.metadata.unfiltered_count, 4);
        assert_eq!(filtered.metadata.filtered_count, 2);
        assert_eq!(
            filtered
                .games
                .iter()
                .map(|game| (game.name.as_str(), game.vod_count))
                .collect::<Vec<_>>(),
            vec![("Alpha Zero", 1), ("Alpha Quest", 2)]
        );
    }

    #[test]
    fn games_date_filter_accepts_legacy_month_values() {
        let vods = vec![
            make_vod("may-1", "2026-05-01T00:00:00Z", &["HITMAN"]),
            make_vod("may-2", "2026-05-31T23:59:59Z", &["HITMAN"]),
            make_vod("june", "2026-06-01T00:00:00Z", &["Elden Ring"]),
        ];
        let all_games = crate::vods::build_games(&vods);
        let params = ListQuery {
            search: None,
            sort: Some("most".into()),
            from: Some("2026-05".into()),
            to: Some("2026-05".into()),
            page: None,
            ..Default::default()
        };

        let filtered = filter_games_with_metadata(&all_games, &vods, &params, "/games", "most");

        assert_eq!(
            filtered
                .games
                .iter()
                .map(|game| (game.name.as_str(), game.vod_count))
                .collect::<Vec<_>>(),
            vec![("HITMAN", 2)]
        );
        assert_eq!(filtered.metadata.filtered_count, 1);
    }

    #[test]
    fn games_default_sort_uses_latest_dominant_stream() {
        let vods = vec![
            make_vod("old-popular-1", "2026-05-01T10:00:00Z", &["Old Popular"]),
            make_vod("old-popular-2", "2026-05-02T10:00:00Z", &["Old Popular"]),
            make_vod("fresh", "2026-05-20T10:00:00Z", &["Fresh Game"]),
        ];
        let all_games = crate::vods::build_games(&vods);
        let params = ListQuery::default();

        let filtered = filter_games_with_metadata(&all_games, &vods, &params, "/games", "recent");

        assert_eq!(
            filtered
                .games
                .iter()
                .map(|game| game.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Fresh Game", "Old Popular"]
        );
    }

    #[test]
    fn games_date_filter_ignores_short_cameos() {
        let vods = vec![
            make_vod_with_chapters(
                "dominant-main",
                "2026-05-20T10:00:00Z",
                4 * 3600 + 300,
                &[
                    ("Main Game", 0.0, 4.0 * 3600.0),
                    ("Cameo Game", 4.0 * 3600.0, 300.0),
                ],
            ),
            make_vod("cameo-old", "2026-05-01T10:00:00Z", &["Cameo Game"]),
        ];
        let all_games = crate::vods::build_games(&vods);
        let params = ListQuery {
            search: None,
            sort: Some("recent".into()),
            from: Some("2026-05-20".into()),
            to: Some("2026-05-20".into()),
            page: None,
            ..Default::default()
        };

        let filtered = filter_games_with_metadata(&all_games, &vods, &params, "/games", "recent");

        assert_eq!(
            filtered
                .games
                .iter()
                .map(|game| game.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Main Game"]
        );
    }

    #[test]
    fn chapter_color_indices_use_handoff_hash() {
        fn handoff_color_idx(name: &str) -> u8 {
            let mut h: u32 = 0;
            for b in name.bytes() {
                h = h.wrapping_mul(31).wrapping_add(u32::from(b));
            }
            (h % 8) as u8
        }

        let vod = make_vod("color", "2026-05-20T10:00:00Z", &["Elden Ring"]);
        let segments = get_chapter_segments(&vod);

        assert_eq!(segments[0].color_idx, handoff_color_idx("Elden Ring"));
    }

    #[test]
    fn test_resolve_watched_chapter_picks_containing_chapter() {
        let vod = make_vod("a", "2024-01-01T00:00:00Z", &["Just Chatting", "Terraria"]);
        // make_vod sets all chapter starts to 0.0 — override for this test.
        let mut vod = vod;
        if let Some(ref mut chs) = vod.chapters {
            chs[0].start = Some(0.0);
            chs[1].start = Some(3600.0);
        }
        let (name, start) = resolve_watched_chapter(&vod, Some(5000)).unwrap();
        assert_eq!(name, "Terraria");
        assert_eq!(start, 3600);

        let (name, _) = resolve_watched_chapter(&vod, Some(100)).unwrap();
        assert_eq!(name, "Just Chatting");

        // No time → first chapter
        let (name, _) = resolve_watched_chapter(&vod, None).unwrap();
        assert_eq!(name, "Just Chatting");
    }

    #[test]
    fn test_resolve_watched_chapter_none_for_empty() {
        let vod = Vod {
            id: "x".into(),
            platform: None,
            platform_vod_id: None,
            platform_stream_id: None,
            title: None,
            created_at: "2024-01-01T00:00:00Z".into(),
            started_at: None,
            updated_at: None,
            duration: None,
            thumbnail_url: None,
            chapters: Some(vec![]),
            youtube: None,
            is_live: false,
        };
        assert!(resolve_watched_chapter(&vod, Some(100)).is_none());
    }

    // Playable like every catalog VOD handlers see (filter_playable_vods runs
    // before the catalog is swapped in).
    fn make_vod(id: &str, created_at: &str, games: &[&str]) -> Vod {
        Vod {
            id: id.into(),
            platform: None,
            platform_vod_id: None,
            platform_stream_id: None,
            title: Some(format!("vod {id}")),
            created_at: created_at.into(),
            started_at: None,
            updated_at: None,
            duration: Some("1h".into()),
            thumbnail_url: None,
            chapters: Some(
                games
                    .iter()
                    .map(|g| crate::vods::Chapter {
                        name: Some((*g).into()),
                        image: None,
                        start: Some(0.0),
                        duration: None,
                        end: None,
                    })
                    .collect(),
            ),
            youtube: Some(vec![crate::vods::YoutubeVideo {
                row_id: None,
                id: format!("yt-{id}"),
                thumbnail_url: None,
                part: None,
                duration: None,
                status: Some("COMPLETED".into()),
                upload_type: Some("vod".into()),
                created_at: None,
            }]),
            is_live: false,
        }
    }

    fn make_vod_with_chapters(
        id: &str,
        created_at: &str,
        duration_secs: i64,
        chapters: &[(&str, f64, f64)],
    ) -> Vod {
        Vod {
            id: id.into(),
            platform: None,
            platform_vod_id: None,
            platform_stream_id: None,
            title: Some(format!("vod {id}")),
            created_at: created_at.into(),
            started_at: None,
            updated_at: None,
            duration: Some(crate::vods::VodDuration::from_seconds(duration_secs)),
            thumbnail_url: None,
            chapters: Some(
                chapters
                    .iter()
                    .map(|(name, start, duration)| crate::vods::Chapter {
                        name: Some((*name).into()),
                        image: None,
                        start: Some(*start),
                        duration: Some(*duration),
                        end: Some(start + duration),
                    })
                    .collect(),
            ),
            youtube: None,
            is_live: false,
        }
    }

    #[test]
    fn test_next_vod_in_period_within_gap() {
        let vods = vec![
            make_vod("a", "2024-01-01T00:00:00Z", &["Elden Ring"]),
            make_vod("b", "2024-01-05T00:00:00Z", &["Elden Ring"]),
            make_vod("c", "2024-01-10T00:00:00Z", &["Elden Ring"]),
        ];
        let next = next_vod_in_period(&vods, "a", "Elden Ring").unwrap();
        assert_eq!(next.id, "b");
    }

    #[test]
    fn test_next_vod_in_period_beyond_gap_returns_none() {
        let vods = vec![
            make_vod("a", "2024-01-01T00:00:00Z", &["Elden Ring"]),
            make_vod("b", "2024-03-01T00:00:00Z", &["Elden Ring"]),
        ];
        assert!(next_vod_in_period(&vods, "a", "Elden Ring").is_none());
    }

    #[test]
    fn test_next_vod_in_period_last_in_period_returns_none() {
        let vods = vec![
            make_vod("a", "2024-01-01T00:00:00Z", &["Elden Ring"]),
            make_vod("b", "2024-01-05T00:00:00Z", &["Elden Ring"]),
        ];
        assert!(next_vod_in_period(&vods, "b", "Elden Ring").is_none());
    }

    #[test]
    fn test_next_vod_in_period_game_not_in_vod_returns_none() {
        let vods = vec![make_vod("a", "2024-01-01T00:00:00Z", &["Dark Souls"])];
        assert!(next_vod_in_period(&vods, "a", "Elden Ring").is_none());
    }

    #[test]
    fn test_next_vod_in_period_is_case_insensitive() {
        let vods = vec![
            make_vod("a", "2024-01-01T00:00:00Z", &["Elden Ring"]),
            make_vod("b", "2024-01-05T00:00:00Z", &["ELDEN RING"]),
        ];
        let next = next_vod_in_period(&vods, "a", "elden ring").unwrap();
        assert_eq!(next.id, "b");
    }

    #[test]
    fn test_next_vod_in_period_skips_unplayable_vods() {
        let mut unplayable = make_vod("b", "2024-01-03T00:00:00Z", &["Elden Ring"]);
        unplayable.youtube = None;
        let vods = vec![
            make_vod("a", "2024-01-01T00:00:00Z", &["Elden Ring"]),
            unplayable,
            make_vod("c", "2024-01-05T00:00:00Z", &["Elden Ring"]),
        ];
        let next = next_vod_in_period(&vods, "a", "Elden Ring").unwrap();
        assert_eq!(next.id, "c");
    }

    #[test]
    fn test_next_vod_in_period_filters_out_other_games() {
        let vods = vec![
            make_vod("a", "2024-01-01T00:00:00Z", &["Elden Ring"]),
            make_vod("b", "2024-01-03T00:00:00Z", &["Dark Souls"]),
            make_vod("c", "2024-01-05T00:00:00Z", &["Elden Ring"]),
        ];
        let next = next_vod_in_period(&vods, "a", "Elden Ring").unwrap();
        assert_eq!(next.id, "c");
    }

    #[test]
    fn test_next_vod_in_period_uses_started_at_for_gap() {
        let mut a = make_vod("a", "2024-03-01T00:00:00Z", &["Elden Ring"]);
        a.started_at = Some("2024-01-01T00:00:00Z".into());
        let mut b = make_vod("b", "2024-03-02T00:00:00Z", &["Elden Ring"]);
        b.started_at = Some("2024-01-05T00:00:00Z".into());
        let vods = vec![a, b];

        let next = next_vod_in_period(&vods, "a", "Elden Ring").unwrap();

        assert_eq!(next.id, "b");
    }

    #[test]
    fn test_chapter_segments_use_explicit_timing_and_clamp_invalid_ranges() {
        let mut vod = make_vod("chapters", "2024-01-01T00:00:00Z", &[]);
        vod.duration = Some(crate::vods::VodDuration::from_seconds(1000));
        vod.chapters = Some(vec![
            crate::vods::Chapter {
                name: Some("Explicit End".into()),
                image: None,
                start: Some(100.0),
                duration: None,
                end: Some(300.0),
            },
            crate::vods::Chapter {
                name: Some("Explicit Duration".into()),
                image: None,
                start: Some(300.0),
                duration: Some(100.0),
                end: None,
            },
            crate::vods::Chapter {
                name: Some("Inferred".into()),
                image: None,
                start: Some(400.0),
                duration: None,
                end: None,
            },
            crate::vods::Chapter {
                name: Some("Clamped".into()),
                image: None,
                start: Some(900.0),
                duration: Some(500.0),
                end: None,
            },
            crate::vods::Chapter {
                name: Some("Skipped".into()),
                image: None,
                start: Some(1200.0),
                duration: Some(10.0),
                end: None,
            },
        ]);

        let segments = get_chapter_segments(&vod);

        assert_eq!(
            segments.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
            vec!["Explicit End", "Explicit Duration", "Inferred", "Clamped"]
        );
        assert_eq!(segments[0].width_pct, 20.0);
        assert_eq!(segments[1].width_pct, 10.0);
        assert_eq!(segments[2].width_pct, 50.0);
        assert_eq!(segments[3].width_pct, 10.0);
        assert_eq!(segments[0].watch_url, "/watch/chapters?t=100");
        assert_eq!(segments[0].start_label, "1:40");
        assert_eq!(segments[2].start_label, "6:40");
    }

    #[test]
    fn test_build_watch_url() {
        assert_eq!(build_watch_url("abc", None, None), "/watch/abc");
        assert_eq!(build_watch_url("abc", Some(42), None), "/watch/abc?t=42");
        assert_eq!(
            build_watch_url("abc", None, Some("Elden Ring")),
            "/watch/abc?game=Elden%20Ring"
        );
        assert_eq!(
            build_watch_url("abc", Some(42), Some("Elden Ring")),
            "/watch/abc?t=42&game=Elden%20Ring"
        );
        assert_eq!(build_watch_url("abc", None, Some("")), "/watch/abc");
    }

    #[test]
    fn test_build_next_url() {
        let params = ListQuery {
            search: Some("test".into()),
            sort: Some("most".into()),
            from: None,
            to: None,
            page: None,
            ..Default::default()
        };
        let url = build_next_url("/games", 1, &params);
        assert!(url.starts_with("/games?"));
        assert!(url.contains("page=1"));
        assert!(url.contains("search=test"));
        assert!(url.contains("sort=most"));
    }

    #[test]
    fn test_build_next_url_includes_lens_and_game() {
        let params = ListQuery {
            sort: Some("newest".into()),
            lens: Some("streams".into()),
            game: Some("Elden Ring".into()),
            ..Default::default()
        };
        let url = build_next_url("/browse/grid", 2, &params);
        assert!(url.contains("page=2"));
        assert!(url.contains("lens=streams"));
        assert!(url.contains("game=Elden%20Ring"));
    }

    #[test]
    fn test_build_clear_url_appends_with_ampersand_when_base_has_query() {
        let params = ListQuery {
            sort: Some("newest".into()),
            ..Default::default()
        };
        assert_eq!(
            build_clear_url("/browse?lens=streams", &params),
            "/browse?lens=streams&sort=newest"
        );
        let none = ListQuery::default();
        assert_eq!(
            build_clear_url("/browse?lens=games", &none),
            "/browse?lens=games"
        );
        assert_eq!(build_clear_url("/games", &none), "/games");
    }
}
