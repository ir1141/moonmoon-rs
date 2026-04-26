use super::{Section, days_to_civil, parse_duration_minutes, render_template};
use crate::SharedState;
use askama::Template;
use axum::extract::{Query, State};
use axum::response::IntoResponse;
use serde::Deserialize;
use std::sync::Arc;

#[derive(Deserialize, Default)]
pub struct CalendarQuery {
    pub year: Option<i32>,
    pub month: Option<u32>,
}

pub struct CalendarGame {
    pub name: String,
    pub image: Option<String>,
}

pub struct CalendarDay {
    pub date: u32,
    pub games: Vec<CalendarGame>,
    pub duration_display: String,
    pub glow: f64,
    pub date_str: String,
}

#[derive(Template)]
#[template(path = "calendar.html")]
struct CalendarPageTemplate {
    year: i32,
    month_name: &'static str,
    first_weekday: u32,
    days: Vec<Option<CalendarDay>>,
    prev_year: i32,
    prev_month: u32,
    next_year: i32,
    next_month: u32,
    has_next: bool,
    today_day: usize,
    active_section: Section,
}

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

fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}

// Tomohiko Sakamoto's algorithm: day of week for any date (0=Sun..6=Sat)
fn day_of_week(year: i32, month: u32, day: u32) -> u32 {
    let t = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let y = if month < 3 { year - 1 } else { year };
    ((y + y / 4 - y / 100 + y / 400 + t[(month - 1) as usize] + day as i32) % 7) as u32
}

fn prev_month(year: i32, month: u32) -> (i32, u32) {
    if month == 1 {
        (year - 1, 12)
    } else {
        (year, month - 1)
    }
}

fn next_month(year: i32, month: u32) -> (i32, u32) {
    if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    }
}

pub async fn calendar_page(
    State(state): State<SharedState>,
    Query(params): Query<CalendarQuery>,
) -> impl IntoResponse {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let days_since_epoch = (now / 86400) as i64;
    let (cur_year, cur_month, cur_day) = days_to_civil(days_since_epoch);

    let year = params.year.unwrap_or(cur_year).clamp(2015, cur_year + 1);
    let month = params.month.unwrap_or(cur_month).clamp(1, 12);

    let month_prefix = format!("{year:04}-{month:02}");
    let dim = days_in_month(year, month);
    let first_wd = day_of_week(year, month, 1);

    let vods = {
        let guard = state.vods.read().await;
        Arc::clone(&*guard)
    };

    let mut day_map: std::collections::HashMap<u32, Vec<&crate::vods::Vod>> =
        std::collections::HashMap::new();
    for vod in vods.iter() {
        if vod.created_at.starts_with(&month_prefix)
            && vod.created_at.len() >= 10
            && let Ok(d) = vod.created_at[8..10].parse::<u32>()
            && d >= 1
            && d <= dim
        {
            day_map.entry(d).or_default().push(vod);
        }
    }

    let mut max_minutes: i64 = 1;
    let mut day_minutes: std::collections::HashMap<u32, i64> = std::collections::HashMap::new();
    for (&d, dvods) in &day_map {
        let total: i64 = dvods
            .iter()
            .map(|v| parse_duration_minutes(v.duration.as_deref().unwrap_or("")))
            .sum();
        day_minutes.insert(d, total);
        if total > max_minutes {
            max_minutes = total;
        }
    }

    let days: Vec<Option<CalendarDay>> = (1..=dim)
        .map(|d| {
            day_map.get(&d).map(|dvods| {
                let mut games = Vec::new();
                let mut seen = std::collections::HashSet::new();
                for vod in dvods {
                    if let Some(ref chapters) = vod.chapters {
                        for ch in chapters {
                            if let Some(ref name) = ch.name
                                && !name.is_empty()
                                && seen.insert(name.to_lowercase())
                                && games.len() < 4
                            {
                                games.push(CalendarGame {
                                    name: name.clone(),
                                    image: ch
                                        .image
                                        .as_deref()
                                        .map(crate::vods::upscale_chapter_image),
                                });
                            }
                        }
                    }
                }
                let total = *day_minutes.get(&d).unwrap_or(&0);
                let duration_display = if total >= 60 {
                    format!("{}h {}m", total / 60, total % 60)
                } else {
                    format!("{total}m")
                };
                CalendarDay {
                    date: d,
                    games,
                    duration_display,
                    glow: (total as f64 / max_minutes as f64).clamp(0.15, 1.0),
                    date_str: format!("{year:04}-{month:02}-{d:02}"),
                }
            })
        })
        .collect();

    let (prev_year, prev_month) = prev_month(year, month);
    let (next_year, next_month) = next_month(year, month);
    let has_next = year < cur_year || (year == cur_year && month < cur_month);

    let tmpl = CalendarPageTemplate {
        year,
        month_name: month_name(month),
        first_weekday: first_wd,
        days,
        prev_year,
        prev_month,
        next_year,
        next_month,
        has_next,
        today_day: if year == cur_year && month == cur_month {
            cur_day as usize
        } else {
            usize::MAX
        },
        active_section: Section::Calendar,
    };
    render_template(&tmpl)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calendar_helpers() {
        assert_eq!(days_in_month(2025, 1), 31);
        assert_eq!(days_in_month(2025, 2), 28);
        assert_eq!(days_in_month(2024, 2), 29);
        assert_eq!(days_in_month(2025, 4), 30);

        assert_eq!(day_of_week(2025, 1, 1), 3);
        assert_eq!(day_of_week(2025, 3, 1), 6);
        assert_eq!(day_of_week(2024, 1, 1), 1);

        assert_eq!(month_name(1), "January");
        assert_eq!(month_name(12), "December");
    }
}
