mod api;
mod calendar;
mod games;
mod history;
mod vods;
mod watch;

pub use api::{chat_proxy, refresh_vods};
pub use calendar::calendar_page;
pub use games::{games_grid, games_page};
pub use history::{history_page, history_vods_grid};
pub use vods::{all_streams_grid, all_streams_page, game_vods_grid, game_vods_page};
pub use watch::{next_in_period, random_vod, vod_detail, watch_page};

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
    Games,
    Streams,
    History,
    Calendar,
}

impl Section {
    pub(crate) fn slug(&self) -> &'static str {
        match self {
            Section::None => "",
            Section::Games => "games",
            Section::Streams => "streams",
            Section::History => "history",
            Section::Calendar => "calendar",
        }
    }
}

// ─── Query types ───

#[derive(Deserialize, Default)]
pub struct ListQuery {
    pub search: Option<String>,
    pub sort: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub page: Option<usize>,
}

// ─── Display types ───

pub(crate) struct GameTag {
    pub name: String,
    pub start_seconds: i64,
}

pub(crate) struct VodDisplay {
    pub id: String,
    pub display_title: String,
    pub formatted_date: String,
    pub duration: Option<String>,
    pub thumbnail_url: Option<String>,
    pub game_tags: Vec<GameTag>,
    pub created_at: String,
    pub duration_minutes: i64,
    pub duration_seconds: i64,
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
        let formatted_date = format_date(&vod.created_at);
        let game_tags = get_game_tags(vod);
        let duration_minutes = parse_duration_minutes(vod.duration.as_deref().unwrap_or(""));
        let duration_seconds = parse_duration_seconds(vod.duration.as_deref().unwrap_or(""));
        let watch_url = build_watch_url(&vod.id, chapter_start, game_name_hint);
        VodDisplay {
            id: vod.id.clone(),
            display_title,
            formatted_date,
            duration: vod.duration.clone(),
            thumbnail_url: vod.thumbnail_url.clone(),
            game_tags,
            created_at: vod.created_at.clone(),
            duration_minutes,
            duration_seconds,
            period_header: None,
            watch_url,
        }
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

pub(crate) fn filter_games(mut games: Vec<Game>, params: &ListQuery) -> Vec<Game> {
    if let Some(ref search) = params.search {
        let search_lower = search.to_lowercase();
        if !search_lower.is_empty() {
            games.retain(|g| g.name.to_lowercase().contains(&search_lower));
        }
    }

    let sort = params.sort.as_deref().unwrap_or("most");
    match sort {
        "fewest" => games.sort_by(|a, b| a.vod_count.cmp(&b.vod_count)),
        "az" => games.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase())),
        "za" => games.sort_by(|a, b| b.name.to_lowercase().cmp(&a.name.to_lowercase())),
        _ => games.sort_by(|a, b| b.vod_count.cmp(&a.vod_count)),
    }

    games
}

pub(crate) fn filter_vod_displays(displays: &mut Vec<VodDisplay>, params: &ListQuery) {
    if let Some(ref search) = params.search {
        let search_lower = search.to_lowercase();
        if !search_lower.is_empty() {
            displays.retain(|v| v.display_title.to_lowercase().contains(&search_lower));
        }
    }

    if let Some(ref from) = params.from
        && !from.is_empty()
    {
        displays.retain(|v| v.created_at.as_str() >= from.as_str());
    }

    if let Some(ref to) = params.to
        && !to.is_empty()
    {
        let to_end = format!("{to}\u{ffff}");
        displays.retain(|v| v.created_at.as_str() <= to_end.as_str());
    }

    let sort = params.sort.as_deref().unwrap_or("newest");
    match sort {
        "oldest" => displays.sort_by(|a, b| a.created_at.cmp(&b.created_at)),
        "longest" => displays.sort_by(|a, b| b.duration_minutes.cmp(&a.duration_minutes)),
        "shortest" => displays.sort_by(|a, b| a.duration_minutes.cmp(&b.duration_minutes)),
        _ => displays.sort_by(|a, b| b.created_at.cmp(&a.created_at)),
    }
}

pub(crate) fn assign_period_headers(displays: &mut [VodDisplay], sort: &str) {
    if displays.len() < 2 {
        return;
    }
    if sort != "newest" && sort != "oldest" {
        return;
    }

    let days: Vec<Option<i64>> = displays
        .iter()
        .map(|d| parse_ymd_to_days(&d.created_at))
        .collect();

    let mut cluster_starts: Vec<usize> = vec![0];
    for i in 1..displays.len() {
        if let (Some(a), Some(b)) = (days[i - 1], days[i])
            && (a - b).abs() > PERIOD_GAP_DAYS
        {
            cluster_starts.push(i);
        }
    }

    if cluster_starts.len() < 2 {
        return;
    }

    for (ci, &start) in cluster_starts.iter().enumerate() {
        let end = cluster_starts
            .get(ci + 1)
            .copied()
            .unwrap_or(displays.len());
        let count = end - start;
        let first_date = displays[start].created_at.clone();
        let last_date = displays[end - 1].created_at.clone();
        let (newest, oldest) = if sort == "newest" {
            (first_date.as_str(), last_date.as_str())
        } else {
            (last_date.as_str(), first_date.as_str())
        };
        displays[start].period_header = Some(build_period_label(oldest, newest, count));
    }
}

pub(crate) fn assign_series_headers(displays: &mut [VodDisplay], keys: &[Option<String>]) {
    if displays.is_empty() || keys.len() != displays.len() {
        return;
    }

    let norm = |p: &Option<String>| p.as_deref().map(|s| s.to_lowercase());

    let mut run_start = 0usize;
    for i in 1..=displays.len() {
        let boundary = i == displays.len() || norm(&keys[i]) != norm(&keys[run_start]);
        if boundary {
            let count = i - run_start;
            let label_name = keys[run_start].as_deref().unwrap_or("Untagged").to_string();
            let noun = if count == 1 { "stream" } else { "streams" };
            displays[run_start].period_header = Some(format!("{label_name} · {count} {noun}"));
            run_start = i;
        }
    }
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

fn build_period_label(oldest: &str, newest: &str, count: usize) -> String {
    let old_my = month_year(oldest);
    let new_my = month_year(newest);
    let range = if old_my == new_my {
        old_my
    } else {
        format!("{old_my} – {new_my}")
    };
    let noun = if count == 1 { "stream" } else { "streams" };
    format!("{range} · {count} {noun}")
}

fn month_year(created_at: &str) -> String {
    let Some(date_part) = created_at.get(..10) else {
        return created_at.to_string();
    };
    let parts: Vec<&str> = date_part.split('-').collect();
    if parts.len() != 3 {
        return date_part.to_string();
    }
    format!("{} {}", month_abbr(parts[1]), parts[0])
}

fn month_abbr(month_part: &str) -> &str {
    match month_part {
        "01" => "Jan",
        "02" => "Feb",
        "03" => "Mar",
        "04" => "Apr",
        "05" => "May",
        "06" => "Jun",
        "07" => "Jul",
        "08" => "Aug",
        "09" => "Sep",
        "10" => "Oct",
        "11" => "Nov",
        "12" => "Dec",
        other => other,
    }
}

pub(crate) fn parse_ymd_to_days(created_at: &str) -> Option<i64> {
    let date_part = created_at.get(..10)?;
    let parts: Vec<&str> = date_part.split('-').collect();
    if parts.len() != 3 {
        return None;
    }
    let y: i32 = parts[0].parse().ok()?;
    let m: u32 = parts[1].parse().ok()?;
    let d: u32 = parts[2].parse().ok()?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    Some(civil_to_days(y, m, d))
}

fn civil_to_days(year: i32, month: u32, day: u32) -> i64 {
    let y = if month <= 2 { year - 1 } else { year } as i64;
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = if month > 2 { month - 3 } else { month + 9 } as i64;
    let doy = (153 * mp + 2) / 5 + day as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

pub(crate) fn get_chapter_start(vod: &Vod, game_name: &str) -> Option<i64> {
    if let Some(ref chapters) = vod.chapters {
        for ch in chapters {
            if let Some(ref name) = ch.name
                && name.eq_ignore_ascii_case(game_name)
            {
                return ch.start.map(|s| s as i64);
            }
        }
    }
    None
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
    let mut matches: Vec<&Vod> = vods.iter().filter(|v| vod_has_game(v, game_name)).collect();
    matches.sort_by(|a, b| a.created_at.cmp(&b.created_at));
    let idx = matches.iter().position(|v| v.id == current_id)?;
    let next = matches.get(idx + 1)?;
    let curr_days = parse_ymd_to_days(&matches[idx].created_at)?;
    let next_days = parse_ymd_to_days(&next.created_at)?;
    if (next_days - curr_days).abs() <= PERIOD_GAP_DAYS {
        Some(NextVod {
            id: next.id.clone(),
            title: next.title.clone().unwrap_or_else(|| "Untitled".into()),
        })
    } else {
        None
    }
}

pub(crate) fn vod_has_game(vod: &Vod, game_name: &str) -> bool {
    if let Some(ref chapters) = vod.chapters {
        chapters.iter().any(|ch| {
            ch.name
                .as_deref()
                .map(|n| n.eq_ignore_ascii_case(game_name))
                .unwrap_or(false)
        })
    } else {
        false
    }
}

pub(crate) fn find_game_image(games: &[Game], game_name: &str) -> Option<String> {
    games
        .iter()
        .find(|g| g.name.eq_ignore_ascii_case(game_name))
        .and_then(|g| g.image.clone())
}

pub(crate) fn get_game_tags(vod: &Vod) -> Vec<GameTag> {
    let mut tags = Vec::new();
    let mut seen = std::collections::HashSet::new();
    if let Some(ref chapters) = vod.chapters {
        for ch in chapters {
            if let Some(ref name) = ch.name
                && !name.is_empty()
                && seen.insert(name.to_lowercase())
            {
                tags.push(GameTag {
                    name: name.clone(),
                    start_seconds: ch.start.map(|s| s as i64).unwrap_or(0),
                });
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

pub(crate) fn parse_duration_minutes(duration: &str) -> i64 {
    parse_duration_seconds(duration) / 60
}

pub(crate) fn parse_duration_seconds(duration: &str) -> i64 {
    let parts: Vec<&str> = duration.split(':').collect();
    if parts.len() == 3 {
        let h = parts[0].parse::<i64>().unwrap_or(0);
        let m = parts[1].parse::<i64>().unwrap_or(0);
        let s = parts[2].parse::<i64>().unwrap_or(0);
        return h * 3600 + m * 60 + s;
    }
    if parts.len() == 2 {
        let m = parts[0].parse::<i64>().unwrap_or(0);
        let s = parts[1].parse::<i64>().unwrap_or(0);
        return m * 60 + s;
    }
    let mut total: i64 = 0;
    let mut num_buf = String::new();
    for ch in duration.chars() {
        if ch.is_ascii_digit() {
            num_buf.push(ch);
        } else if ch == 'h' || ch == 'H' {
            if let Ok(h) = num_buf.parse::<i64>() {
                total += h * 3600;
            }
            num_buf.clear();
        } else if ch == 'm' || ch == 'M' {
            if let Ok(m) = num_buf.parse::<i64>() {
                total += m * 60;
            }
            num_buf.clear();
        } else if ch == 's' || ch == 'S' {
            if let Ok(s) = num_buf.parse::<i64>() {
                total += s;
            }
            num_buf.clear();
        } else {
            num_buf.clear();
        }
    }
    total
}

pub(crate) fn paginate<T>(items: Vec<T>, page: usize, batch: usize) -> Vec<T> {
    let start = page * batch;
    if start >= items.len() {
        return vec![];
    }
    let end = (start + batch).min(items.len());
    items.into_iter().skip(start).take(end - start).collect()
}

pub(crate) fn build_next_url(base: &str, page: usize, params: &ListQuery) -> String {
    let mut parts = vec![format!("page={page}")];
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
    fn test_parse_duration_minutes() {
        assert_eq!(parse_duration_minutes("3h 20m"), 200);
        assert_eq!(parse_duration_minutes("1h"), 60);
        assert_eq!(parse_duration_minutes("45m"), 45);
        assert_eq!(parse_duration_minutes(""), 0);
        assert_eq!(parse_duration_minutes("10h 5m"), 605);
        assert_eq!(parse_duration_minutes("07:02:52"), 422);
        assert_eq!(parse_duration_minutes("09:28:29"), 568);
    }

    #[test]
    fn test_parse_duration_seconds() {
        assert_eq!(parse_duration_seconds("07:02:52"), 25372);
        assert_eq!(parse_duration_seconds("09:28:29"), 34109);
        assert_eq!(parse_duration_seconds("00:30:00"), 1800);
        assert_eq!(parse_duration_seconds("3h 20m"), 12000);
        assert_eq!(parse_duration_seconds(""), 0);
    }

    #[test]
    fn test_format_date() {
        assert_eq!(format_date("2025-01-15T00:00:00Z"), "Jan 15, 2025");
        assert_eq!(format_date("2025-12-01T12:30:00Z"), "Dec 1, 2025");
    }

    #[test]
    fn test_format_date_handles_non_ascii_without_panicking() {
        assert_eq!(format_date("éééééé"), "éééééé");
    }

    #[test]
    fn test_get_game_tags() {
        let vod = Vod {
            id: "1".into(),
            title: Some("Test".into()),
            created_at: "2025-01-01T00:00:00Z".into(),
            duration: Some("2h".into()),
            thumbnail_url: None,
            chapters: Some(vec![
                crate::vods::Chapter {
                    name: Some("Game A".into()),
                    image: None,
                    start: Some(0.0),
                },
                crate::vods::Chapter {
                    name: Some("Game A".into()),
                    image: None,
                    start: Some(100.0),
                },
                crate::vods::Chapter {
                    name: Some("Game B".into()),
                    image: None,
                    start: Some(200.0),
                },
            ]),
            youtube: None,
        };
        let tags = get_game_tags(&vod);
        let names: Vec<&str> = tags.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, vec!["Game A", "Game B"]);
        assert_eq!(tags[0].start_seconds, 0);
        assert_eq!(tags[1].start_seconds, 200);
    }

    #[test]
    fn test_get_game_tags_case_insensitive() {
        let vod = Vod {
            id: "1".into(),
            title: Some("Test".into()),
            created_at: "2025-01-01T00:00:00Z".into(),
            duration: Some("2h".into()),
            thumbnail_url: None,
            chapters: Some(vec![
                crate::vods::Chapter {
                    name: Some("Elden Ring".into()),
                    image: None,
                    start: Some(0.0),
                },
                crate::vods::Chapter {
                    name: Some("ELDEN RING".into()),
                    image: None,
                    start: Some(500.0),
                },
            ]),
            youtube: None,
        };
        let tags = get_game_tags(&vod);
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].name, "Elden Ring");
    }

    #[test]
    fn test_vod_has_game() {
        let vod = Vod {
            id: "1".into(),
            title: Some("Test".into()),
            created_at: "2025-01-01T00:00:00Z".into(),
            duration: Some("2h".into()),
            thumbnail_url: None,
            chapters: Some(vec![crate::vods::Chapter {
                name: Some("Elden Ring".into()),
                image: None,
                start: None,
            }]),
            youtube: None,
        };
        assert!(vod_has_game(&vod, "Elden Ring"));
        assert!(vod_has_game(&vod, "elden ring"));
        assert!(vod_has_game(&vod, "ELDEN RING"));
        assert!(!vod_has_game(&vod, "Dark Souls"));
    }

    #[test]
    fn test_find_game_image_is_case_insensitive() {
        let games = vec![
            Game {
                name: "Elden Ring".into(),
                image: Some("elden.jpg".into()),
                vod_count: 2,
            },
            Game {
                name: "Dark Souls".into(),
                image: None,
                vod_count: 1,
            },
        ];
        assert_eq!(
            find_game_image(&games, "elden ring").as_deref(),
            Some("elden.jpg")
        );
        assert_eq!(
            find_game_image(&games, "ELDEN RING").as_deref(),
            Some("elden.jpg")
        );
        assert_eq!(find_game_image(&games, "Sekiro"), None);
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

    fn make_display(id: &str, created_at: &str) -> VodDisplay {
        VodDisplay {
            id: id.into(),
            display_title: "t".into(),
            formatted_date: "".into(),
            duration: None,
            thumbnail_url: None,
            game_tags: vec![],
            created_at: created_at.into(),
            duration_minutes: 0,
            duration_seconds: 0,
            period_header: None,
            watch_url: format!("/watch/{id}"),
        }
    }

    #[test]
    fn test_parse_ymd_to_days() {
        let a = parse_ymd_to_days("2024-01-01T00:00:00Z").unwrap();
        let b = parse_ymd_to_days("2024-01-15T00:00:00Z").unwrap();
        assert_eq!(b - a, 14);
        let c = parse_ymd_to_days("2025-01-01T00:00:00Z").unwrap();
        assert_eq!(c - a, 366); // 2024 is a leap year
        assert!(parse_ymd_to_days("bogus").is_none());
    }

    #[test]
    fn test_assign_period_headers_splits_by_gap() {
        let mut displays = vec![
            make_display("1", "2024-03-10T00:00:00Z"),
            make_display("2", "2024-03-05T00:00:00Z"),
            make_display("3", "2024-01-20T00:00:00Z"),
            make_display("4", "2024-01-15T00:00:00Z"),
        ];
        assign_period_headers(&mut displays, "newest");
        assert!(displays[0].period_header.is_some());
        assert!(displays[1].period_header.is_none());
        assert!(displays[2].period_header.is_some());
        assert!(displays[3].period_header.is_none());
        assert!(
            displays[0]
                .period_header
                .as_ref()
                .unwrap()
                .contains("2 streams")
        );
    }

    #[test]
    fn test_assign_period_headers_skips_when_single_cluster() {
        let mut displays = vec![
            make_display("1", "2024-03-10T00:00:00Z"),
            make_display("2", "2024-03-05T00:00:00Z"),
            make_display("3", "2024-03-01T00:00:00Z"),
        ];
        assign_period_headers(&mut displays, "newest");
        assert!(displays.iter().all(|d| d.period_header.is_none()));
    }

    fn display_with_games(id: &str, games: &[&str]) -> VodDisplay {
        let mut d = make_display(id, "2024-01-01T00:00:00Z");
        d.game_tags = games
            .iter()
            .map(|g| GameTag {
                name: (*g).into(),
                start_seconds: 0,
            })
            .collect();
        d
    }

    fn keys_from_first_tag(displays: &[VodDisplay]) -> Vec<Option<String>> {
        displays
            .iter()
            .map(|d| d.game_tags.first().map(|t| t.name.clone()))
            .collect()
    }

    #[test]
    fn test_assign_series_headers_groups_consecutive() {
        let mut displays = vec![
            display_with_games("1", &["Elden Ring"]),
            display_with_games("2", &["Elden Ring"]),
            display_with_games("3", &["Dark Souls"]),
            display_with_games("4", &["Elden Ring"]),
        ];
        let keys = keys_from_first_tag(&displays);
        assign_series_headers(&mut displays, &keys);
        assert_eq!(
            displays[0].period_header.as_deref(),
            Some("Elden Ring · 2 streams")
        );
        assert!(displays[1].period_header.is_none());
        assert_eq!(
            displays[2].period_header.as_deref(),
            Some("Dark Souls · 1 stream")
        );
        assert_eq!(
            displays[3].period_header.as_deref(),
            Some("Elden Ring · 1 stream")
        );
    }

    #[test]
    fn test_assign_series_headers_handles_untagged() {
        let mut displays = vec![
            display_with_games("1", &[]),
            display_with_games("2", &[]),
            display_with_games("3", &["Elden Ring"]),
        ];
        let keys = keys_from_first_tag(&displays);
        assign_series_headers(&mut displays, &keys);
        assert_eq!(
            displays[0].period_header.as_deref(),
            Some("Untagged · 2 streams")
        );
        assert!(displays[1].period_header.is_none());
        assert_eq!(
            displays[2].period_header.as_deref(),
            Some("Elden Ring · 1 stream")
        );
    }

    #[test]
    fn test_assign_series_headers_case_insensitive() {
        let mut displays = vec![
            display_with_games("1", &["Elden Ring"]),
            display_with_games("2", &["elden ring"]),
        ];
        let keys = keys_from_first_tag(&displays);
        assign_series_headers(&mut displays, &keys);
        assert_eq!(
            displays[0].period_header.as_deref(),
            Some("Elden Ring · 2 streams")
        );
        assert!(displays[1].period_header.is_none());
    }

    #[test]
    fn test_assign_series_headers_honours_custom_keys() {
        // Same first-chapter on both VODs, but the resume time puts the user
        // in a later chapter on one of them — the two VODs should land in
        // separate groups.
        let mut displays = vec![
            display_with_games("1", &["Just Chatting", "Terraria"]),
            display_with_games("2", &["Just Chatting", "Terraria"]),
        ];
        let keys = vec![
            Some("Just Chatting".to_string()),
            Some("Terraria".to_string()),
        ];
        assign_series_headers(&mut displays, &keys);
        assert_eq!(
            displays[0].period_header.as_deref(),
            Some("Just Chatting · 1 stream")
        );
        assert_eq!(
            displays[1].period_header.as_deref(),
            Some("Terraria · 1 stream")
        );
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
            title: None,
            created_at: "2024-01-01T00:00:00Z".into(),
            duration: None,
            thumbnail_url: None,
            chapters: Some(vec![]),
            youtube: None,
        };
        assert!(resolve_watched_chapter(&vod, Some(100)).is_none());
    }

    #[test]
    fn test_assign_period_headers_skips_for_non_chronological_sort() {
        let mut displays = vec![
            make_display("1", "2024-03-10T00:00:00Z"),
            make_display("2", "2024-01-01T00:00:00Z"),
        ];
        assign_period_headers(&mut displays, "longest");
        assert!(displays.iter().all(|d| d.period_header.is_none()));
    }

    fn make_vod(id: &str, created_at: &str, games: &[&str]) -> Vod {
        Vod {
            id: id.into(),
            title: Some(format!("vod {id}")),
            created_at: created_at.into(),
            duration: Some("1h".into()),
            thumbnail_url: None,
            chapters: Some(
                games
                    .iter()
                    .map(|g| crate::vods::Chapter {
                        name: Some((*g).into()),
                        image: None,
                        start: Some(0.0),
                    })
                    .collect(),
            ),
            youtube: None,
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
        };
        let url = build_next_url("/games", 1, &params);
        assert!(url.starts_with("/games?"));
        assert!(url.contains("page=1"));
        assert!(url.contains("search=test"));
        assert!(url.contains("sort=most"));
    }
}
