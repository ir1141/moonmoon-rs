use crate::emotes::EmoteRecord;
use crate::emotes::parse;
use std::collections::HashMap;
use std::time::Duration;

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

/// Fetch + merge all 6 endpoints (3 providers × {channel, global}) into one
/// map. Individual provider failures are logged and skipped — we'd rather
/// return a partial map than fail the whole boot.
pub async fn load_prefetched(client: &reqwest::Client) -> HashMap<String, EmoteRecord> {
    let user_id = MOONMOON_TWITCH_ID;
    #[allow(clippy::type_complexity)]
    let endpoints: Vec<(
        String,
        fn(&serde_json::Value) -> HashMap<String, EmoteRecord>,
    )> = vec![
        (
            "https://7tv.io/v3/emote-sets/global".to_string(),
            parse::parse_seventv_global,
        ),
        (
            format!("https://7tv.io/v3/users/twitch/{user_id}"),
            parse::parse_seventv_user,
        ),
        (
            "https://api.betterttv.net/3/cached/emotes/global".to_string(),
            parse::parse_bttv_global,
        ),
        (
            format!("https://api.betterttv.net/3/cached/users/twitch/{user_id}"),
            parse::parse_bttv_user,
        ),
        (
            "https://api.frankerfacez.com/v1/set/global".to_string(),
            parse::parse_ffz,
        ),
        (
            format!("https://api.frankerfacez.com/v1/room/id/{user_id}"),
            parse::parse_ffz,
        ),
    ];

    let mut out = HashMap::new();
    for (url, parser) in endpoints {
        match fetch_json(client, &url).await {
            Ok(json) => {
                let map = parser(&json);
                tracing::info!("emotes: {} returned {} entries", url, map.len());
                for (k, v) in map {
                    out.entry(k).or_insert(v);
                }
            }
            Err(e) => {
                tracing::warn!("emotes: failed to load {url}: {e}");
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moonmoon_twitch_id_is_numeric_string() {
        assert!(MOONMOON_TWITCH_ID.chars().all(|c| c.is_ascii_digit()));
        assert_eq!(MOONMOON_TWITCH_ID, "121059319");
    }
}
