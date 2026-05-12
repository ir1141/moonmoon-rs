use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Vod {
    #[serde(deserialize_with = "deserialize_id_string")]
    pub id: String,
    #[serde(default, deserialize_with = "deserialize_optional_id_string")]
    pub platform_vod_id: Option<String>,
    pub title: Option<String>,
    pub created_at: String,
    #[serde(default)]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub duration: Option<VodDuration>,
    #[serde(default)]
    pub thumbnail_url: Option<String>,
    pub chapters: Option<Vec<Chapter>>,
    #[serde(rename = "vod_uploads", alias = "youtube", default)]
    pub youtube: Option<Vec<YoutubeVideo>>,
    #[serde(default)]
    pub is_live: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Chapter {
    pub name: Option<String>,
    pub image: Option<String>,
    pub start: Option<f64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct YoutubeVideo {
    // Each upload has both an integer `id` (DB row) and a string `upload_id`
    // (YouTube video ID); we want the latter. Aliasing `"id"` would match the
    // integer first and fail to deserialize as a String.
    #[serde(rename = "upload_id")]
    pub id: String,
    #[serde(default)]
    pub thumbnail_url: Option<String>,
}

fn deserialize_id_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum IdValue {
        Int(i64),
        Str(String),
    }
    Ok(match IdValue::deserialize(deserializer)? {
        IdValue::Int(n) => n.to_string(),
        IdValue::Str(s) => s,
    })
}

fn deserialize_optional_id_string<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum IdValue {
        Int(i64),
        Str(String),
    }
    let value: Option<IdValue> = Option::deserialize(deserializer)?;
    Ok(value.map(|v| match v {
        IdValue::Int(n) => n.to_string(),
        IdValue::Str(s) => s,
    }))
}

fn format_duration_hm(secs: i64) -> String {
    if secs <= 0 {
        return "0m".into();
    }
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    if h > 0 && m > 0 {
        format!("{h}h {m}m")
    } else if h > 0 {
        format!("{h}h")
    } else if m > 0 {
        format!("{m}m")
    } else {
        format!("{secs}s")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VodDuration {
    display: String,
    seconds: i64,
}

impl VodDuration {
    pub fn from_seconds(seconds: i64) -> Self {
        Self {
            display: format_duration_hm(seconds),
            seconds: seconds.max(0),
        }
    }

    fn from_display(display: String) -> Self {
        let seconds = parse_duration_display_seconds(&display);
        Self { display, seconds }
    }

    pub fn display(&self) -> &str {
        &self.display
    }

    pub fn seconds(&self) -> i64 {
        self.seconds
    }
}

impl std::ops::Deref for VodDuration {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.display
    }
}

impl From<&str> for VodDuration {
    fn from(value: &str) -> Self {
        Self::from_display(value.to_string())
    }
}

impl From<String> for VodDuration {
    fn from(value: String) -> Self {
        Self::from_display(value)
    }
}

impl Serialize for VodDuration {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.display)
    }
}

impl<'de> Deserialize<'de> for VodDuration {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum DurationValue {
            Int(i64),
            Str(String),
        }

        Ok(match DurationValue::deserialize(deserializer)? {
            DurationValue::Int(secs) => Self::from_seconds(secs),
            DurationValue::Str(s) => Self::from_display(s),
        })
    }
}

fn parse_duration_display_seconds(duration: &str) -> i64 {
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

    let mut total = 0;
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
struct ApiMeta {
    pub total: usize,
}

#[derive(Deserialize)]
struct ApiResponse {
    pub data: Vec<Vod>,
    pub meta: ApiMeta,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CatalogSnapshot {
    pub total: usize,
    pub latest_id: Option<String>,
    pub latest_updated_at: Option<String>,
}

impl CatalogSnapshot {
    #[cfg(test)]
    pub(crate) fn from_vods(vods: &[Vod]) -> Self {
        let latest = vods.first();
        Self {
            total: vods.len(),
            latest_id: latest.map(|vod| vod.id.clone()),
            latest_updated_at: latest.and_then(|vod| vod.updated_at.clone()),
        }
    }

    fn from_api_response(resp: &ApiResponse) -> Self {
        let latest = resp.data.first();
        Self {
            total: resp.meta.total,
            latest_id: latest.map(|vod| vod.id.clone()),
            latest_updated_at: latest.and_then(|vod| vod.updated_at.clone()),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct CatalogLoad {
    pub(crate) vods: Vec<Vod>,
    pub(crate) snapshot: CatalogSnapshot,
}

impl CatalogLoad {
    pub(crate) fn empty() -> Self {
        Self {
            vods: Vec::new(),
            snapshot: CatalogSnapshot {
                total: 0,
                latest_id: None,
                latest_updated_at: None,
            },
        }
    }

    fn from_raw_vods(snapshot: CatalogSnapshot, mut vods: Vec<Vod>) -> Self {
        filter_playable_vods(&mut vods);
        backfill_thumbnails(&mut vods);

        Self { vods, snapshot }
    }
}

pub fn upscale_chapter_image(url: &str) -> String {
    url.replace("{width}x{height}", "285x380")
        .replace("40x53", "285x380")
}

const API: &str = "https://archive.overpowered.tv/api/v1/moonmoon/vods";
const PAGE_SIZE: usize = 50;
const MAX_429_RETRIES: usize = 6;
const INITIAL_429_BACKOFF_MS: u64 = 250;

fn page_url(page_one_based: usize) -> String {
    format!("{API}?page={page_one_based}&limit={PAGE_SIZE}&sort=created_at&order=desc")
}

fn snapshot_url() -> String {
    format!("{API}?page=1&limit=1&sort=created_at&order=desc")
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

async fn fetch_catalog_snapshot(
    client: &reqwest::Client,
) -> Result<CatalogSnapshot, reqwest::Error> {
    let resp = fetch_api_response(client, &snapshot_url()).await?;
    let latest = resp.data.first();
    Ok(CatalogSnapshot {
        total: resp.meta.total,
        latest_id: latest.map(|vod| vod.id.clone()),
        latest_updated_at: latest.and_then(|vod| vod.updated_at.clone()),
    })
}

pub fn is_playable_vod(vod: &Vod) -> bool {
    !vod.is_live
        && vod
            .youtube
            .as_ref()
            .is_some_and(|uploads| !uploads.is_empty())
}

pub fn filter_playable_vods(vods: &mut Vec<Vod>) {
    vods.retain(is_playable_vod);
}

pub(crate) async fn fetch_catalog_load(
    client: &reqwest::Client,
) -> Result<CatalogLoad, reqwest::Error> {
    let first = fetch_api_response(client, &page_url(1)).await?;
    let total = first.meta.total;
    tracing::info!("fetching {total} vods...");

    let total_pages = pages(total);
    if total_pages == 0 {
        return Ok(CatalogLoad::from_raw_vods(
            CatalogSnapshot::from_api_response(&first),
            first.data,
        ));
    }

    let snapshot = CatalogSnapshot::from_api_response(&first);
    let mut vods = Vec::with_capacity(total);
    vods.extend(first.data);

    for page_idx in 2..=total_pages {
        let resp = fetch_api_response(client, &page_url(page_idx)).await?;
        vods.extend(resp.data);
        tracing::debug!("page {page_idx} of {total_pages} done");
    }

    let catalog = CatalogLoad::from_raw_vods(snapshot, vods);

    tracing::info!("{} / {} vods fetched", catalog.vods.len(), total);
    Ok(catalog)
}

/// Upstream doesn't expose `thumbnail_url` at the VOD level — only on each
/// `vod_uploads` entry. Lift it from the first upload so templates and
/// `VodDisplay` can keep reading `vod.thumbnail_url` directly.
fn backfill_thumbnails(vods: &mut [Vod]) {
    for vod in vods.iter_mut() {
        if vod.thumbnail_url.is_some() {
            continue;
        }
        if let Some(uploads) = vod.youtube.as_ref()
            && let Some(thumb) = uploads.iter().find_map(|u| u.thumbnail_url.clone())
        {
            vod.thumbnail_url = Some(thumb);
        }
    }
}

pub async fn load_catalog(client: &reqwest::Client) -> CatalogLoad {
    match fetch_catalog_load(client).await {
        Ok(catalog) => catalog,
        Err(e) => {
            tracing::error!("failed to fetch vods: {e}");
            tracing::error!("starting with 0 vods — site will be empty until next refresh");
            CatalogLoad::empty()
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
    games.sort_by_key(|g| std::cmp::Reverse(g.vod_count));
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

    let cached_snapshot = state.catalog_snapshot.read().await.clone();

    let remote_snapshot = match fetch_catalog_snapshot(&state.http_client).await {
        Ok(snapshot) => snapshot,
        Err(e) => {
            tracing::error!("refresh: failed to check catalog snapshot: {e}");
            return RefreshOutcome::Error(format!("failed to check catalog snapshot: {e}"));
        }
    };

    if remote_snapshot == cached_snapshot {
        tracing::info!("refresh: catalog unchanged ({cached_snapshot:?})");
        let count = state.vods.read().await.len();
        return RefreshOutcome::Unchanged(count);
    }

    tracing::info!(
        "refresh: catalog changed ({cached_snapshot:?} -> {remote_snapshot:?}), fetching..."
    );
    let catalog = match fetch_catalog_load(&state.http_client).await {
        Ok(catalog) => catalog,
        Err(e) => {
            tracing::error!("refresh: failed to fetch vods: {e}");
            return RefreshOutcome::Error(format!("failed to fetch vods: {e}"));
        }
    };

    let new_vods = std::sync::Arc::new(catalog.vods);
    let new_games = std::sync::Arc::new(build_games(&new_vods));
    let count = new_vods.len();

    {
        let mut vods_w = state.vods.write().await;
        let mut games_w = state.games.write().await;
        let mut snapshot_w = state.catalog_snapshot.write().await;
        *vods_w = new_vods;
        *games_w = new_games;
        *snapshot_w = catalog.snapshot;
    }

    tracing::info!("refresh: complete ({count} vods)");
    RefreshOutcome::Refreshed(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vod_deserialize_string_fields() {
        let json = r#"{"id":"abc123","platform_vod_id":"2237432794","title":"Test Stream","created_at":"2025-01-15T00:00:00Z","duration":"3h 20m","thumbnail_url":"https://example.com/thumb.jpg","chapters":[{"name":"Elden Ring","image":"https://example.com/40x53.jpg"}]}"#;
        let vod: Vod = serde_json::from_str(json).unwrap();
        assert_eq!(vod.id, "abc123");
        assert_eq!(vod.platform_vod_id.as_deref(), Some("2237432794"));
        assert_eq!(vod.duration.as_deref(), Some("3h 20m"));
        assert_eq!(vod.duration.as_ref().map(VodDuration::seconds), Some(12000));
        assert_eq!(vod.chapters.unwrap()[0].name.as_deref(), Some("Elden Ring"));
    }

    #[test]
    fn test_vod_deserialize_int_fields() {
        let json = r#"{
            "id": 1430,
            "platform_vod_id": "2768249708",
            "title": "Test Stream",
            "created_at": "2026-05-09T22:35:39.000Z",
            "duration": 25194,
            "vod_uploads": [
                {"upload_id": "M1giB9QeXNM", "thumbnail_url": "https://i.ytimg.com/vi/M1giB9QeXNM/mqdefault.jpg"}
            ],
            "chapters": [
                {"name": "HITMAN", "image": "https://example.com/{width}x{height}.jpg", "start": 0}
            ]
        }"#;
        let mut vods: Vec<Vod> = vec![serde_json::from_str(json).unwrap()];
        backfill_thumbnails(&mut vods);
        let vod = &vods[0];
        assert_eq!(vod.id, "1430");
        assert_eq!(vod.platform_vod_id.as_deref(), Some("2768249708"));
        assert_eq!(vod.duration.as_deref(), Some("6h 59m"));
        assert_eq!(vod.duration.as_ref().map(VodDuration::seconds), Some(25194));
        assert_eq!(vod.youtube.as_ref().unwrap()[0].id, "M1giB9QeXNM");
        assert_eq!(
            vod.thumbnail_url.as_deref(),
            Some("https://i.ytimg.com/vi/M1giB9QeXNM/mqdefault.jpg")
        );
    }

    #[test]
    fn test_format_duration_hm() {
        assert_eq!(format_duration_hm(25194), "6h 59m");
        assert_eq!(format_duration_hm(3600), "1h");
        assert_eq!(format_duration_hm(2700), "45m");
        assert_eq!(format_duration_hm(45), "45s");
        assert_eq!(format_duration_hm(0), "0m");
    }

    #[test]
    fn test_vod_duration_parses_string_fallbacks() {
        assert_eq!(VodDuration::from("07:02:52").seconds(), 25372);
        assert_eq!(VodDuration::from("3h 20m").seconds(), 12000);
        assert_eq!(VodDuration::from("").seconds(), 0);
    }

    #[test]
    fn test_upscale_chapter_image_placeholder() {
        assert_eq!(
            upscale_chapter_image("https://x.tv/foo_{width}x{height}.jpg"),
            "https://x.tv/foo_285x380.jpg"
        );
        assert_eq!(
            upscale_chapter_image("https://x.tv/foo_40x53.jpg"),
            "https://x.tv/foo_285x380.jpg"
        );
    }

    #[test]
    fn test_build_games_deduplicates() {
        let vods = vec![Vod {
            id: "1".into(),
            platform_vod_id: None,
            title: Some("Stream 1".into()),
            created_at: "2025-01-01T00:00:00Z".into(),
            updated_at: None,
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
                platform_vod_id: None,
                title: Some("Stream 1".into()),
                created_at: "2025-01-01T00:00:00Z".into(),
                updated_at: None,
                duration: Some("2h".into()),
                thumbnail_url: None,
                chapters: Some(vec![Chapter {
                    name: Some("Elden Ring".into()),
                    image: None,
                    start: None,
                }]),
                youtube: None,
                is_live: false,
            },
            Vod {
                id: "2".into(),
                platform_vod_id: None,
                title: Some("Stream 2".into()),
                created_at: "2025-01-02T00:00:00Z".into(),
                updated_at: None,
                duration: Some("3h".into()),
                thumbnail_url: None,
                chapters: Some(vec![Chapter {
                    name: Some("ELDEN RING".into()),
                    image: None,
                    start: None,
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
        let url = page_url(3);
        assert!(url.starts_with("https://archive.overpowered.tv/api/v1/moonmoon/vods?"));
        assert!(url.contains("page=3"), "missing page=3: {url}");
        assert!(url.contains("limit=50"), "missing limit=50: {url}");
        assert!(
            url.contains("sort=created_at"),
            "missing sort=created_at: {url}"
        );
        assert!(url.contains("order=desc"), "missing order=desc: {url}");
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

    #[test]
    fn test_live_empty_upload_row_is_not_playable() {
        let vod = Vod {
            id: "live".into(),
            platform_vod_id: None,
            title: Some("Live Stream".into()),
            created_at: "2026-05-12T00:00:00Z".into(),
            updated_at: Some("2026-05-12T00:10:00Z".into()),
            duration: None,
            thumbnail_url: None,
            chapters: None,
            youtube: Some(vec![]),
            is_live: true,
        };

        assert!(!is_playable_vod(&vod));
    }

    #[test]
    fn test_non_live_row_with_uploads_is_playable() {
        let vod = Vod {
            id: "1430".into(),
            platform_vod_id: Some("2768249708".into()),
            title: Some("Playable Stream".into()),
            created_at: "2026-05-09T22:35:39.000Z".into(),
            updated_at: Some("2026-05-10T00:00:00.000Z".into()),
            duration: Some("6h 59m".into()),
            thumbnail_url: None,
            chapters: None,
            youtube: Some(vec![YoutubeVideo {
                id: "M1giB9QeXNM".into(),
                thumbnail_url: None,
            }]),
            is_live: false,
        };

        assert!(is_playable_vod(&vod));
    }

    #[test]
    fn test_filter_playable_vods_removes_live_empty_upload_rows() {
        let mut vods = vec![
            Vod {
                id: "live".into(),
                platform_vod_id: None,
                title: Some("Live Stream".into()),
                created_at: "2026-05-12T00:00:00Z".into(),
                updated_at: Some("2026-05-12T00:10:00Z".into()),
                duration: None,
                thumbnail_url: None,
                chapters: None,
                youtube: Some(vec![]),
                is_live: true,
            },
            Vod {
                id: "1430".into(),
                platform_vod_id: Some("2768249708".into()),
                title: Some("Playable Stream".into()),
                created_at: "2026-05-09T22:35:39.000Z".into(),
                updated_at: Some("2026-05-10T00:00:00.000Z".into()),
                duration: Some("6h 59m".into()),
                thumbnail_url: None,
                chapters: None,
                youtube: Some(vec![YoutubeVideo {
                    id: "M1giB9QeXNM".into(),
                    thumbnail_url: None,
                }]),
                is_live: false,
            },
        ];

        filter_playable_vods(&mut vods);

        assert_eq!(vods.len(), 1);
        assert_eq!(vods[0].id, "1430");
    }

    #[test]
    fn test_catalog_snapshot_includes_latest_id_and_updated_at() {
        let vods = vec![Vod {
            id: "1430".into(),
            platform_vod_id: Some("2768249708".into()),
            title: Some("Playable Stream".into()),
            created_at: "2026-05-09T22:35:39.000Z".into(),
            updated_at: Some("2026-05-10T00:00:00.000Z".into()),
            duration: Some("6h 59m".into()),
            thumbnail_url: None,
            chapters: None,
            youtube: Some(vec![YoutubeVideo {
                id: "M1giB9QeXNM".into(),
                thumbnail_url: None,
            }]),
            is_live: false,
        }];

        let snapshot = CatalogSnapshot::from_vods(&vods);

        assert_eq!(snapshot.total, 1);
        assert_eq!(snapshot.latest_id.as_deref(), Some("1430"));
        assert_eq!(
            snapshot.latest_updated_at.as_deref(),
            Some("2026-05-10T00:00:00.000Z")
        );
    }

    #[test]
    fn test_catalog_snapshot_detects_same_count_updated_at_changes() {
        let cached = CatalogSnapshot {
            total: 1,
            latest_id: Some("1430".into()),
            latest_updated_at: Some("2026-05-10T00:00:00.000Z".into()),
        };
        let remote = CatalogSnapshot {
            total: 1,
            latest_id: Some("1430".into()),
            latest_updated_at: Some("2026-05-11T00:00:00.000Z".into()),
        };

        assert_ne!(cached, remote);
    }

    #[test]
    fn test_catalog_load_keeps_raw_snapshot_when_latest_row_is_not_playable() {
        let first = ApiResponse {
            meta: ApiMeta { total: 2 },
            data: vec![
                Vod {
                    id: "live".into(),
                    platform_vod_id: Some("2769756119".into()),
                    title: Some("Live Stream".into()),
                    created_at: "2026-05-12T00:00:00Z".into(),
                    updated_at: Some("2026-05-12T02:41:51.672Z".into()),
                    duration: None,
                    thumbnail_url: None,
                    chapters: None,
                    youtube: Some(vec![]),
                    is_live: true,
                },
                Vod {
                    id: "1430".into(),
                    platform_vod_id: Some("2768249708".into()),
                    title: Some("Playable Stream".into()),
                    created_at: "2026-05-09T22:35:39.000Z".into(),
                    updated_at: Some("2026-05-10T23:05:44.967Z".into()),
                    duration: Some("6h 59m".into()),
                    thumbnail_url: None,
                    chapters: None,
                    youtube: Some(vec![YoutubeVideo {
                        id: "M1giB9QeXNM".into(),
                        thumbnail_url: None,
                    }]),
                    is_live: false,
                },
            ],
        };

        let snapshot = CatalogSnapshot::from_api_response(&first);
        let catalog = CatalogLoad::from_raw_vods(snapshot, first.data);

        assert_eq!(
            catalog.snapshot,
            CatalogSnapshot {
                total: 2,
                latest_id: Some("live".into()),
                latest_updated_at: Some("2026-05-12T02:41:51.672Z".into()),
            }
        );
        assert_eq!(catalog.vods.len(), 1);
        assert_eq!(catalog.vods[0].id, "1430");
    }
}
