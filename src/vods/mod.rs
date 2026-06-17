use serde::{Deserialize, Serialize};

mod catalog;
mod games;

pub(crate) use catalog::{CatalogLoad, CatalogSnapshot, next_refresh_delay};
pub use catalog::{RefreshOutcome, load_catalog, refresh_in_place};
pub use games::{Game, build_dominant_games, build_games};
pub(crate) use games::{chapter_color_idx, month_abbr, month_abbr_num};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Vod {
    #[serde(deserialize_with = "deserialize_id_string")]
    pub id: String,
    #[serde(default)]
    pub platform: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_id_string")]
    pub platform_vod_id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_id_string")]
    pub platform_stream_id: Option<String>,
    pub title: Option<String>,
    pub created_at: String,
    #[serde(default)]
    pub started_at: Option<String>,
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
    #[serde(default)]
    pub duration: Option<f64>,
    #[serde(default)]
    pub end: Option<f64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct YoutubeVideo {
    #[serde(rename = "id", default)]
    pub row_id: Option<i64>,
    // Each upload has both an integer `id` (DB row) and a string `upload_id`
    // (YouTube video ID); we want the latter. Aliasing `"id"` would match the
    // integer first and fail to deserialize as a String.
    #[serde(rename = "upload_id")]
    pub id: String,
    #[serde(default)]
    pub thumbnail_url: Option<String>,
    #[serde(default)]
    pub part: Option<i64>,
    #[serde(default)]
    pub duration: Option<i64>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(rename = "type", default)]
    pub upload_type: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum IdValue {
    Int(i64),
    Str(String),
}

impl IdValue {
    fn into_string(self) -> String {
        match self {
            IdValue::Int(n) => n.to_string(),
            IdValue::Str(s) => s,
        }
    }
}

fn deserialize_id_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(IdValue::deserialize(deserializer)?.into_string())
}

fn deserialize_optional_id_string<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value: Option<IdValue> = Option::deserialize(deserializer)?;
    Ok(value.map(IdValue::into_string))
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

pub fn upscale_chapter_image(url: &str) -> String {
    url.replace("{width}x{height}", "285x380")
        .replace("40x53", "285x380")
}

pub fn is_playable_vod(vod: &Vod) -> bool {
    !vod.is_live && !canonical_youtube_uploads(vod).is_empty()
}

pub fn filter_playable_vods(vods: &mut Vec<Vod>) {
    vods.retain(is_playable_vod);
}

pub fn canonical_youtube_uploads(vod: &Vod) -> Vec<YoutubeVideo> {
    let Some(uploads) = vod.youtube.as_ref() else {
        return Vec::new();
    };
    if uploads.is_empty() {
        return Vec::new();
    }

    let completed: Vec<(usize, &YoutubeVideo)> = uploads
        .iter()
        .enumerate()
        .filter(|(_, upload)| {
            upload
                .status
                .as_deref()
                .is_some_and(|status| status.eq_ignore_ascii_case("COMPLETED"))
        })
        .collect();
    let mut selected = if completed.is_empty() {
        uploads.iter().enumerate().collect()
    } else {
        let typed = |upload_type: &str| -> Vec<(usize, &YoutubeVideo)> {
            completed
                .iter()
                .copied()
                .filter(|(_, upload)| {
                    upload
                        .upload_type
                        .as_deref()
                        .is_some_and(|value| value.eq_ignore_ascii_case(upload_type))
                })
                .collect()
        };
        let vod_uploads = typed("vod");
        if !vod_uploads.is_empty() {
            let live_uploads = typed("live");
            if upload_set_covers_stream(&vod_uploads, vod) == Some(false)
                && upload_set_covers_stream(&live_uploads, vod) == Some(true)
            {
                live_uploads
            } else {
                vod_uploads
            }
        } else {
            let live_uploads = typed("live");
            if live_uploads.is_empty() {
                completed
            } else {
                live_uploads
            }
        }
    };

    selected.sort_by(
        |(left_idx, left), (right_idx, right)| match (left.part, right.part) {
            (Some(a), Some(b)) => a.cmp(&b).then_with(|| left_idx.cmp(right_idx)),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => left_idx.cmp(right_idx),
        },
    );
    selected
        .into_iter()
        .map(|(_, upload)| upload.clone())
        .collect()
}

fn upload_set_covers_stream(uploads: &[(usize, &YoutubeVideo)], vod: &Vod) -> Option<bool> {
    let stream_duration = vod.duration.as_ref()?.seconds();
    if stream_duration <= 0 || uploads.is_empty() {
        return None;
    }

    let mut upload_duration = 0_i64;
    for (_, upload) in uploads {
        let duration = upload.duration?;
        if duration <= 0 {
            return None;
        }
        upload_duration = upload_duration.saturating_add(duration);
    }

    Some(upload_duration >= stream_duration)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn upload(
        id: &str,
        part: Option<i64>,
        duration: Option<i64>,
        status: Option<&str>,
        upload_type: Option<&str>,
    ) -> YoutubeVideo {
        YoutubeVideo {
            row_id: None,
            id: id.into(),
            thumbnail_url: Some(format!("https://img.example/{id}.jpg")),
            part,
            duration,
            status: status.map(str::to_string),
            upload_type: upload_type.map(str::to_string),
            created_at: None,
        }
    }

    fn vod_with_uploads(uploads: Vec<YoutubeVideo>) -> Vod {
        Vod {
            id: "vod".into(),
            platform: Some("twitch".into()),
            platform_vod_id: Some("platform-vod".into()),
            platform_stream_id: Some("platform-stream".into()),
            title: Some("Playable Stream".into()),
            created_at: "2026-05-10T00:00:00.000Z".into(),
            started_at: Some("2026-05-09T22:35:39.000Z".into()),
            updated_at: None,
            duration: Some("6h".into()),
            thumbnail_url: None,
            chapters: None,
            youtube: Some(uploads),
            is_live: false,
        }
    }

    #[test]
    fn test_vod_deserialize_new_api_fields() {
        let json = r#"{
            "id": 1430,
            "platform": "twitch",
            "platform_vod_id": "2768249708",
            "platform_stream_id": "51234567890",
            "title": "Test Stream",
            "created_at": "2026-05-10T23:05:44.967Z",
            "started_at": "2026-05-09T22:35:39.000Z",
            "duration": 25194,
            "vod_uploads": [
                {
                    "id": 99,
                    "upload_id": "M1giB9QeXNM",
                    "thumbnail_url": "https://i.ytimg.com/vi/M1giB9QeXNM/mqdefault.jpg",
                    "part": 1,
                    "duration": 10800,
                    "status": "COMPLETED",
                    "type": "vod",
                    "created_at": "2026-05-10T01:00:00.000Z"
                }
            ],
            "chapters": [
                {"name": "HITMAN", "image": "https://example.com/{width}x{height}.jpg", "start": 0, "duration": 3600.5, "end": 3600.5}
            ]
        }"#;

        let vod: Vod = serde_json::from_str(json).unwrap();

        assert_eq!(vod.platform.as_deref(), Some("twitch"));
        assert_eq!(vod.platform_stream_id.as_deref(), Some("51234567890"));
        assert_eq!(vod.started_at.as_deref(), Some("2026-05-09T22:35:39.000Z"));
        let upload = &vod.youtube.as_ref().unwrap()[0];
        assert_eq!(upload.row_id, Some(99));
        assert_eq!(upload.part, Some(1));
        assert_eq!(upload.duration, Some(10800));
        assert_eq!(upload.status.as_deref(), Some("COMPLETED"));
        assert_eq!(upload.upload_type.as_deref(), Some("vod"));
        assert_eq!(
            upload.created_at.as_deref(),
            Some("2026-05-10T01:00:00.000Z")
        );
        let chapter = &vod.chapters.as_ref().unwrap()[0];
        assert_eq!(chapter.duration, Some(3600.5));
        assert_eq!(chapter.end, Some(3600.5));
    }

    #[test]
    fn test_canonical_youtube_uploads_prefers_completed_vod_set() {
        let vod = vod_with_uploads(vec![
            upload(
                "live-1",
                Some(1),
                Some(100),
                Some("COMPLETED"),
                Some("live"),
            ),
            upload("vod-2", Some(2), Some(200), Some("COMPLETED"), Some("vod")),
            upload("vod-1", Some(1), Some(100), Some("COMPLETED"), Some("vod")),
            upload("pending", Some(1), Some(100), Some("PENDING"), Some("vod")),
        ]);

        let ids: Vec<_> = canonical_youtube_uploads(&vod)
            .into_iter()
            .map(|u| u.id)
            .collect();

        assert_eq!(ids, vec!["vod-1", "vod-2"]);
    }

    #[test]
    fn test_canonical_youtube_uploads_falls_back_to_live_when_vod_set_is_incomplete() {
        let vod = vod_with_uploads(vec![
            upload(
                "live-1",
                Some(1),
                Some(10800),
                Some("COMPLETED"),
                Some("live"),
            ),
            upload(
                "live-2",
                Some(2),
                Some(10800),
                Some("COMPLETED"),
                Some("live"),
            ),
            upload(
                "live-3",
                Some(3),
                Some(7830),
                Some("COMPLETED"),
                Some("live"),
            ),
            upload(
                "vod-1",
                Some(1),
                Some(10800),
                Some("COMPLETED"),
                Some("vod"),
            ),
            upload(
                "vod-2",
                Some(2),
                Some(10800),
                Some("COMPLETED"),
                Some("vod"),
            ),
        ]);
        let mut vod = vod;
        vod.duration = Some(VodDuration::from_seconds(29430));

        let ids: Vec<_> = canonical_youtube_uploads(&vod)
            .into_iter()
            .map(|u| u.id)
            .collect();

        assert_eq!(ids, vec!["live-1", "live-2", "live-3"]);
    }

    #[test]
    fn test_canonical_youtube_uploads_uses_live_when_no_vod_set() {
        let vod = vod_with_uploads(vec![
            upload("live-2", Some(2), None, Some("COMPLETED"), Some("live")),
            upload("live-1", Some(1), None, Some("COMPLETED"), Some("live")),
        ]);

        let ids: Vec<_> = canonical_youtube_uploads(&vod)
            .into_iter()
            .map(|u| u.id)
            .collect();

        assert_eq!(ids, vec!["live-1", "live-2"]);
    }

    #[test]
    fn test_canonical_youtube_uploads_sorts_missing_parts_last_stably() {
        let vod = vod_with_uploads(vec![
            upload("missing-a", None, None, Some("COMPLETED"), Some("vod")),
            upload("part-1", Some(1), None, Some("COMPLETED"), Some("vod")),
            upload("missing-b", None, None, Some("COMPLETED"), Some("vod")),
        ]);

        let ids: Vec<_> = canonical_youtube_uploads(&vod)
            .into_iter()
            .map(|u| u.id)
            .collect();

        assert_eq!(ids, vec!["part-1", "missing-a", "missing-b"]);
    }

    #[test]
    fn test_canonical_youtube_uploads_falls_back_when_no_completed_uploads() {
        let vod = vod_with_uploads(vec![
            upload(
                "processing-vod",
                Some(2),
                None,
                Some("PROCESSING"),
                Some("vod"),
            ),
            upload(
                "processing-live",
                Some(1),
                None,
                Some("PROCESSING"),
                Some("live"),
            ),
        ]);

        let ids: Vec<_> = canonical_youtube_uploads(&vod)
            .into_iter()
            .map(|u| u.id)
            .collect();

        assert_eq!(ids, vec!["processing-live", "processing-vod"]);
        assert!(is_playable_vod(&vod));
    }

    #[test]
    fn test_canonical_youtube_uploads_empty_uploads_are_not_playable() {
        let vod = vod_with_uploads(vec![]);

        assert!(canonical_youtube_uploads(&vod).is_empty());
        assert!(!is_playable_vod(&vod));
    }

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
    fn test_live_empty_upload_row_is_not_playable() {
        let vod = Vod {
            id: "live".into(),
            platform: None,
            platform_vod_id: None,
            platform_stream_id: None,
            title: Some("Live Stream".into()),
            created_at: "2026-05-12T00:00:00Z".into(),
            started_at: None,
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
        };

        assert!(is_playable_vod(&vod));
    }

    #[test]
    fn test_filter_playable_vods_removes_live_empty_upload_rows() {
        let mut vods = vec![
            Vod {
                id: "live".into(),
                platform: None,
                platform_vod_id: None,
                platform_stream_id: None,
                title: Some("Live Stream".into()),
                created_at: "2026-05-12T00:00:00Z".into(),
                started_at: None,
                updated_at: Some("2026-05-12T00:10:00Z".into()),
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
            },
        ];

        filter_playable_vods(&mut vods);

        assert_eq!(vods.len(), 1);
        assert_eq!(vods[0].id, "1430");
    }
}
