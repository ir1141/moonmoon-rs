use super::{
    ChapterSegment, Section, build_watch_url, days_to_civil, get_chapter_segments,
    parse_ymd_to_days, render_template, vod_stream_time,
};
use crate::SharedState;
use crate::middleware::CspNonce;
use crate::vods::Vod;
use askama::Template;
use axum::extract::{Extension, Query, State};
use axum::response::IntoResponse;
use serde::Deserialize;
use std::sync::Arc;

#[derive(Deserialize, Default)]
pub struct CalendarQuery {
    pub week: Option<String>,
    pub year: Option<i32>,
    pub month: Option<u32>,
}

struct AxisTick {
    label: &'static str,
    left_pct: f64,
}

struct GuideSeg {
    name: String,
    width_pct: f64,
    color_idx: u8,
}

struct GuideBlock {
    left_pct: f64,
    width_pct: f64,
    range: String,
    total: String,
    primary_game: String,
    segments: Vec<GuideSeg>,
    watch_url: String,
}

struct GuideDay {
    weekday: &'static str,
    date_label: String,
    is_off: bool,
    blocks: Vec<GuideBlock>,
}

struct TimeGuideView {
    week_label: String,
    prev_week: String,
    next_week: String,
    has_next: bool,
    timezone_note: &'static str,
    axis_ticks: Vec<AxisTick>,
    days: Vec<GuideDay>,
}

#[derive(Template)]
#[template(path = "calendar.html")]
struct CalendarPageTemplate {
    guide: TimeGuideView,
    active_section: Section,
    nonce: String,
}

const AXIS_START_HOUR: f64 = 12.0;
const AXIS_END_HOUR: f64 = 24.0;
const SECONDS_PER_DAY: i64 = 86_400;
const PACIFIC_STANDARD_OFFSET_SECS: i64 = -8 * 3600;
const PACIFIC_DAYLIGHT_OFFSET_SECS: i64 = -7 * 3600;
const TIMEZONE_NOTE: &str = "Times in PT";

fn month_name(m: u32) -> &'static str {
    match m {
        1 => "January",
        2 => "February",
        3 => "March",
        4 => "April",
        5 => "May",
        6 => "June",
        7 => "July",
        8 => "August",
        9 => "September",
        10 => "October",
        11 => "November",
        12 => "December",
        _ => "Unknown",
    }
}

fn short_month_name(m: u32) -> &'static str {
    match m {
        1 => "Jan",
        2 => "Feb",
        3 => "Mar",
        4 => "Apr",
        5 => "May",
        6 => "Jun",
        7 => "Jul",
        8 => "Aug",
        9 => "Sep",
        10 => "Oct",
        11 => "Nov",
        12 => "Dec",
        _ => "???",
    }
}

// Tomohiko Sakamoto's algorithm: day of week for any date (0=Sun..6=Sat)
fn day_of_week(year: i32, month: u32, day: u32) -> u32 {
    let t = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let y = if month < 3 { year - 1 } else { year };
    ((y + y / 4 - y / 100 + y / 400 + t[(month - 1) as usize] + day as i32) % 7) as u32
}

fn calendar_duration_display(total_minutes: i64) -> String {
    if total_minutes >= 60 {
        format!("{}h {}m", total_minutes / 60, total_minutes % 60)
    } else {
        format!("{total_minutes}m")
    }
}

fn parse_date_query_days(value: &str) -> Option<i64> {
    parse_ymd_to_days(value)
}

fn date_query(days: i64) -> String {
    let (year, month, day) = days_to_civil(days);
    format!("{year:04}-{month:02}-{day:02}")
}

fn date_label(days: i64) -> String {
    let (_, month, day) = days_to_civil(days);
    format!("{} {day}", short_month_name(month))
}

fn weekday_label(days: i64) -> &'static str {
    let (year, month, day) = days_to_civil(days);
    match day_of_week(year, month, day) {
        0 => "Sun",
        1 => "Mon",
        2 => "Tue",
        3 => "Wed",
        4 => "Thu",
        5 => "Fri",
        _ => "Sat",
    }
}

fn week_start_for_days(days: i64) -> i64 {
    let (year, month, day) = days_to_civil(days);
    days - i64::from(day_of_week(year, month, day))
}

fn format_week_label(week_start: i64) -> String {
    let week_end = week_start + 6;
    let (start_year, start_month, start_day) = days_to_civil(week_start);
    let (end_year, end_month, end_day) = days_to_civil(week_end);

    if start_year == end_year && start_month == end_month {
        format!(
            "{} {start_day} - {end_day}, {start_year}",
            month_name(start_month)
        )
    } else if start_year == end_year {
        format!(
            "{} {start_day} - {} {end_day}, {start_year}",
            month_name(start_month),
            month_name(end_month)
        )
    } else {
        format!(
            "{} {start_day}, {start_year} - {} {end_day}, {end_year}",
            month_name(start_month),
            month_name(end_month)
        )
    }
}

fn axis_ticks() -> Vec<AxisTick> {
    [
        ("12p", 12.0),
        ("2p", 14.0),
        ("4p", 16.0),
        ("6p", 18.0),
        ("8p", 20.0),
        ("10p", 22.0),
        ("12a", 24.0),
    ]
    .into_iter()
    .map(|(label, hour)| AxisTick {
        label,
        left_pct: ((hour - AXIS_START_HOUR) / (AXIS_END_HOUR - AXIS_START_HOUR)) * 100.0,
    })
    .collect()
}

fn selected_week_start(params: &CalendarQuery, current_local_days: i64) -> i64 {
    let selected_days = params
        .week
        .as_deref()
        .and_then(parse_date_query_days)
        .or_else(|| {
            let year = params.year?;
            let month = params.month?;
            parse_date_query_days(&format!("{year:04}-{month:02}-01"))
        })
        .unwrap_or(current_local_days);

    let current_week_start = week_start_for_days(current_local_days);
    let min_week_start =
        week_start_for_days(parse_date_query_days("2015-01-01").unwrap_or(current_week_start));

    week_start_for_days(selected_days).clamp(min_week_start, current_week_start)
}

#[derive(Clone, Copy)]
struct PacificLocalTime {
    days: i64,
    seconds_of_day: i64,
}

fn parse_utc_timestamp(timestamp: &str) -> Option<(i64, i64)> {
    let days = parse_ymd_to_days(timestamp)?;
    let hour = timestamp.get(11..13)?.parse::<i64>().ok()?;
    let minute = timestamp.get(14..16)?.parse::<i64>().ok()?;
    let second = timestamp.get(17..19)?.parse::<i64>().ok()?;
    if !(0..=23).contains(&hour) || !(0..=59).contains(&minute) || !(0..=60).contains(&second) {
        return None;
    }
    Some((days, hour * 3600 + minute * 60 + second.min(59)))
}

fn date_to_days(year: i32, month: u32, day: u32) -> i64 {
    parse_date_query_days(&format!("{year:04}-{month:02}-{day:02}")).unwrap_or_default()
}

fn nth_weekday_of_month(year: i32, month: u32, weekday: u32, n: u32) -> u32 {
    let first_weekday = day_of_week(year, month, 1);
    let offset = (weekday + 7 - first_weekday) % 7;
    1 + offset + (n - 1) * 7
}

fn pacific_dst_bounds_utc(year: i32) -> (i64, i64) {
    let dst_start_day = nth_weekday_of_month(year, 3, 0, 2);
    let dst_end_day = nth_weekday_of_month(year, 11, 0, 1);
    let start_utc = date_to_days(year, 3, dst_start_day) * SECONDS_PER_DAY + 10 * 3600;
    let end_utc = date_to_days(year, 11, dst_end_day) * SECONDS_PER_DAY + 9 * 3600;
    (start_utc, end_utc)
}

fn pacific_offset_seconds(utc_unix_seconds: i64) -> i64 {
    let (year, _, _) = days_to_civil(utc_unix_seconds.div_euclid(SECONDS_PER_DAY));
    let (dst_start, dst_end) = pacific_dst_bounds_utc(year);
    if utc_unix_seconds >= dst_start && utc_unix_seconds < dst_end {
        PACIFIC_DAYLIGHT_OFFSET_SECS
    } else {
        PACIFIC_STANDARD_OFFSET_SECS
    }
}

fn pacific_local_from_unix_seconds(utc_unix_seconds: i64) -> PacificLocalTime {
    let local_seconds = utc_unix_seconds + pacific_offset_seconds(utc_unix_seconds);
    PacificLocalTime {
        days: local_seconds.div_euclid(SECONDS_PER_DAY),
        seconds_of_day: local_seconds.rem_euclid(SECONDS_PER_DAY),
    }
}

fn pacific_local_from_timestamp(timestamp: &str) -> Option<PacificLocalTime> {
    let (days, seconds_of_day) = parse_utc_timestamp(timestamp)?;
    Some(pacific_local_from_unix_seconds(
        days * SECONDS_PER_DAY + seconds_of_day,
    ))
}

struct RawSession<'a> {
    vod: &'a Vod,
    local: PacificLocalTime,
    duration_seconds: i64,
}

fn color_idx_for_name(name: &str) -> u8 {
    let mut h: u32 = 5381;
    for b in name.as_bytes() {
        h = h
            .wrapping_mul(33)
            .wrapping_add(u32::from(b.to_ascii_lowercase()));
    }
    (h % 8) as u8
}

fn fallback_segment(vod: &Vod) -> GuideSeg {
    let name = vod
        .chapters
        .as_deref()
        .and_then(|chapters| chapters.iter().find_map(|ch| ch.name.clone()))
        .or_else(|| vod.title.clone())
        .unwrap_or_else(|| "Stream".to_string());
    GuideSeg {
        color_idx: color_idx_for_name(&name),
        name,
        width_pct: 100.0,
    }
}

fn guide_segments(vod: &Vod, duration_seconds: i64) -> Vec<GuideSeg> {
    let segments = get_chapter_segments(vod, duration_seconds);
    if segments.is_empty() {
        return vec![fallback_segment(vod)];
    }
    segments
        .into_iter()
        .map(
            |ChapterSegment {
                 name,
                 width_pct,
                 color_idx,
                 ..
             }| GuideSeg {
                name,
                width_pct,
                color_idx,
            },
        )
        .collect()
}

fn primary_game(segments: &[GuideSeg]) -> String {
    segments
        .iter()
        .max_by(|a, b| a.width_pct.total_cmp(&b.width_pct))
        .map(|segment| segment.name.clone())
        .unwrap_or_else(|| "Stream".to_string())
}

fn format_clock(seconds: i64, include_meridiem: bool) -> String {
    let minute_of_day = seconds.div_euclid(60).rem_euclid(24 * 60);
    let hour24 = minute_of_day / 60;
    let minute = minute_of_day % 60;
    let hour12 = match hour24 % 12 {
        0 => 12,
        hour => hour,
    };
    let meridiem = if hour24 < 12 { "AM" } else { "PM" };
    if include_meridiem {
        format!("{hour12}:{minute:02} {meridiem}")
    } else {
        format!("{hour12}:{minute:02}")
    }
}

fn format_time_range(start_seconds: i64, duration_seconds: i64) -> String {
    let end_seconds = start_seconds + duration_seconds.max(0);
    let start_meridiem = start_seconds.div_euclid(3600).rem_euclid(24) < 12;
    let end_meridiem = end_seconds.div_euclid(3600).rem_euclid(24) < 12;
    let include_start_meridiem = start_meridiem != end_meridiem;
    format!(
        "{} - {}",
        format_clock(start_seconds, include_start_meridiem),
        format_clock(end_seconds, true)
    )
}

fn block_position(start_seconds: i64, duration_seconds: i64) -> (f64, f64) {
    let start_hour = start_seconds as f64 / 3600.0;
    let end_hour = start_hour + duration_seconds.max(0) as f64 / 3600.0;
    let clipped_start = start_hour.clamp(AXIS_START_HOUR, AXIS_END_HOUR);
    let clipped_end = end_hour.clamp(AXIS_START_HOUR, AXIS_END_HOUR);
    let axis_span = AXIS_END_HOUR - AXIS_START_HOUR;
    (
        ((clipped_start - AXIS_START_HOUR) / axis_span) * 100.0,
        ((clipped_end - clipped_start).max(0.0) / axis_span) * 100.0,
    )
}

fn build_guide_block(session: &RawSession<'_>, watch_url: String) -> GuideBlock {
    let segments = guide_segments(session.vod, session.duration_seconds);
    let primary_game = primary_game(&segments);
    let (left_pct, width_pct) =
        block_position(session.local.seconds_of_day, session.duration_seconds);
    GuideBlock {
        left_pct,
        width_pct,
        range: format_time_range(session.local.seconds_of_day, session.duration_seconds),
        total: calendar_duration_display(session.duration_seconds / 60),
        primary_game,
        segments,
        watch_url,
    }
}

fn build_time_guide(vods: &[Vod], week_start: i64, current_week_start: i64) -> TimeGuideView {
    let mut sessions_by_day: Vec<Vec<RawSession<'_>>> = (0..7).map(|_| Vec::new()).collect();

    for vod in vods {
        let Some(local) = pacific_local_from_timestamp(vod_stream_time(vod)) else {
            continue;
        };
        let duration_seconds = vod
            .duration
            .as_ref()
            .map_or(0, |duration| duration.seconds());
        if duration_seconds <= 0 {
            continue;
        }
        if local.days < week_start || local.days >= week_start + 7 {
            continue;
        }
        let day_idx = (local.days - week_start) as usize;
        sessions_by_day[day_idx].push(RawSession {
            vod,
            local,
            duration_seconds,
        });
    }

    let days = sessions_by_day
        .into_iter()
        .enumerate()
        .map(|(idx, mut sessions)| {
            let day_days = week_start + idx as i64;
            sessions.sort_by_key(|session| session.local.seconds_of_day);
            let stream_url = format!(
                "/streams?from={date}&to={date}",
                date = date_query(day_days)
            );
            let single_session = sessions.len() == 1;
            let blocks = sessions
                .iter()
                .map(|session| {
                    let watch_url = if single_session {
                        build_watch_url(&session.vod.id, None, None)
                    } else {
                        stream_url.clone()
                    };
                    build_guide_block(session, watch_url)
                })
                .collect::<Vec<_>>();

            GuideDay {
                weekday: weekday_label(day_days),
                date_label: date_label(day_days),
                is_off: blocks.is_empty(),
                blocks,
            }
        })
        .collect();

    TimeGuideView {
        week_label: format_week_label(week_start),
        prev_week: date_query(week_start - 7),
        next_week: date_query(week_start + 7),
        has_next: week_start < current_week_start,
        timezone_note: TIMEZONE_NOTE,
        axis_ticks: axis_ticks(),
        days,
    }
}

pub async fn calendar_page(
    State(state): State<SharedState>,
    Extension(nonce): Extension<CspNonce>,
    Query(params): Query<CalendarQuery>,
) -> impl IntoResponse {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let current_local_days = pacific_local_from_unix_seconds(now).days;
    let current_week_start = week_start_for_days(current_local_days);
    let week_start = selected_week_start(&params, current_local_days);

    let vods = {
        let guard = state.vods.read().await;
        Arc::clone(&*guard)
    };
    let guide = build_time_guide(&vods, week_start, current_week_start);

    let tmpl = CalendarPageTemplate {
        guide,
        active_section: Section::Calendar,
        nonce: nonce.0,
    };
    render_template(&tmpl)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vods::{Chapter, Vod, VodDuration};

    #[test]
    fn test_calendar_helpers() {
        assert_eq!(day_of_week(2025, 1, 1), 3);
        assert_eq!(day_of_week(2025, 3, 1), 6);
        assert_eq!(day_of_week(2024, 1, 1), 1);

        assert_eq!(month_name(1), "January");
        assert_eq!(month_name(12), "December");
    }

    fn test_vod(id: &str, started_at: &str, duration_secs: i64, chapters: Vec<Chapter>) -> Vod {
        Vod {
            id: id.into(),
            platform: Some("twitch".into()),
            platform_vod_id: None,
            platform_stream_id: None,
            title: Some("Playable Stream".into()),
            created_at: started_at.into(),
            started_at: Some(started_at.into()),
            updated_at: None,
            duration: Some(VodDuration::from_seconds(duration_secs)),
            thumbnail_url: None,
            chapters: Some(chapters),
            youtube: None,
            is_live: false,
        }
    }

    fn assert_close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 0.01,
            "expected {actual} to be close to {expected}"
        );
    }

    #[test]
    fn test_time_guide_uses_existing_sunday_week_start() {
        let selected = parse_date_query_days("2026-05-26").unwrap();
        let week_start = week_start_for_days(selected);

        assert_eq!(date_query(week_start), "2026-05-24");

        let guide = build_time_guide(&[], week_start, week_start + 7);

        assert_eq!(guide.week_label, "May 24 - 30, 2026");
        assert_eq!(guide.prev_week, "2026-05-17");
        assert_eq!(guide.next_week, "2026-05-31");
        assert!(guide.has_next);
        assert_eq!(guide.timezone_note, "Times in PT");
        assert_eq!(
            guide
                .axis_ticks
                .iter()
                .map(|tick| tick.label)
                .collect::<Vec<_>>(),
            ["12p", "2p", "4p", "6p", "8p", "10p", "12a"]
        );
        assert_eq!(guide.days[0].weekday, "Sun");
        assert_eq!(guide.days[6].weekday, "Sat");
        assert!(guide.days.iter().all(|day| day.is_off));
    }

    #[test]
    fn test_time_guide_places_utc_streams_on_pt_axis_with_segments() {
        let week_start = parse_date_query_days("2026-05-24").unwrap();
        let vod = test_vod(
            "v1",
            "2026-05-25T20:30:00.000Z",
            6 * 3600 + 20 * 60,
            vec![
                Chapter {
                    name: Some("Elden Ring".into()),
                    image: None,
                    start: Some(0.0),
                    duration: Some(4.0 * 3600.0),
                    end: None,
                },
                Chapter {
                    name: Some("Schedule I".into()),
                    image: None,
                    start: Some(4.0 * 3600.0),
                    duration: Some(2.0 * 3600.0 + 20.0 * 60.0),
                    end: None,
                },
            ],
        );

        let guide = build_time_guide(&[vod], week_start, week_start + 7);
        let monday = &guide.days[1];
        let block = &monday.blocks[0];

        assert!(!monday.is_off);
        assert_eq!(monday.date_label, "May 25");
        assert_eq!(block.range, "1:30 - 7:50 PM");
        assert_eq!(block.total, "6h 20m");
        assert_eq!(block.primary_game, "Elden Ring");
        assert_eq!(block.watch_url, "/watch/v1");
        assert_close(block.left_pct, 12.5);
        assert_close(block.width_pct, 52.77);
        assert_eq!(block.segments.len(), 2);
        assert_eq!(block.segments[0].name, "Elden Ring");
        assert_close(block.segments[0].width_pct, 63.15);
        assert_eq!(block.segments[1].name, "Schedule I");
        assert_close(block.segments[1].width_pct, 36.84);
    }
}
