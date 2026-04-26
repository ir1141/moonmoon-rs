use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Vod {
    pub id: String,
    pub title: Option<String>,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    pub duration: Option<String>,
    pub thumbnail_url: Option<String>,
    pub chapters: Option<Vec<Chapter>>,
    pub youtube: Option<Vec<YoutubeVideo>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Chapter {
    pub name: Option<String>,
    pub image: Option<String>,
    pub start: Option<f64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct YoutubeVideo {
    pub id: String,
}

#[derive(Debug, Clone)]
pub struct Game {
    pub name: String,
    pub image: Option<String>,
    pub vod_count: usize,
}

#[must_use]
#[derive(Debug, Clone)]
pub enum RefreshOutcome {
    Busy,
    Unchanged(usize),
    Refreshed(usize),
    Error(String),
}

#[derive(Deserialize)]
struct ApiResponse {
    pub data: Vec<Vod>,
    pub total: usize,
}

pub fn upscale_chapter_image(url: &str) -> String {
    url.replace("40x53", "285x380")
}

const API: &str = "https://archive.overpowered.tv/moonmoon/vods";
const PAGE_SIZE: usize = 50;
const MAX_429_RETRIES: usize = 6;
const INITIAL_429_BACKOFF_MS: u64 = 250;

fn page_url(skip: usize) -> String {
    format!(
        "{API}?$limit={PAGE_SIZE}&$skip={skip}&$sort[createdAt]=-1\
         &$select[]=id&$select[]=title&$select[]=createdAt\
         &$select[]=duration&$select[]=thumbnail_url\
         &$select[]=chapters&$select[]=youtube"
    )
}

fn pages(total: usize) -> usize {
    total.div_ceil(PAGE_SIZE)
}

fn backoff_delay(attempt: usize) -> Duration {
    Duration::from_millis(INITIAL_429_BACKOFF_MS.saturating_mul(1_u64 << attempt.min(6)))
}

// Spread retries randomly across [0.5x, 1.5x] of `base` so concurrent
// in-flight requests that all hit 429 don't wake up at the same instant
// and re-collide on the upstream.
fn jittered(base: Duration) -> Duration {
    use rand::Rng;
    let ms = u64::try_from(base.as_millis()).unwrap_or(u64::MAX);
    let half = ms / 2;
    // adding 0..=ms to half yields 0.5x..1.5x of base
    let extra = rand::rng().random_range(0..=ms);
    Duration::from_millis(half.saturating_add(extra))
}

fn parse_retry_after(headers: &reqwest::header::HeaderMap) -> Option<Duration> {
    headers
        .get(reqwest::header::RETRY_AFTER)?
        .to_str()
        .ok()?
        .parse::<u64>()
        .ok()
        .map(Duration::from_secs)
}

fn should_retry(status: reqwest::StatusCode, attempt: usize) -> bool {
    status == reqwest::StatusCode::TOO_MANY_REQUESTS && attempt < MAX_429_RETRIES
}

async fn fetch_api_response(
    client: &reqwest::Client,
    url: &str,
) -> Result<ApiResponse, reqwest::Error> {
    let mut attempt = 0;
    loop {
        let resp = client.get(url).send().await?;
        if should_retry(resp.status(), attempt) {
            let base = parse_retry_after(resp.headers()).unwrap_or_else(|| backoff_delay(attempt));
            let delay = jittered(base);
            tracing::warn!(
                "rate limited fetching {url}; retrying in {}ms (attempt {}/{})",
                delay.as_millis(),
                attempt + 1,
                MAX_429_RETRIES
            );
            tokio::time::sleep(delay).await;
            attempt += 1;
            continue;
        }
        return resp.error_for_status()?.json().await;
    }
}

pub async fn fetch_vod_count(client: &reqwest::Client) -> Result<usize, reqwest::Error> {
    let resp = fetch_api_response(client, &format!("{API}?$limit=1&$skip=0")).await?;
    Ok(resp.total)
}

pub async fn fetch_all_vods(client: &reqwest::Client) -> Result<Vec<Vod>, reqwest::Error> {
    let first = fetch_api_response(client, &page_url(0)).await?;
    let total = first.total;
    tracing::info!("fetching {total} vods...");

    let total_pages = pages(total);
    if total_pages == 0 {
        return Ok(Vec::new());
    }

    let mut result: Vec<Vod> = Vec::with_capacity(total);
    result.extend(first.data);

    for page_idx in 1..total_pages {
        let resp = fetch_api_response(client, &page_url(page_idx * PAGE_SIZE)).await?;
        result.extend(resp.data);
        tracing::debug!("page {} of {} done", page_idx + 1, total_pages);
    }

    tracing::info!("{} / {} vods fetched", result.len(), total);
    Ok(result)
}

pub async fn load_vods(client: &reqwest::Client) -> Vec<Vod> {
    match fetch_all_vods(client).await {
        Ok(vods) => vods,
        Err(e) => {
            tracing::error!("failed to fetch vods: {e}");
            tracing::error!("starting with 0 vods — site will be empty until next refresh");
            Vec::new()
        }
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
                    });
                    entry.vod_count += 1;
                    if entry.image.is_none() {
                        entry.image = ch.image.as_deref().map(upscale_chapter_image);
                    }
                }
            }
        }
    }
    let mut games: Vec<Game> = games.into_values().collect();
    games.sort_by(|a, b| b.vod_count.cmp(&a.vod_count));
    games
}

pub async fn refresh_in_place(state: &crate::SharedState) -> RefreshOutcome {
    let _refresh_guard = match state.refresh_lock.try_lock() {
        Ok(g) => g,
        Err(_) => {
            tracing::info!("refresh: already in progress, skipping");
            return RefreshOutcome::Busy;
        }
    };

    let cached_count = state.vods.read().await.len();

    let remote_count = match fetch_vod_count(&state.http_client).await {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("refresh: failed to check vod count: {e}");
            return RefreshOutcome::Error(format!("failed to check vod count: {e}"));
        }
    };

    if remote_count == cached_count {
        tracing::info!("refresh: vod count unchanged ({cached_count})");
        return RefreshOutcome::Unchanged(cached_count);
    }

    tracing::info!("refresh: vod count changed ({cached_count} -> {remote_count}), fetching...");
    let new_vods = match fetch_all_vods(&state.http_client).await {
        Ok(v) => v,
        Err(e) => {
            tracing::error!("refresh: failed to fetch vods: {e}");
            return RefreshOutcome::Error(format!("failed to fetch vods: {e}"));
        }
    };

    let new_vods = std::sync::Arc::new(new_vods);
    let new_games = std::sync::Arc::new(build_games(&new_vods));
    let count = new_vods.len();

    {
        let mut vods_w = state.vods.write().await;
        let mut games_w = state.games.write().await;
        *vods_w = new_vods;
        *games_w = new_games;
    }

    tracing::info!("refresh: complete ({count} vods)");
    RefreshOutcome::Refreshed(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vod_deserialize() {
        let json = r#"{"id":"abc123","title":"Test Stream","createdAt":"2025-01-15T00:00:00Z","duration":"3h 20m","thumbnail_url":"https://example.com/thumb.jpg","chapters":[{"name":"Elden Ring","image":"https://example.com/40x53.jpg"}]}"#;
        let vod: Vod = serde_json::from_str(json).unwrap();
        assert_eq!(vod.id, "abc123");
        assert_eq!(vod.chapters.unwrap()[0].name.as_deref(), Some("Elden Ring"));
    }

    #[test]
    fn test_build_games_deduplicates() {
        let vods = vec![Vod {
            id: "1".into(),
            title: Some("Stream 1".into()),
            created_at: "2025-01-01T00:00:00Z".into(),
            duration: Some("2h".into()),
            thumbnail_url: None,
            chapters: Some(vec![
                Chapter {
                    name: Some("Game A".into()),
                    image: None,
                    start: None,
                },
                Chapter {
                    name: Some("Game A".into()),
                    image: None,
                    start: None,
                },
                Chapter {
                    name: Some("Game B".into()),
                    image: None,
                    start: None,
                },
            ]),
            youtube: None,
        }];
        let games = build_games(&vods);
        assert_eq!(games.len(), 2);
    }

    #[test]
    fn test_build_games_case_insensitive() {
        let vods = vec![
            Vod {
                id: "1".into(),
                title: Some("Stream 1".into()),
                created_at: "2025-01-01T00:00:00Z".into(),
                duration: Some("2h".into()),
                thumbnail_url: None,
                chapters: Some(vec![Chapter {
                    name: Some("Elden Ring".into()),
                    image: None,
                    start: None,
                }]),
                youtube: None,
            },
            Vod {
                id: "2".into(),
                title: Some("Stream 2".into()),
                created_at: "2025-01-02T00:00:00Z".into(),
                duration: Some("3h".into()),
                thumbnail_url: None,
                chapters: Some(vec![Chapter {
                    name: Some("ELDEN RING".into()),
                    image: None,
                    start: None,
                }]),
                youtube: None,
            },
        ];
        let games = build_games(&vods);
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].name, "Elden Ring"); // keeps first-seen casing
        assert_eq!(games[0].vod_count, 2);
    }

    #[test]
    fn test_backoff_delay_grows() {
        assert_eq!(backoff_delay(0), Duration::from_millis(250));
        assert_eq!(backoff_delay(1), Duration::from_millis(500));
        assert_eq!(backoff_delay(2), Duration::from_millis(1000));
    }

    #[test]
    fn test_jittered_stays_in_band() {
        let base = Duration::from_millis(1000);
        for _ in 0..100 {
            let d = jittered(base);
            assert!(
                d >= Duration::from_millis(500) && d <= Duration::from_millis(1500),
                "jittered out of [0.5x, 1.5x]: {d:?}"
            );
        }
    }

    #[test]
    fn test_parse_retry_after_seconds() {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(reqwest::header::RETRY_AFTER, "3".parse().unwrap());
        assert_eq!(parse_retry_after(&headers), Some(Duration::from_secs(3)));
    }

    #[test]
    fn test_parse_retry_after_missing() {
        let headers = reqwest::header::HeaderMap::new();
        assert_eq!(parse_retry_after(&headers), None);
    }

    #[test]
    fn test_parse_retry_after_non_numeric() {
        // HTTP-date format is valid per spec but we don't handle it; should
        // gracefully fall through to our exponential backoff instead of panicking.
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::RETRY_AFTER,
            "Wed, 21 Oct 2026 07:28:00 GMT".parse().unwrap(),
        );
        assert_eq!(parse_retry_after(&headers), None);
    }

    #[test]
    fn test_page_url_includes_required_params() {
        let url = page_url(100);
        assert!(url.starts_with("https://archive.overpowered.tv/moonmoon/vods?"));
        assert!(url.contains("$limit=50"), "missing $limit=50: {url}");
        assert!(url.contains("$skip=100"), "missing $skip=100: {url}");
        assert!(url.contains("$sort[createdAt]=-1"), "missing $sort: {url}");
        for field in [
            "id",
            "title",
            "createdAt",
            "duration",
            "thumbnail_url",
            "chapters",
            "youtube",
        ] {
            assert!(
                url.contains(&format!("$select[]={field}")),
                "missing $select[]={field} in: {url}"
            );
        }
    }

    #[test]
    fn test_pages_handles_edges() {
        assert_eq!(pages(0), 0);
        assert_eq!(pages(1), 1);
        assert_eq!(pages(50), 1);
        assert_eq!(pages(51), 2);
        assert_eq!(pages(100), 2);
        assert_eq!(pages(1419), 29);
    }
}
