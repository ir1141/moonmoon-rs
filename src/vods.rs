use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::{Duration, SystemTime};

const CACHE_PATH: &str = "data/vods.json";
const CACHE_MAX_AGE_SECS: u64 = 86400;

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
const MAX_429_RETRIES: usize = 3;
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
            let delay = backoff_delay(attempt);
            tracing::warn!(
                "rate limited fetching {url}; retrying in {}ms",
                delay.as_millis()
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
    let first = fetch_api_response(client, &format!("{API}?$limit=1&$skip=0")).await?;

    let total = first.total;
    tracing::info!("fetching {total} vods...");
    let mut all_vods = Vec::with_capacity(total);
    let mut skip = 0;

    while skip < total {
        let resp = fetch_api_response(
            client,
            &format!("{API}?$limit={PAGE_SIZE}&$skip={skip}&$sort[createdAt]=-1"),
        )
        .await?;
        let got = resp.data.len();
        if got == 0 {
            break;
        }
        all_vods.extend(resp.data);
        skip += got;
        tracing::info!("{} / {} vods", all_vods.len().min(total), total);
        if got < PAGE_SIZE {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }
    Ok(all_vods)
}

fn read_cache() -> Option<Vec<Vod>> {
    let path = Path::new(CACHE_PATH);
    if !path.exists() {
        return None;
    }
    let metadata = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(e) => {
            tracing::warn!("cache metadata error: {e}");
            return None;
        }
    };
    let modified = match metadata.modified() {
        Ok(m) => m,
        Err(e) => {
            tracing::warn!("cache modified time error: {e}");
            return None;
        }
    };
    let age = match SystemTime::now().duration_since(modified) {
        Ok(a) => a,
        Err(e) => {
            tracing::warn!("cache time error (clock skew?): {e}");
            return None;
        }
    };
    if age.as_secs() > CACHE_MAX_AGE_SECS {
        tracing::info!("cache is {}s old, refreshing", age.as_secs());
        return None;
    }
    tracing::info!("loading from cache ({}s old)", age.as_secs());
    let data = match std::fs::read_to_string(path) {
        Ok(d) => d,
        Err(e) => {
            tracing::warn!("cache read error: {e}");
            return None;
        }
    };
    match serde_json::from_str(&data) {
        Ok(vods) => Some(vods),
        Err(e) => {
            tracing::warn!("cache parse error: {e}");
            None
        }
    }
}

pub fn write_cache(vods: &[Vod]) {
    if let Some(parent) = Path::new(CACHE_PATH).parent()
        && let Err(e) = std::fs::create_dir_all(parent)
    {
        tracing::warn!("failed to create cache directory {:?}: {e}", parent);
    }
    match serde_json::to_string(vods) {
        Ok(json) => {
            if let Err(e) = std::fs::write(CACHE_PATH, json) {
                tracing::warn!("failed to write cache: {e}");
            } else {
                tracing::info!("cache written ({} vods)", vods.len());
            }
        }
        Err(e) => tracing::warn!("failed to serialize cache: {e}"),
    }
}

pub async fn load_vods(client: &reqwest::Client) -> Vec<Vod> {
    if let Some(vods) = read_cache() {
        tracing::info!("loaded {} vods from cache", vods.len());
        return vods;
    }
    match fetch_all_vods(client).await {
        Ok(vods) => {
            write_cache(&vods);
            vods
        }
        Err(e) => {
            tracing::error!("failed to fetch vods: {e}");
            tracing::error!("starting with 0 vods — site will be empty");
            vec![]
        }
    }
}

pub fn build_games(vods: &[Vod]) -> Vec<Game> {
    use std::collections::HashMap;
    let mut games: HashMap<String, Game> = HashMap::new();

    for vod in vods {
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
    fn test_page_url_includes_required_params() {
        let url = page_url(100);
        assert!(url.starts_with("https://archive.overpowered.tv/moonmoon/vods?"));
        assert!(url.contains("$limit=50"), "missing $limit=50: {url}");
        assert!(url.contains("$skip=100"), "missing $skip=100: {url}");
        assert!(url.contains("$sort[createdAt]=-1"), "missing $sort: {url}");
        for field in [
            "id", "title", "createdAt", "duration", "thumbnail_url", "chapters", "youtube",
        ] {
            assert!(
                url.contains(&format!("$select[]={field}")),
                "missing $select[]={field} in: {url}"
            );
        }
    }

    #[test]
    fn test_page_url_skip_zero() {
        let url = page_url(0);
        assert!(url.contains("$skip=0"));
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
