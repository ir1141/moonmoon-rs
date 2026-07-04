use super::{Vod, upscale_chapter_image};

/// A named, clamped chapter span within a single `Vod`.
///
/// Produced by [`Vod::chapter_spans`]: `start`/`end` are clamped to
/// `[0, total_duration]` and spans where `end <= start` are dropped, so every
/// returned span has a strictly positive length. This is the one home for the
/// chapter-range math; `dominant_game` and the watch-page chapter bar both
/// derive from it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChapterSpan<'a> {
    pub name: &'a str,
    pub start: i64,
    pub end: i64,
}

impl Vod {
    /// Earliest moment this stream is known to have started: `started_at` when
    /// the upstream provides it, falling back to `created_at`.
    pub fn stream_time(&self) -> &str {
        self.started_at.as_deref().unwrap_or(&self.created_at)
    }

    /// A VOD is playable when it is not live and has at least one canonical
    /// YouTube upload to send the viewer to.
    pub fn is_playable(&self) -> bool {
        !self.is_live && !super::canonical_youtube_uploads(self).is_empty()
    }

    /// Whether any chapter of this VOD is tagged with `name` (case-insensitive).
    pub fn has_game(&self, game_name: &str) -> bool {
        self.chapters.as_ref().is_some_and(|chapters| {
            chapters.iter().any(|ch| {
                ch.name
                    .as_deref()
                    .is_some_and(|n| n.eq_ignore_ascii_case(game_name))
            })
        })
    }

    /// Start offset (seconds) of the first chapter matching `name`, reading
    /// chapters directly. Not derived from `chapter_spans` — that filters
    /// degenerate spans, and this must still report a raw start for them.
    pub fn chapter_start_for(&self, game_name: &str) -> Option<i64> {
        let chapters = self.chapters.as_ref()?;
        chapters.iter().find_map(|ch| {
            let name = ch.name.as_deref()?;
            name.eq_ignore_ascii_case(game_name)
                .then(|| ch.start.map(|s| s as i64))?
        })
    }

    /// Clamped, positive-length spans over this VOD's named chapters.
    ///
    /// The total duration is read from `self.duration`; a VOD with no duration
    /// yields no spans. For each named chapter the end is the explicit `end`,
    /// else `start + duration`, else the next chapter's start, else the total.
    /// Spans whose final `end <= start` are dropped.
    pub fn chapter_spans(&self) -> Vec<ChapterSpan<'_>> {
        let Some(chapters) = self.chapters.as_ref() else {
            return Vec::new();
        };
        if chapters.is_empty() {
            return Vec::new();
        }
        let total_duration = self
            .duration
            .as_ref()
            .map_or(0, |duration| duration.seconds());
        if total_duration <= 0 {
            return Vec::new();
        }

        // (name, start, explicit_end) for chapters with a non-empty name.
        let mut named: Vec<(&str, i64, Option<i64>)> = Vec::new();
        for ch in chapters {
            if let Some(name) = ch.name.as_deref().filter(|n| !n.is_empty()) {
                let start = ch.start.map(|s| s as i64).unwrap_or(0);
                let explicit_end = ch
                    .end
                    .map(|end| end as i64)
                    .or_else(|| ch.duration.map(|duration| start + duration as i64));
                named.push((name, start, explicit_end));
            }
        }
        if named.is_empty() {
            return Vec::new();
        }

        let mut out = Vec::with_capacity(named.len());
        for (k, &(name, start, explicit_end)) in named.iter().enumerate() {
            let inferred_end = named.get(k + 1).map(|n| n.1).unwrap_or(total_duration);
            let start = start.clamp(0, total_duration);
            let end = explicit_end
                .unwrap_or(inferred_end)
                .clamp(0, total_duration);
            if end <= start {
                continue;
            }
            out.push(ChapterSpan { name, start, end });
        }
        out
    }
}

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
            update_dominant_stream_stats(entry, vod.stream_time());
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
        update_dominant_stream_stats(entry, vod.stream_time());
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

    let spans = vod.chapter_spans();
    if spans.is_empty() {
        return None;
    }

    // First upscaled image per game (case-insensitive), from the raw chapters.
    let mut first_image: HashMap<String, Option<String>> = HashMap::new();
    if let Some(chapters) = vod.chapters.as_ref() {
        for ch in chapters {
            if let Some(name) = ch.name.as_deref().filter(|n| !n.is_empty()) {
                first_image
                    .entry(name.to_lowercase())
                    .or_insert_with(|| ch.image.as_deref().map(upscale_chapter_image));
            }
        }
    }

    // Aggregate durations per game. The span order (earliest-first) doubles as
    // the tiebreak the original used "lowest chapter index" for: when two games
    // tie on total duration, the one that appears earlier wins.
    let mut totals: HashMap<String, (String, i64, Option<String>, usize)> = HashMap::new();
    for (position, span) in spans.iter().enumerate() {
        let key = span.name.to_lowercase();
        let image = first_image.get(&key).cloned().flatten();
        let entry = totals
            .entry(key)
            .or_insert_with(|| (span.name.to_string(), 0, image.clone(), position));
        entry.1 += span.end - span.start;
        if entry.2.is_none() {
            entry.2 = image;
        }
        entry.3 = entry.3.min(position);
    }

    totals
        .into_values()
        .max_by(|a, b| a.1.cmp(&b.1).then_with(|| b.3.cmp(&a.3)))
        .map(|(name, _, image, _)| DominantGame { name, image })
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
    use crate::vods::{Chapter, VodDuration};

    fn vod_with(duration_secs: i64, chapters: Vec<Chapter>) -> Vod {
        Vod {
            id: "v".into(),
            platform: None,
            platform_vod_id: None,
            platform_stream_id: None,
            title: None,
            created_at: "2025-01-01T00:00:00Z".into(),
            started_at: None,
            updated_at: None,
            duration: Some(VodDuration::from_seconds(duration_secs)),
            thumbnail_url: None,
            chapters: Some(chapters),
            youtube: None,
            is_live: false,
        }
    }

    fn ch(name: &str, start: Option<f64>, duration: Option<f64>, end: Option<f64>) -> Chapter {
        Chapter {
            name: Some(name.into()),
            image: None,
            start,
            duration,
            end,
        }
    }

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

    #[test]
    fn stream_time_prefers_started_at() {
        let mut vod = vod_with(3600, vec![ch("Game", Some(0.0), None, None)]);
        assert_eq!(vod.stream_time(), "2025-01-01T00:00:00Z");
        vod.started_at = Some("2025-01-02T12:00:00Z".into());
        assert_eq!(vod.stream_time(), "2025-01-02T12:00:00Z");
    }

    #[test]
    fn chapter_spans_multi_chapter_inferred_end() {
        // start=0/end=100, start=100 (end inferred from next=300), start=300 (end=total=1000)
        let vod = vod_with(
            1000,
            vec![
                ch("A", Some(0.0), None, Some(100.0)),
                ch("B", Some(100.0), None, None),
                ch("C", Some(300.0), None, None),
            ],
        );
        let spans = vod.chapter_spans();
        let names: Vec<&str> = spans.iter().map(|s| s.name).collect();
        assert_eq!(names, vec!["A", "B", "C"]);
        assert_eq!(
            spans[0],
            ChapterSpan {
                name: "A",
                start: 0,
                end: 100
            }
        );
        assert_eq!(
            spans[1],
            ChapterSpan {
                name: "B",
                start: 100,
                end: 300
            }
        );
        assert_eq!(
            spans[2],
            ChapterSpan {
                name: "C",
                start: 300,
                end: 1000
            }
        );
    }

    #[test]
    fn chapter_spans_drops_zero_and_negative_length() {
        // A span whose clamped end <= start is dropped. Zero-length (end == start)
        // and a chapter ending before it starts (clamped so end < start would
        // need start beyond total) are both filtered.
        let vod = vod_with(
            1000,
            vec![
                ch("Zero", Some(100.0), None, Some(100.0)), // end == start → dropped
                ch("Ok", Some(0.0), None, Some(100.0)),     // 0..100 → kept
                ch("Beyond", Some(1000.0), None, Some(1000.0)), // start==end==total → dropped
            ],
        );
        let spans = vod.chapter_spans();
        let names: Vec<&str> = spans.iter().map(|s| s.name).collect();
        assert_eq!(names, vec!["Ok"]);
    }

    #[test]
    fn chapter_spans_empty_without_duration() {
        let mut vod = vod_with(3600, vec![ch("A", Some(0.0), None, None)]);
        assert_eq!(vod.chapter_spans().len(), 1);
        vod.duration = None;
        assert!(vod.chapter_spans().is_empty());
    }

    #[test]
    fn dominant_game_no_longer_credits_degenerate_last_chapter() {
        // Last chapter starts at/after total → its span is dropped by
        // chapter_spans, so it must not receive the full total_duration.
        // Before unification, dominant_game credited total_duration (1000) to
        // such a chapter; that fallback is gone.
        let vod = vod_with(
            1000,
            vec![
                ch("Real", Some(0.0), None, Some(500.0)), // 0..500 → 500s
                ch("Phantom", Some(1000.0), None, None),  // start==total → dropped
            ],
        );
        let dominant = dominant_game(&vod).expect("a dominant game");
        assert_eq!(dominant.name, "Real");
    }
}
