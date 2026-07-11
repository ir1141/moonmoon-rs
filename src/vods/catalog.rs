use super::{Vod, canonical_youtube_uploads, filter_playable_vods};
use serde::Deserialize;
use std::future::Future;
use std::time::Duration;

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
    #[serde(deserialize_with = "deserialize_lossy_vods")]
    pub data: Vec<Vod>,
    pub meta: ApiMeta,
}

/// The upstream API is inconsistent; a single malformed row must not fail the
/// entire page (and with it the whole catalog fetch). Parse rows individually
/// and skip the broken ones with a warning.
///
/// Every row failing is different: that's schema drift, and returning Ok with
/// an empty page would let a refresh swap a healthy in-memory catalog for
/// nothing (and the snapshot comparison would then report Unchanged forever,
/// pinning the site empty). Fail the page so the caller keeps what it has.
fn deserialize_lossy_vods<'de, D>(deserializer: D) -> Result<Vec<Vod>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let values = Vec::<serde_json::Value>::deserialize(deserializer)?;
    let total = values.len();
    let vods: Vec<Vod> = values
        .into_iter()
        .filter_map(|value| match serde_json::from_value::<Vod>(value) {
            Ok(vod) => Some(vod),
            Err(e) => {
                tracing::warn!("skipping malformed vod row: {e}");
                None
            }
        })
        .collect();
    if vods.is_empty() && total > 0 {
        return Err(serde::de::Error::custom(format!(
            "all {total} vod rows failed to parse — refusing lossy result"
        )));
    }
    Ok(vods)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CatalogSnapshot {
    pub total: usize,
    pub latest_id: Option<String>,
    pub latest_updated_at: Option<String>,
    /// Whether the newest upstream row is playable. The newest VOD is often
    /// still live or awaiting upload transcoding when first seen, so it gets
    /// filtered out of the catalog while `total`/`latest_id`/`latest_updated_at`
    /// already record it. Without this flag the cheap refresh check would then
    /// report Unchanged forever and never re-ingest it once it becomes playable.
    pub latest_playable: bool,
}

impl CatalogSnapshot {
    #[cfg(test)]
    pub(crate) fn from_vods(vods: &[Vod]) -> Self {
        let latest = vods.first();
        Self {
            total: vods.len(),
            latest_id: latest.map(|vod| vod.id.clone()),
            latest_updated_at: latest.and_then(|vod| vod.updated_at.clone()),
            latest_playable: latest.is_some_and(|vod| vod.is_playable()),
        }
    }

    fn from_api_response(resp: &ApiResponse) -> Self {
        let latest = resp.data.first();
        Self {
            total: resp.meta.total,
            latest_id: latest.map(|vod| vod.id.clone()),
            latest_updated_at: latest.and_then(|vod| vod.updated_at.clone()),
            latest_playable: latest.is_some_and(|vod| vod.is_playable()),
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
                latest_playable: false,
            },
        }
    }

    fn from_raw_vods(snapshot: CatalogSnapshot, mut vods: Vec<Vod>) -> Self {
        filter_playable_vods(&mut vods);
        backfill_thumbnails(&mut vods);

        Self { vods, snapshot }
    }
}

pub(crate) const REFRESH_INTERVAL: Duration = Duration::from_secs(60 * 60);
/// While the catalog is empty (failed/timed-out boot fetch) retry much faster
/// so a bad boot doesn't leave the site empty for six hours.
pub(crate) const EMPTY_RETRY_INTERVAL: Duration = Duration::from_secs(60);

pub(crate) fn next_refresh_delay(vod_count: usize) -> Duration {
    if vod_count == 0 {
        EMPTY_RETRY_INTERVAL
    } else {
        REFRESH_INTERVAL
    }
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
    Ok(CatalogSnapshot::from_api_response(&resp))
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

/// The catalog's error currency: a `reqwest::Error` in production, or whatever
/// a test fake needs to raise. It is only ever surfaced as a `String`.
type SourceError = Box<dyn std::error::Error + Send + Sync>;

/// A source of catalog data: the cheap snapshot poll and the full paged load.
/// [`HttpArchive`] serves it from the live archive; a test fake serves it from
/// memory, so [`plan_refresh`] can be driven without touching the network. The
/// futures are `Send` because refresh runs inside `tokio::spawn`.
trait VodSource {
    fn snapshot(&self) -> impl Future<Output = Result<CatalogSnapshot, SourceError>> + Send;
    fn load(&self) -> impl Future<Output = Result<CatalogLoad, SourceError>> + Send;
}

/// The sole production adapter: every catalog network access goes through here.
struct HttpArchive<'a> {
    client: &'a reqwest::Client,
}

impl<'a> HttpArchive<'a> {
    fn new(client: &'a reqwest::Client) -> Self {
        Self { client }
    }
}

impl VodSource for HttpArchive<'_> {
    async fn snapshot(&self) -> Result<CatalogSnapshot, SourceError> {
        Ok(fetch_catalog_snapshot(self.client).await?)
    }

    async fn load(&self) -> Result<CatalogLoad, SourceError> {
        Ok(fetch_catalog_load(self.client).await?)
    }
}

/// Upstream doesn't expose `thumbnail_url` at the VOD level — only on each
/// `vod_uploads` entry. Lift it from the first upload so templates and
/// `VodDisplay` can keep reading `vod.thumbnail_url` directly.
fn backfill_thumbnails(vods: &mut [Vod]) {
    for vod in vods.iter_mut() {
        if vod.thumbnail_url.is_some() {
            continue;
        }
        let thumb = canonical_youtube_uploads(vod)
            .iter()
            .find_map(|u| u.thumbnail_url.clone());
        if let Some(thumb) = thumb {
            vod.thumbnail_url = Some(thumb);
        }
    }
}

pub async fn load_catalog(client: &reqwest::Client) -> CatalogLoad {
    match HttpArchive::new(client).load().await {
        Ok(catalog) => catalog,
        Err(e) => {
            tracing::error!("failed to fetch vods: {e}");
            tracing::error!("starting with 0 vods — site will be empty until next refresh");
            CatalogLoad::empty()
        }
    }
}

/// The refresh decision, isolated from the lock and the catalog swap so it can
/// be driven end-to-end against a fake source. Poll the cheap snapshot; if it
/// matches the cached one nothing moved, otherwise fetch the full catalog. The
/// `latest_playable` field means a head VOD that was filtered out as not-yet-
/// playable still flips the snapshot and re-ingests once it is ready.
#[derive(Debug)]
enum RefreshPlan {
    Unchanged,
    Refresh(CatalogLoad),
    Failed(String),
}

async fn plan_refresh(cached: &CatalogSnapshot, source: &impl VodSource) -> RefreshPlan {
    let remote = match source.snapshot().await {
        Ok(snapshot) => snapshot,
        Err(e) => {
            tracing::error!("refresh: failed to check catalog snapshot: {e}");
            return RefreshPlan::Failed(format!("failed to check catalog snapshot: {e}"));
        }
    };

    if remote == *cached {
        tracing::info!("refresh: catalog unchanged ({cached:?})");
        return RefreshPlan::Unchanged;
    }

    tracing::info!("refresh: catalog changed ({cached:?} -> {remote:?}), fetching...");
    match source.load().await {
        Ok(catalog) => RefreshPlan::Refresh(catalog),
        Err(e) => {
            tracing::error!("refresh: failed to fetch vods: {e}");
            RefreshPlan::Failed(format!("failed to fetch vods: {e}"))
        }
    }
}

pub async fn refresh_in_place(state: &crate::SharedState) -> RefreshOutcome {
    let _refresh_guard = match state.refresh_lock.try_lock() {
        Ok(g) => g,
        Err(_) => {
            tracing::info!("refresh: already in progress, skipping");
            return RefreshOutcome::Busy;
        }
    };

    let cached_snapshot = state.catalog.read().await.snapshot.clone();

    match plan_refresh(&cached_snapshot, &HttpArchive::new(&state.http_client)).await {
        RefreshPlan::Unchanged => {
            let count = state.catalog.read().await.vods.len();
            RefreshOutcome::Unchanged(count)
        }
        RefreshPlan::Refresh(catalog) => {
            let new_catalog = std::sync::Arc::new(crate::Catalog::build(catalog));
            let count = new_catalog.vods.len();
            *state.catalog.write().await = new_catalog;
            tracing::info!("refresh: complete ({count} vods)");
            RefreshOutcome::Refreshed(count)
        }
        RefreshPlan::Failed(msg) => RefreshOutcome::Error(msg),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vods::{VodDuration, YoutubeVideo};

    #[test]
    fn test_api_response_skips_malformed_rows() {
        let json = r#"{"meta":{"total":2},"data":[
            {"id":1,"title":"good","created_at":"2026-01-01T00:00:00Z"},
            {"title":"missing id and created_at"}
        ]}"#;
        let resp: ApiResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.data.len(), 1);
        assert_eq!(resp.data[0].id, "1");
    }

    #[test]
    fn test_api_response_rejects_page_where_every_row_is_malformed() {
        // Every row failing to parse means upstream schema drift, not a few
        // bad rows — treating it as a successful empty page would let a
        // refresh wipe a healthy catalog.
        let json = r#"{"meta":{"total":2},"data":[
            {"title":"missing id and created_at"},
            {"title":"also malformed"}
        ]}"#;
        assert!(serde_json::from_str::<ApiResponse>(json).is_err());
    }

    #[test]
    fn test_api_response_accepts_genuinely_empty_page() {
        let json = r#"{"meta":{"total":0},"data":[]}"#;
        let resp: ApiResponse = serde_json::from_str(json).unwrap();
        assert!(resp.data.is_empty());
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
    fn test_catalog_snapshot_includes_latest_id_and_updated_at() {
        let vods = vec![Vod {
            id: "1430".into(),
            platform: None,
            platform_vod_id: Some("2768249708".into()),
            platform_stream_id: None,
            title: Some("Playable Stream".into()),
            created_at: "2026-05-09T22:35:39.000Z".into(),
            started_at: None,
            updated_at: Some("2026-05-10T00:00:00.000Z".into()),
            duration: Some("6h 59m".into()),
            thumbnail_url: None,
            chapters: None,
            youtube: Some(vec![YoutubeVideo {
                row_id: None,
                id: "M1giB9QeXNM".into(),
                thumbnail_url: None,
                part: None,
                duration: None,
                status: None,
                upload_type: None,
                created_at: None,
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
            latest_playable: true,
        };
        let remote = CatalogSnapshot {
            total: 1,
            latest_id: Some("1430".into()),
            latest_updated_at: Some("2026-05-11T00:00:00.000Z".into()),
            latest_playable: true,
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
                    platform: None,
                    platform_vod_id: Some("2769756119".into()),
                    platform_stream_id: None,
                    title: Some("Live Stream".into()),
                    created_at: "2026-05-12T00:00:00Z".into(),
                    started_at: None,
                    updated_at: Some("2026-05-12T02:41:51.672Z".into()),
                    duration: None,
                    thumbnail_url: None,
                    chapters: None,
                    youtube: Some(vec![]),
                    is_live: true,
                },
                Vod {
                    id: "1430".into(),
                    platform: None,
                    platform_vod_id: Some("2768249708".into()),
                    platform_stream_id: None,
                    title: Some("Playable Stream".into()),
                    created_at: "2026-05-09T22:35:39.000Z".into(),
                    started_at: None,
                    updated_at: Some("2026-05-10T23:05:44.967Z".into()),
                    duration: Some("6h 59m".into()),
                    thumbnail_url: None,
                    chapters: None,
                    youtube: Some(vec![YoutubeVideo {
                        row_id: None,
                        id: "M1giB9QeXNM".into(),
                        thumbnail_url: None,
                        part: None,
                        duration: None,
                        status: None,
                        upload_type: None,
                        created_at: None,
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
                latest_playable: false,
            }
        );
        assert_eq!(catalog.vods.len(), 1);
        assert_eq!(catalog.vods[0].id, "1430");
    }

    #[test]
    fn test_snapshot_changes_when_latest_row_becomes_playable() {
        // The newest VOD while still live and the same VOD once its uploads have
        // completed share id/total/updated_at — only playability differs. The
        // snapshot must distinguish them, otherwise the refresh never re-ingests
        // a head VOD that was filtered out as not-yet-playable.
        let head = |is_live: bool, uploads: Vec<YoutubeVideo>| ApiResponse {
            meta: ApiMeta { total: 1 },
            data: vec![Vod {
                id: "1465".into(),
                platform: None,
                platform_vod_id: None,
                platform_stream_id: None,
                title: Some("gamer time".into()),
                created_at: "2026-06-27T22:20:27.000Z".into(),
                started_at: None,
                updated_at: Some("2026-06-28T06:44:01.803Z".into()),
                duration: None,
                thumbnail_url: None,
                chapters: None,
                youtube: Some(uploads),
                is_live,
            }],
        };

        let pending = CatalogSnapshot::from_api_response(&head(true, vec![]));
        let ready = CatalogSnapshot::from_api_response(&head(
            false,
            vec![YoutubeVideo {
                row_id: None,
                id: "XH3oi5Mf1Lo".into(),
                thumbnail_url: None,
                part: None,
                duration: None,
                status: Some("COMPLETED".into()),
                upload_type: Some("live".into()),
                created_at: None,
            }],
        ));

        assert_eq!(pending.latest_id, ready.latest_id);
        assert_eq!(pending.latest_updated_at, ready.latest_updated_at);
        assert_eq!(pending.total, ready.total);
        assert!(!pending.latest_playable);
        assert!(ready.latest_playable);
        assert_ne!(pending, ready);
    }

    #[test]
    fn test_next_refresh_delay_retries_fast_when_empty() {
        assert_eq!(next_refresh_delay(0), EMPTY_RETRY_INTERVAL);
        assert_eq!(next_refresh_delay(1500), REFRESH_INTERVAL);
    }

    /// An in-memory [`VodSource`]: canned snapshot and load, either of which can
    /// be an error the live archive could never mint. Drives the whole refresh
    /// decision tree without a network.
    struct FakeArchive {
        snapshot: Result<CatalogSnapshot, String>,
        load: Result<CatalogLoad, String>,
    }

    impl VodSource for FakeArchive {
        async fn snapshot(&self) -> Result<CatalogSnapshot, SourceError> {
            self.snapshot.clone().map_err(Into::into)
        }
        async fn load(&self) -> Result<CatalogLoad, SourceError> {
            self.load.clone().map_err(Into::into)
        }
    }

    fn make_snapshot(total: usize, latest_id: &str, latest_playable: bool) -> CatalogSnapshot {
        CatalogSnapshot {
            total,
            latest_id: Some(latest_id.into()),
            latest_updated_at: Some("2026-05-10T00:00:00.000Z".into()),
            latest_playable,
        }
    }

    #[tokio::test]
    async fn plan_refresh_reports_unchanged_when_snapshot_matches() {
        let cached = make_snapshot(10, "1430", true);
        let source = FakeArchive {
            snapshot: Ok(cached.clone()),
            load: Err("load must not be called when unchanged".into()),
        };
        assert!(matches!(
            plan_refresh(&cached, &source).await,
            RefreshPlan::Unchanged
        ));
    }

    #[tokio::test]
    async fn plan_refresh_fetches_when_snapshot_moves() {
        let cached = make_snapshot(10, "1430", true);
        let source = FakeArchive {
            snapshot: Ok(make_snapshot(11, "1500", true)),
            load: Ok(CatalogLoad::empty()),
        };
        assert!(matches!(
            plan_refresh(&cached, &source).await,
            RefreshPlan::Refresh(_)
        ));
    }

    #[tokio::test]
    async fn plan_refresh_reingests_when_only_latest_playable_flips() {
        // Same id/total/updated_at as the cached head VOD — only playability
        // differs. This is the not-yet-playable head that must re-ingest once
        // its uploads complete, and it had no coverage before the seam.
        let cached = make_snapshot(10, "1465", false);
        let source = FakeArchive {
            snapshot: Ok(make_snapshot(10, "1465", true)),
            load: Ok(CatalogLoad::empty()),
        };
        assert!(matches!(
            plan_refresh(&cached, &source).await,
            RefreshPlan::Refresh(_)
        ));
    }

    #[tokio::test]
    async fn plan_refresh_fails_when_snapshot_poll_errors() {
        let cached = make_snapshot(10, "1430", true);
        let source = FakeArchive {
            snapshot: Err("upstream 503".into()),
            load: Err("load must not be called after a poll failure".into()),
        };
        match plan_refresh(&cached, &source).await {
            RefreshPlan::Failed(msg) => {
                assert!(msg.contains("failed to check catalog snapshot"), "{msg}")
            }
            other => panic!("expected Failed, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn plan_refresh_fails_when_load_errors() {
        let cached = make_snapshot(10, "1430", true);
        let source = FakeArchive {
            snapshot: Ok(make_snapshot(11, "1500", true)),
            load: Err("connection reset mid-page".into()),
        };
        match plan_refresh(&cached, &source).await {
            RefreshPlan::Failed(msg) => assert!(msg.contains("failed to fetch vods"), "{msg}"),
            other => panic!("expected Failed, got {other:?}"),
        }
    }
}
