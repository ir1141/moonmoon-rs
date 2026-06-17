use super::{Chapter, Vod, upscale_chapter_image};

#[derive(Debug, Clone)]
pub struct Game {
    pub name: String,
    pub image: Option<String>,
    pub vod_count: usize,
    pub dominant_stream_count: usize,
    pub first_streamed: Option<String>,
    pub last_streamed: Option<String>,
}

impl Game {
    pub fn first_streamed_label(&self) -> Option<String> {
        self.first_streamed.as_deref().map(format_stream_date)
    }

    pub fn last_streamed_label(&self) -> Option<String> {
        self.last_streamed.as_deref().map(format_stream_date)
    }
}

pub fn build_games(vods: &[Vod]) -> Vec<Game> {
    use std::collections::HashMap;
    let mut games: HashMap<String, Game> = HashMap::new();

    for vod in vods {
        // Deduplicate chapters within this VOD (cross-VOD merges happen in the outer HashMap)
        let mut seen = std::collections::HashSet::new();
        if let Some(chapters) = &vod.chapters {
            for ch in chapters {
                if let Some(name) = &ch.name {
                    let key = name.to_lowercase();
                    if name.is_empty() || !seen.insert(key.clone()) {
                        continue;
                    }
                    let entry = games.entry(key).or_insert_with(|| Game {
                        name: name.clone(),
                        image: None,
                        vod_count: 0,
                        dominant_stream_count: 0,
                        first_streamed: None,
                        last_streamed: None,
                    });
                    entry.vod_count += 1;
                    if entry.image.is_none() {
                        entry.image = ch.image.as_deref().map(upscale_chapter_image);
                    }
                }
            }
        }
        if let Some(dominant) = dominant_game(vod) {
            let key = dominant.name.to_lowercase();
            let entry = games.entry(key).or_insert_with(|| Game {
                name: dominant.name.clone(),
                image: dominant.image.clone(),
                vod_count: 0,
                dominant_stream_count: 0,
                first_streamed: None,
                last_streamed: None,
            });
            if entry.image.is_none() {
                entry.image = dominant.image;
            }
            update_dominant_stream_stats(entry, stream_time_for_vod(vod));
        }
    }
    let mut games: Vec<Game> = games.into_values().collect();
    games.sort_by_key(|g| std::cmp::Reverse(g.vod_count));
    games
}

pub fn build_dominant_games<'a, I>(vods: I) -> Vec<Game>
where
    I: IntoIterator<Item = &'a Vod>,
{
    use std::collections::HashMap;
    let mut games: HashMap<String, Game> = HashMap::new();

    for vod in vods {
        let Some(dominant) = dominant_game(vod) else {
            continue;
        };
        let key = dominant.name.to_lowercase();
        let entry = games.entry(key).or_insert_with(|| Game {
            name: dominant.name.clone(),
            image: dominant.image.clone(),
            vod_count: 0,
            dominant_stream_count: 0,
            first_streamed: None,
            last_streamed: None,
        });
        entry.vod_count += 1;
        if entry.image.is_none() {
            entry.image = dominant.image;
        }
        update_dominant_stream_stats(entry, stream_time_for_vod(vod));
    }

    let mut games: Vec<Game> = games.into_values().collect();
    games.sort_by_key(|g| std::cmp::Reverse(g.vod_count));
    games
}

pub(crate) fn chapter_color_idx(game_name: &str) -> u8 {
    let mut h: u32 = 0;
    for b in game_name.bytes() {
        h = h.wrapping_mul(31).wrapping_add(u32::from(b));
    }
    (h % 8) as u8
}

struct DominantGame {
    name: String,
    image: Option<String>,
}

fn dominant_game(vod: &Vod) -> Option<DominantGame> {
    use std::collections::HashMap;

    let chapters = vod.chapters.as_ref()?;
    let total_duration = vod
        .duration
        .as_ref()
        .map_or(0, |duration| duration.seconds());
    let named: Vec<(usize, &Chapter)> = chapters
        .iter()
        .enumerate()
        .filter(|(_, chapter)| chapter.name.as_deref().is_some_and(|name| !name.is_empty()))
        .collect();
    if named.is_empty() {
        return None;
    }

    let mut totals: HashMap<String, (String, i64, Option<String>, usize)> = HashMap::new();
    for (position, &(chapter_idx, chapter)) in named.iter().enumerate() {
        let name = chapter.name.as_deref().unwrap_or_default();
        let key = name.to_lowercase();
        let duration = chapter_duration_seconds(&named, position, chapter_idx, total_duration);
        let image = chapter.image.as_deref().map(upscale_chapter_image);
        let entry = totals
            .entry(key)
            .or_insert_with(|| (name.to_string(), 0, image.clone(), chapter_idx));
        entry.1 += duration;
        if entry.2.is_none() {
            entry.2 = image;
        }
        entry.3 = entry.3.min(chapter_idx);
    }

    totals
        .into_values()
        .max_by(|a, b| a.1.cmp(&b.1).then_with(|| b.3.cmp(&a.3)))
        .map(|(name, _, image, _)| DominantGame { name, image })
}

fn chapter_duration_seconds(
    named: &[(usize, &Chapter)],
    position: usize,
    chapter_idx: usize,
    total_duration: i64,
) -> i64 {
    let chapter = named[position].1;
    let start = chapter.start.map(|s| s as i64).unwrap_or(0).max(0);
    let explicit_end = chapter
        .end
        .map(|end| end as i64)
        .or_else(|| chapter.duration.map(|duration| start + duration as i64));
    let inferred_end = named
        .get(position + 1)
        .and_then(|(_, next)| next.start.map(|start| start as i64))
        .unwrap_or(total_duration);
    let end = explicit_end
        .unwrap_or(inferred_end)
        .clamp(0, total_duration.max(0));
    let duration = end.saturating_sub(start);
    if duration > 0 {
        return duration;
    }
    if named.len() == 1 || chapter_idx == named.last().map(|(idx, _)| *idx).unwrap_or(chapter_idx) {
        total_duration.max(0)
    } else {
        0
    }
}

fn update_dominant_stream_stats(game: &mut Game, stream_time: &str) {
    game.dominant_stream_count += 1;
    if game
        .first_streamed
        .as_deref()
        .is_none_or(|first| stream_time < first)
    {
        game.first_streamed = Some(stream_time.to_string());
    }
    if game
        .last_streamed
        .as_deref()
        .is_none_or(|last| stream_time > last)
    {
        game.last_streamed = Some(stream_time.to_string());
    }
}

fn stream_time_for_vod(vod: &Vod) -> &str {
    vod.started_at.as_deref().unwrap_or(&vod.created_at)
}

pub(crate) fn format_stream_date(timestamp: &str) -> String {
    let Some(date_part) = timestamp.get(..10) else {
        return timestamp.to_string();
    };
    let parts: Vec<&str> = date_part.split('-').collect();
    if parts.len() != 3 {
        return date_part.to_string();
    }
    let day = parts[2].trim_start_matches('0');
    format!("{} {day}, {}", month_abbr(parts[1]), parts[0])
}

pub(crate) fn month_abbr_num(month: u32) -> &'static str {
    match month {
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

pub(crate) fn month_abbr(month_part: &str) -> &str {
    match month_part.parse::<u32>() {
        Ok(m @ 1..=12) => month_abbr_num(m),
        _ => month_part,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_month_abbr_num() {
        assert_eq!(month_abbr_num(1), "Jan");
        assert_eq!(month_abbr_num(12), "Dec");
        assert_eq!(month_abbr_num(13), "???");
    }

    #[test]
    fn test_build_games_deduplicates() {
        let vods = vec![Vod {
            id: "1".into(),
            platform: None,
            platform_vod_id: None,
            platform_stream_id: None,
            title: Some("Stream 1".into()),
            created_at: "2025-01-01T00:00:00Z".into(),
            started_at: None,
            updated_at: None,
            duration: Some("2h".into()),
            thumbnail_url: None,
            chapters: Some(vec![
                Chapter {
                    name: Some("Game A".into()),
                    image: None,
                    start: None,
                    duration: None,
                    end: None,
                },
                Chapter {
                    name: Some("Game A".into()),
                    image: None,
                    start: None,
                    duration: None,
                    end: None,
                },
                Chapter {
                    name: Some("Game B".into()),
                    image: None,
                    start: None,
                    duration: None,
                    end: None,
                },
            ]),
            youtube: None,
            is_live: false,
        }];
        let games = build_games(&vods);
        assert_eq!(games.len(), 2);
    }

    #[test]
    fn test_build_games_case_insensitive() {
        let vods = vec![
            Vod {
                id: "1".into(),
                platform: None,
                platform_vod_id: None,
                platform_stream_id: None,
                title: Some("Stream 1".into()),
                created_at: "2025-01-01T00:00:00Z".into(),
                started_at: None,
                updated_at: None,
                duration: Some("2h".into()),
                thumbnail_url: None,
                chapters: Some(vec![Chapter {
                    name: Some("Elden Ring".into()),
                    image: None,
                    start: None,
                    duration: None,
                    end: None,
                }]),
                youtube: None,
                is_live: false,
            },
            Vod {
                id: "2".into(),
                platform: None,
                platform_vod_id: None,
                platform_stream_id: None,
                title: Some("Stream 2".into()),
                created_at: "2025-01-02T00:00:00Z".into(),
                started_at: None,
                updated_at: None,
                duration: Some("3h".into()),
                thumbnail_url: None,
                chapters: Some(vec![Chapter {
                    name: Some("ELDEN RING".into()),
                    image: None,
                    start: None,
                    duration: None,
                    end: None,
                }]),
                youtube: None,
                is_live: false,
            },
        ];
        let games = build_games(&vods);
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].name, "Elden Ring"); // keeps first-seen casing
        assert_eq!(games[0].vod_count, 2);
    }
}
