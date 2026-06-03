// Suppressed until the fetch.rs / handler callers land in Tasks 7-10.
#![allow(dead_code)]
use crate::emotes::{EmoteProvider, EmoteRecord};
use std::collections::HashMap;

/// Parse 7TV's `/v3/users/twitch/{id}` response. Shape:
/// `{ "emote_set": { "emotes": [{ name, data: { host: { url }, owner: { ... } } }] } }`.
pub fn parse_seventv_user(json: &serde_json::Value) -> HashMap<String, EmoteRecord> {
    let items = json
        .get("emote_set")
        .and_then(|s| s.get("emotes"))
        .and_then(|e| e.as_array())
        .map(Vec::as_slice);
    parse_seventv_emote_list(items)
}

/// Parse 7TV's `/v3/emote-sets/global` response. Shape:
/// `{ "emotes": [...] }` — same item shape as the user endpoint.
pub fn parse_seventv_global(json: &serde_json::Value) -> HashMap<String, EmoteRecord> {
    parse_seventv_emote_list(
        json.get("emotes")
            .and_then(|e| e.as_array())
            .map(Vec::as_slice),
    )
}

fn parse_seventv_emote_list(items: Option<&[serde_json::Value]>) -> HashMap<String, EmoteRecord> {
    let mut out = HashMap::new();
    let Some(items) = items else { return out };
    for item in items {
        let Some(name) = item.get("name").and_then(|n| n.as_str()) else {
            continue;
        };
        let data = item.get("data");
        let host_url = data
            .and_then(|d| d.get("host"))
            .and_then(|h| h.get("url"))
            .and_then(|u| u.as_str());
        let Some(host_url) = host_url else { continue };
        let url = normalize_url(host_url) + "/1x.webp";
        let owner = data.and_then(|d| d.get("owner")).and_then(seventv_owner);
        out.insert(
            name.to_string(),
            EmoteRecord {
                url,
                provider: EmoteProvider::SevenTv,
                owner,
            },
        );
    }
    out
}

fn seventv_owner(owner: &serde_json::Value) -> Option<String> {
    if owner.is_null() {
        return None;
    }
    if let Some(conns) = owner.get("connections").and_then(|c| c.as_array()) {
        for c in conns {
            let platform = c.get("platform").and_then(|p| p.as_str());
            let name = c.get("display_name").and_then(|n| n.as_str());
            if platform == Some("TWITCH")
                && let Some(name) = name
            {
                return Some(name.to_string());
            }
        }
    }
    owner
        .get("display_name")
        .and_then(|n| n.as_str())
        .map(str::to_string)
}

fn normalize_url(url: &str) -> String {
    if let Some(rest) = url.strip_prefix("//") {
        format!("https://{rest}")
    } else if let Some(rest) = url.strip_prefix("http://") {
        format!("https://{rest}")
    } else {
        url.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load_fixture(path: &str) -> serde_json::Value {
        let raw = std::fs::read_to_string(path).expect("fixture exists");
        serde_json::from_str(&raw).expect("fixture is valid JSON")
    }

    #[test]
    fn seventv_user_parses_channel_emote_with_twitch_owner() {
        let json = load_fixture("tests/fixtures/emotes/7tv_channel.json");
        let map = parse_seventv_user(&json);
        let r = map.get("moon2A").expect("moon2A present");
        assert_eq!(r.provider, EmoteProvider::SevenTv);
        assert_eq!(
            r.url,
            "https://cdn.7tv.app/emote/01F6Q5BG1R000179HAZRRJVAY7/1x.webp"
        );
        assert_eq!(r.owner.as_deref(), Some("MOONMOON"));
    }

    #[test]
    fn seventv_user_skips_emotes_without_host_url() {
        let json = load_fixture("tests/fixtures/emotes/7tv_channel.json");
        let map = parse_seventv_user(&json);
        assert!(!map.contains_key("missing-host-field"));
    }

    #[test]
    fn seventv_global_parses_top_level_emotes_array() {
        let json = load_fixture("tests/fixtures/emotes/7tv_global.json");
        let map = parse_seventv_global(&json);
        let r = map.get("PauseChamp").expect("PauseChamp present");
        assert_eq!(r.owner, None);
        assert!(r.url.starts_with("https://"));
    }

    #[test]
    fn seventv_handles_missing_emote_set_field() {
        let json: serde_json::Value = serde_json::from_str("{}").unwrap();
        assert!(parse_seventv_user(&json).is_empty());
    }

    #[test]
    fn seventv_owner_falls_back_to_top_level_when_no_twitch_connection() {
        let owner: serde_json::Value = serde_json::from_str(
            r#"{ "display_name": "SomeCreator",
                 "connections": [{ "platform": "YOUTUBE", "display_name": "SomeYT" }] }"#,
        )
        .unwrap();
        assert_eq!(seventv_owner(&owner).as_deref(), Some("SomeCreator"));
    }
}
