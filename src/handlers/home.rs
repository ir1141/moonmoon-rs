use super::{
    Section, VodDisplay, current_utc_days, days_to_civil, format_date, parse_ymd_to_days,
    render_template, vod_stream_time,
};
use crate::SharedState;
use crate::middleware::CspNonce;
use crate::vods::{Game, Vod, chapter_color_idx};
use askama::Template;
use axum::extract::{Extension, State};
use axum::response::IntoResponse;
use std::sync::Arc;

/// How many cards each landing rail shows before the user has to "See all".
const RECENT_RAIL_SIZE: usize = 12;
const GAMES_RAIL_SIZE: usize = 12;
/// Top games surfaced as quick-filter chips (followed by the "This week" lens).
const CHIP_GAME_COUNT: usize = 4;
/// Sliding window (inclusive of today) for the "new this week" stat and chip.
const WEEK_DAYS: i64 = 7;
/// Shown when the catalog is empty so the hero never reads "since 0".
const FALLBACK_START_YEAR: i32 = 2019;

#[derive(Template)]
#[template(path = "landing.html")]
struct HomePageTemplate {
    total_vods_label: String,
    total_games: usize,
    live: bool,
    new_this_week: usize,
    archive_since: i32,
    recent_vods: Vec<VodDisplay>,
    top_games: Vec<Game>,
    chips: Vec<HomeChip>,
    on_this_day: Option<OnThisDayView>,
    today_label: String,
    // Rail rendering flags consumed by the shared vod_card.html / game_card.html
    // partials: tag multi-game VODs, but drop the per-game recency line on tiles.
    show_game_tags: bool,
    show_recency: bool,
    show_oldest_recency: bool,
    active_section: Section,
    nonce: String,
}

struct HomeChip {
    label: String,
    count_label: String,
    href: String,
    is_game: bool,
    color_idx: u8,
}

struct OnThisDayView {
    years_ago_label: String,
    date_label: String,
    game: String,
    duration: Option<String>,
    title: String,
    watch_url: String,
}

pub async fn home_page(
    State(state): State<SharedState>,
    Extension(nonce): Extension<CspNonce>,
) -> impl IntoResponse {
    let vods = {
        let guard = state.vods.read().await;
        Arc::clone(&*guard)
    };
    let games = {
        let guard = state.games.read().await;
        Arc::clone(&*guard)
    };

    let today_days = current_utc_days();
    let (today_year, today_month, today_day) = days_to_civil(today_days);
    let week_from = date_string(today_days - (WEEK_DAYS - 1));

    let new_this_week = count_streams_since(&vods, &week_from);

    // `games` is pre-sorted by VOD count (desc), so the most-streamed rail and the
    // chips are just prefixes — no re-sorting needed.
    let mut chips: Vec<HomeChip> = games
        .iter()
        .take(CHIP_GAME_COUNT)
        .map(|game| HomeChip {
            label: game.name.clone(),
            count_label: game.vod_count.to_string(),
            href: format!("/game/{}", urlencoding::encode(&game.name)),
            is_game: true,
            color_idx: chapter_color_idx(&game.name),
        })
        .collect();
    chips.push(HomeChip {
        label: "This week".to_string(),
        count_label: new_this_week.to_string(),
        href: format!("/streams?from={week_from}"),
        is_game: false,
        color_idx: 0,
    });

    let on_this_day =
        find_on_this_day(&vods, today_year, today_month, today_day).map(|(idx, matched_year)| {
            let display = VodDisplay::from_vod(&vods[idx]);
            let years_ago = (today_year - matched_year).max(1);
            OnThisDayView {
                years_ago_label: if years_ago == 1 {
                    "1 year ago".to_string()
                } else {
                    format!("{years_ago} years ago")
                },
                date_label: display.formatted_date,
                game: display
                    .chapter_names
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "a stream".to_string()),
                duration: display.duration,
                title: display.display_title,
                watch_url: display.watch_url,
            }
        });

    render_template(&HomePageTemplate {
        total_vods_label: format_thousands(vods.len()),
        total_games: games.len(),
        live: vods.iter().any(|vod| vod.is_live),
        new_this_week,
        archive_since: archive_start_year(&vods),
        recent_vods: vods
            .iter()
            .take(RECENT_RAIL_SIZE)
            .map(VodDisplay::from_vod)
            .collect(),
        top_games: games.iter().take(GAMES_RAIL_SIZE).cloned().collect(),
        chips,
        on_this_day,
        today_label: format_date(&date_string(today_days)),
        show_game_tags: true,
        show_recency: false,
        show_oldest_recency: false,
        active_section: Section::Home,
        nonce: nonce.0,
    })
}

/// `YYYY-MM-DD` for a day offset from the Unix epoch.
fn date_string(days: i64) -> String {
    let (year, month, day) = days_to_civil(days);
    format!("{year:04}-{month:02}-{day:02}")
}

/// Group the integer's digits with thousands separators, e.g. `2841` → `"2,841"`.
fn format_thousands(n: usize) -> String {
    let digits = n.to_string();
    let len = digits.len();
    let mut out = String::with_capacity(len + (len.saturating_sub(1)) / 3);
    for (i, ch) in digits.char_indices() {
        if i > 0 && (len - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

/// Count streams whose stream date is on or after `from_date` (`YYYY-MM-DD`).
/// Uses the same stream-time field and inclusive lower bound as the `/streams`
/// `?from=` filter, so the "This week" chip count matches its destination.
fn count_streams_since(vods: &[Vod], from_date: &str) -> usize {
    vods.iter()
        .filter(|vod| {
            vod_stream_time(vod)
                .get(..10)
                .is_some_and(|date| date >= from_date)
        })
        .count()
}

/// Earliest stream year in the catalog, falling back to a sensible constant.
fn archive_start_year(vods: &[Vod]) -> i32 {
    vods.iter()
        .filter_map(|vod| vod_stream_time(vod).get(..4)?.parse::<i32>().ok())
        .min()
        .unwrap_or(FALLBACK_START_YEAR)
}

/// The best "on this day" VOD as `(index, year)`: same month/day as today, an
/// earlier year, preferring the most recent anniversary and then the longest
/// stream. Returning the year too spares the caller a redundant date re-parse.
fn find_on_this_day(
    vods: &[Vod],
    today_year: i32,
    today_month: u32,
    today_day: u32,
) -> Option<(usize, i32)> {
    let mut best: Option<(usize, i32, i64)> = None;
    for (idx, vod) in vods.iter().enumerate() {
        let Some(days) = parse_ymd_to_days(vod_stream_time(vod)) else {
            continue;
        };
        let (year, month, day) = days_to_civil(days);
        if month != today_month || day != today_day || year >= today_year {
            continue;
        }
        let duration = vod.duration.as_ref().map_or(0, |d| d.seconds());
        let better = match best {
            None => true,
            Some((_, best_year, best_duration)) => {
                year > best_year || (year == best_year && duration > best_duration)
            }
        };
        if better {
            best = Some((idx, year, duration));
        }
    }
    best.map(|(idx, year, _)| (idx, year))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vods::{Chapter, Vod};

    fn vod(id: &str, created_at: &str, duration: &str, games: &[&str]) -> Vod {
        Vod {
            id: id.into(),
            platform: None,
            platform_vod_id: None,
            platform_stream_id: None,
            title: Some(format!("Stream {id}")),
            created_at: created_at.into(),
            started_at: None,
            updated_at: None,
            duration: Some(duration.into()),
            thumbnail_url: None,
            chapters: Some(
                games
                    .iter()
                    .map(|name| Chapter {
                        name: Some((*name).into()),
                        image: None,
                        start: Some(0.0),
                        duration: None,
                        end: None,
                    })
                    .collect(),
            ),
            youtube: None,
            is_live: false,
        }
    }

    #[test]
    fn format_thousands_groups_digits() {
        assert_eq!(format_thousands(0), "0");
        assert_eq!(format_thousands(42), "42");
        assert_eq!(format_thousands(247), "247");
        assert_eq!(format_thousands(2841), "2,841");
        assert_eq!(format_thousands(1_000_000), "1,000,000");
    }

    #[test]
    fn count_streams_since_uses_inclusive_lower_bound() {
        let vods = vec![
            vod("a", "2026-06-08T00:00:00Z", "1h", &["A"]),
            vod("b", "2026-06-02T00:00:00Z", "1h", &["B"]),
            vod("c", "2026-06-01T00:00:00Z", "1h", &["C"]),
            vod("d", "2025-06-08T00:00:00Z", "1h", &["D"]),
        ];
        // Window opens 2026-06-02: a and b qualify, c (day before) and d (last year) do not.
        assert_eq!(count_streams_since(&vods, "2026-06-02"), 2);
    }

    #[test]
    fn archive_start_year_picks_earliest_or_falls_back() {
        let vods = vec![
            vod("a", "2026-01-01T00:00:00Z", "1h", &["A"]),
            vod("b", "2019-07-04T00:00:00Z", "1h", &["B"]),
            vod("c", "2022-03-03T00:00:00Z", "1h", &["C"]),
        ];
        assert_eq!(archive_start_year(&vods), 2019);
        assert_eq!(archive_start_year(&[]), FALLBACK_START_YEAR);
    }

    #[test]
    fn find_on_this_day_prefers_recent_anniversary_then_longest() {
        let vods = vec![
            vod("old", "2022-06-08T12:00:00Z", "2h", &["Subnautica"]),
            vod(
                "recent-short",
                "2024-06-08T12:00:00Z",
                "1h",
                &["Elden Ring"],
            ),
            vod("recent-long", "2024-06-08T20:00:00Z", "5h", &["Elden Ring"]),
            vod("wrong-day", "2023-06-09T12:00:00Z", "9h", &["Sekiro"]),
            vod("this-year", "2026-06-08T12:00:00Z", "9h", &["Minecraft"]),
        ];
        // 2024 beats 2022; within 2024 the longer stream wins; today (2026) is excluded.
        let (idx, year) = find_on_this_day(&vods, 2026, 6, 8).unwrap();
        assert_eq!(vods[idx].id, "recent-long");
        assert_eq!(year, 2024);
    }

    #[test]
    fn find_on_this_day_returns_none_without_a_match() {
        let vods = vec![vod("a", "2024-12-25T12:00:00Z", "1h", &["A"])];
        assert!(find_on_this_day(&vods, 2026, 6, 8).is_none());
    }
}
