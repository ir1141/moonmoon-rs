use std::collections::HashMap;
use std::time::Duration;

use crate::emotes::EmoteRecord;
use crate::emotes::parse;

/// Moonmoon's Twitch user ID — shared with player.js (MOONMOON_TWITCH_ID).
/// All three providers key channel emotes by this ID.
pub const MOONMOON_TWITCH_ID: &str = "121059319";

/// Fetch a JSON URL with one short retry on transient failures (5xx or io).
/// 4xx responses are returned as Err immediately.
async fn fetch_json(
    client: &reqwest::Client,
    url: &str,
) -> Result<serde_json::Value, reqwest::Error> {
    for attempt in 0..2 {
        match client.get(url).send().await {
            Ok(resp) => {
                let status = resp.status();
                if status.is_server_error() && attempt == 0 {
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    continue;
                }
                let resp = resp.error_for_status()?;
                return resp.json().await;
            }
            Err(e) if attempt == 0 && e.is_timeout() => {
                tokio::time::sleep(Duration::from_millis(500)).await;
                continue;
            }
            Err(e) => return Err(e),
        }
    }
    unreachable!("loop returns inside both branches");
}

type EmoteParser = fn(&serde_json::Value) -> HashMap<String, EmoteRecord>;

/// Per-endpoint budget, covering `fetch_json`'s retry. Endpoints are fetched
/// concurrently, so this also bounds a whole prefetch.
const ENDPOINT_TIMEOUT: Duration = Duration::from_secs(15);

/// The 6 prefetched endpoints (3 providers × {channel, global}). Order is
/// merge priority: on a name collision the earlier endpoint wins, so every
/// channel set outranks every global one, as on Twitch.
fn endpoints() -> Vec<(String, EmoteParser)> {
    let user_id = MOONMOON_TWITCH_ID;
    vec![
        (
            format!("https://7tv.io/v3/users/twitch/{user_id}"),
            parse::parse_seventv_user,
        ),
        (
            format!("https://api.betterttv.net/3/cached/users/twitch/{user_id}"),
            parse::parse_bttv_user,
        ),
        (
            format!("https://api.frankerfacez.com/v1/room/id/{user_id}"),
            parse::parse_ffz,
        ),
        (
            "https://7tv.io/v3/emote-sets/global".to_string(),
            parse::parse_seventv_global,
        ),
        (
            "https://api.betterttv.net/3/cached/emotes/global".to_string(),
            parse::parse_bttv_global,
        ),
        (
            "https://api.frankerfacez.com/v1/set/global".to_string(),
            parse::parse_ffz,
        ),
    ]
}

type EmoteSet = HashMap<String, EmoteRecord>;

/// One slot per endpoint, in `endpoints()` order; `None` where it failed.
pub async fn fetch_sets(client: &reqwest::Client) -> Vec<Option<EmoteSet>> {
    fetch_sets_from(client, endpoints(), ENDPOINT_TIMEOUT).await
}

async fn fetch_sets_from(
    client: &reqwest::Client,
    endpoints: Vec<(String, EmoteParser)>,
    timeout: Duration,
) -> Vec<Option<EmoteSet>> {
    let mut out = vec![None; endpoints.len()];
    let mut tasks = tokio::task::JoinSet::new();
    for (i, (url, parser)) in endpoints.into_iter().enumerate() {
        let client = client.clone();
        tasks.spawn(async move {
            let set = match tokio::time::timeout(timeout, fetch_json(&client, &url)).await {
                Ok(Ok(json)) => {
                    let set = parser(&json);
                    tracing::info!("emotes: {url} returned {} entries", set.len());
                    Some(set)
                }
                Ok(Err(e)) => {
                    tracing::warn!("emotes: failed to load {url}: {e}");
                    None
                }
                Err(_) => {
                    tracing::warn!("emotes: {url} timed out after {timeout:?}");
                    None
                }
            };
            (i, set)
        });
    }
    while let Some(joined) = tasks.join_next().await {
        if let Ok((i, set)) = joined {
            out[i] = set;
        }
    }
    out
}

/// Last successful set per endpoint. A failed fetch keeps that endpoint's
/// previous set, so one provider outage can't drop its emotes from the index.
#[derive(Default)]
pub struct PrefetchCache {
    sets: Vec<Option<EmoteSet>>,
}

impl PrefetchCache {
    pub fn update(&mut self, fetched: Vec<Option<EmoteSet>>) {
        self.sets.resize(fetched.len(), None);
        for (slot, set) in self.sets.iter_mut().zip(fetched) {
            if set.is_some() {
                *slot = set;
            }
        }
    }

    pub fn is_complete(&self) -> bool {
        !self.sets.is_empty() && self.sets.iter().all(Option::is_some)
    }

    pub fn merged(&self) -> EmoteSet {
        let mut out = HashMap::new();
        for set in self.sets.iter().flatten() {
            for (k, v) in set {
                out.entry(k.clone()).or_insert_with(|| v.clone());
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::emotes::EmoteProvider;

    fn set(names: &[(&str, &str)]) -> EmoteSet {
        names
            .iter()
            .map(|(name, url)| {
                (
                    name.to_string(),
                    EmoteRecord {
                        url: url.to_string(),
                        provider: EmoteProvider::Bttv,
                        owner: None,
                    },
                )
            })
            .collect()
    }

    #[test]
    fn prefetch_cache_keeps_previous_set_when_endpoint_fails() {
        let mut cache = PrefetchCache::default();
        cache.update(vec![Some(set(&[("A", "a1")])), Some(set(&[("B", "b1")]))]);
        cache.update(vec![None, Some(set(&[("B", "b2")]))]);
        let merged = cache.merged();
        assert_eq!(merged["A"].url, "a1");
        assert_eq!(merged["B"].url, "b2");
        assert!(cache.is_complete());
    }

    #[test]
    fn prefetch_cache_is_incomplete_until_every_endpoint_has_loaded() {
        let mut cache = PrefetchCache::default();
        assert!(!cache.is_complete());
        cache.update(vec![Some(set(&[])), None]);
        assert!(!cache.is_complete());
        cache.update(vec![None, Some(set(&[]))]);
        assert!(cache.is_complete());
    }

    #[test]
    fn prefetch_cache_merge_prefers_earlier_endpoint_on_collision() {
        let mut cache = PrefetchCache::default();
        cache.update(vec![
            Some(set(&[("X", "first")])),
            Some(set(&[("X", "second")])),
        ]);
        assert_eq!(cache.merged()["X"].url, "first");
    }

    fn parse_names(json: &serde_json::Value) -> EmoteSet {
        let names: Vec<&str> = json["names"]
            .as_array()
            .map(|a| a.iter().filter_map(|v| v.as_str()).collect())
            .unwrap_or_default();
        set(&names.iter().map(|n| (*n, "u")).collect::<Vec<_>>())
    }

    async fn serve_once(response: String) -> String {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            while let Ok((mut sock, _)) = listener.accept().await {
                let mut buf = [0u8; 1024];
                let _ = sock.read(&mut buf).await;
                let _ = sock.write_all(response.as_bytes()).await;
            }
        });
        format!("http://{addr}/")
    }

    async fn serve_hang() -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let mut held = Vec::new();
            while let Ok((sock, _)) = listener.accept().await {
                held.push(sock);
            }
        });
        format!("http://{addr}/")
    }

    #[tokio::test]
    async fn hung_endpoint_does_not_discard_other_endpoints() {
        let body = r#"{"names":["Kappa"]}"#;
        let ok = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        );
        let endpoints: Vec<(String, EmoteParser)> = vec![
            (serve_hang().await, parse_names),
            (serve_once(ok).await, parse_names),
            (
                serve_once(
                    "HTTP/1.1 404 Not Found\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
                        .to_string(),
                )
                .await,
                parse_names,
            ),
        ];
        let started = std::time::Instant::now();
        let sets = fetch_sets_from(
            &reqwest::Client::new(),
            endpoints,
            Duration::from_millis(300),
        )
        .await;
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "{:?}",
            started.elapsed()
        );
        assert!(sets[0].is_none());
        assert!(sets[1].as_ref().unwrap().contains_key("Kappa"));
        assert!(sets[2].is_none());
    }

    #[test]
    fn channel_sets_outrank_global_sets() {
        let urls: Vec<String> = endpoints().into_iter().map(|(u, _)| u).collect();
        let is_global = |u: &String| u.contains("global");
        let first_global = urls.iter().position(is_global).unwrap();
        assert_eq!(first_global, 3, "{urls:?}");
        assert!(urls[first_global..].iter().all(is_global), "{urls:?}");
    }

    #[test]
    fn moonmoon_twitch_id_is_numeric_string() {
        assert!(MOONMOON_TWITCH_ID.chars().all(|c| c.is_ascii_digit()));
        assert_eq!(MOONMOON_TWITCH_ID, "121059319");
    }
}
