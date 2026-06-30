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

/// Parse BTTV's `/3/cached/users/twitch/{id}` response. Shape:
/// `{ "channelEmotes": [...], "sharedEmotes": [...] }`.
pub fn parse_bttv_user(json: &serde_json::Value) -> HashMap<String, EmoteRecord> {
    let mut out = HashMap::new();
    for key in ["channelEmotes", "sharedEmotes"] {
        if let Some(items) = json.get(key).and_then(|v| v.as_array()) {
            absorb_bttv_items(&mut out, items);
        }
    }
    out
}

/// Parse BTTV's `/3/cached/emotes/global` response. Top-level JSON array.
pub fn parse_bttv_global(json: &serde_json::Value) -> HashMap<String, EmoteRecord> {
    let mut out = HashMap::new();
    if let Some(items) = json.as_array() {
        absorb_bttv_items(&mut out, items);
    }
    out
}

fn absorb_bttv_items(out: &mut HashMap<String, EmoteRecord>, items: &[serde_json::Value]) {
    for item in items {
        let id = item.get("id").and_then(|v| v.as_str());
        let code = item.get("code").and_then(|v| v.as_str());
        let (Some(id), Some(code)) = (id, code) else {
            continue;
        };
        let owner = item
            .get("user")
            .and_then(|u| u.get("displayName"))
            .and_then(|n| n.as_str())
            .map(str::to_string);
        out.entry(code.to_string()).or_insert(EmoteRecord {
            url: format!("https://cdn.betterttv.net/emote/{id}/1x"),
            provider: EmoteProvider::Bttv,
            owner,
        });
    }
}

/// Parse FFZ's `/v1/room/id/{id}` and `/v1/set/global` — same shape:
/// `{ "sets": { "<setid>": { "emoticons": [...] } } }`.
pub fn parse_ffz(json: &serde_json::Value) -> HashMap<String, EmoteRecord> {
    let mut out = HashMap::new();
    let Some(sets) = json.get("sets").and_then(|s| s.as_object()) else {
        return out;
    };
    for set in sets.values() {
        let Some(items) = set.get("emoticons").and_then(|e| e.as_array()) else {
            continue;
        };
        for item in items {
            let Some(name) = item.get("name").and_then(|n| n.as_str()) else {
                continue;
            };
            let url = pick_ffz_url(item.get("urls"));
            let Some(url) = url else { continue };
            let owner = item
                .get("owner")
                .and_then(|o| o.get("display_name"))
                .and_then(|n| n.as_str())
                .map(str::to_string);
            out.entry(name.to_string()).or_insert(EmoteRecord {
                url,
                provider: EmoteProvider::Ffz,
                owner,
            });
        }
    }
    out
}

fn pick_ffz_url(urls: Option<&serde_json::Value>) -> Option<String> {
    let urls = urls?.as_object()?;
    for key in ["1", "2", "4"] {
        if let Some(u) = urls.get(key).and_then(|v| v.as_str()) {
            return Some(normalize_url(u));
        }
    }
    None
}

/// Parse 7TV's GraphQL `SearchEmotes` response and return the record only when
/// the result name matches `name` byte-for-byte. Provider search endpoints
/// return prefix/substring matches; we only render on an exact match.
pub fn parse_seventv_search(json: &serde_json::Value, name: &str) -> Option<EmoteRecord> {
    let items = json.get("data")?.get("emotes")?.get("items")?.as_array()?;
    for item in items {
        if item.get("name").and_then(|n| n.as_str()) != Some(name) {
            continue;
        }
        let host_url = item
            .get("host")
            .and_then(|h| h.get("url"))
            .and_then(|u| u.as_str())?;
        let owner = item.get("owner").and_then(seventv_owner);
        return Some(EmoteRecord {
            url: normalize_url(host_url) + "/1x.webp",
            provider: EmoteProvider::SevenTv,
            owner,
        });
    }
    None
}

/// Parse the archive's per-VOD emote snapshot (the `data` object from
/// `/vods/{id}/emotes`). Entries are minimal `{id, code}` (7TV also carries
/// `flags`/`width`/`height`, which we ignore). URLs are built directly from the
/// id per provider; the snapshot carries no owner, so `owner` is always None.
/// 7TV is absorbed first, so it wins any cross-provider name collision.
#[allow(dead_code)]
pub fn parse_vod_emote_snapshot(data: &serde_json::Value) -> HashMap<String, EmoteRecord> {
    let mut out = HashMap::new();
    absorb_snapshot(&mut out, data.get("seventv_emotes"), EmoteProvider::SevenTv);
    absorb_snapshot(&mut out, data.get("bttv_emotes"), EmoteProvider::Bttv);
    absorb_snapshot(&mut out, data.get("ffz_emotes"), EmoteProvider::Ffz);
    out
}

fn absorb_snapshot(
    out: &mut HashMap<String, EmoteRecord>,
    arr: Option<&serde_json::Value>,
    provider: EmoteProvider,
) {
    let Some(items) = arr.and_then(|v| v.as_array()) else {
        return;
    };
    for item in items {
        let id = item.get("id").and_then(|v| v.as_str());
        let name = item
            .get("code")
            .or_else(|| item.get("name"))
            .and_then(|v| v.as_str());
        let (Some(id), Some(name)) = (id, name) else {
            continue;
        };
        let url = match provider {
            EmoteProvider::SevenTv => format!("https://cdn.7tv.app/emote/{id}/1x.webp"),
            EmoteProvider::Bttv => format!("https://cdn.betterttv.net/emote/{id}/1x"),
            EmoteProvider::Ffz => format!("https://cdn.frankerfacez.com/emote/{id}/1"),
        };
        out.entry(name.to_string()).or_insert(EmoteRecord {
            url,
            provider,
            owner: None,
        });
    }
}

/// Parse BTTV's `/3/emotes/shared/search?query=...` response (top-level JSON array).
pub fn parse_bttv_search(json: &serde_json::Value, name: &str) -> Option<EmoteRecord> {
    let items = json.as_array()?;
    for item in items {
        if item.get("code").and_then(|c| c.as_str()) != Some(name) {
            continue;
        }
        let id = item.get("id").and_then(|v| v.as_str())?;
        let owner = item
            .get("user")
            .and_then(|u| u.get("displayName"))
            .and_then(|n| n.as_str())
            .map(str::to_string);
        return Some(EmoteRecord {
            url: format!("https://cdn.betterttv.net/emote/{id}/1x"),
            provider: EmoteProvider::Bttv,
            owner,
        });
    }
    None
}

/// Parse FFZ's `/v1/emotes?q=...` response. Shape: `{ "emoticons": [...] }`.
pub fn parse_ffz_search(json: &serde_json::Value, name: &str) -> Option<EmoteRecord> {
    let items = json.get("emoticons")?.as_array()?;
    for item in items {
        if item.get("name").and_then(|n| n.as_str()) != Some(name) {
            continue;
        }
        let url = pick_ffz_url(item.get("urls"))?;
        let owner = item
            .get("owner")
            .and_then(|o| o.get("display_name"))
            .and_then(|n| n.as_str())
            .map(str::to_string);
        return Some(EmoteRecord {
            url,
            provider: EmoteProvider::Ffz,
            owner,
        });
    }
    None
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

    #[test]
    fn bttv_user_parses_channel_and_shared() {
        let json = load_fixture("tests/fixtures/emotes/bttv_channel.json");
        let map = parse_bttv_user(&json);
        assert_eq!(
            map.get("catJAM").unwrap().url,
            "https://cdn.betterttv.net/emote/5f1b0186cf6d2144653d2970/1x"
        );
        assert_eq!(
            map.get("Pepega").unwrap().owner.as_deref(),
            Some("OmegaPepega")
        );
    }

    #[test]
    fn bttv_global_parses_top_level_array() {
        let json = load_fixture("tests/fixtures/emotes/bttv_global.json");
        let map = parse_bttv_global(&json);
        assert!(map.contains_key(":tf:"));
        assert!(map.contains_key("CiGrip"));
        assert_eq!(map.get("CiGrip").unwrap().owner, None);
    }

    #[test]
    fn ffz_channel_parses_room_set() {
        let json = load_fixture("tests/fixtures/emotes/ffz_channel.json");
        let map = parse_ffz(&json);
        let r = map.get("ZreknarF").unwrap();
        assert_eq!(r.url, "https://cdn.frankerfacez.com/emote/28138/1");
        assert_eq!(r.owner.as_deref(), Some("Zreknarf"));
    }

    #[test]
    fn ffz_global_passes_absolute_https_url_through() {
        let json = load_fixture("tests/fixtures/emotes/ffz_global.json");
        let map = parse_ffz(&json);
        assert_eq!(
            map.get("ZreknarP").unwrap().url,
            "https://cdn.frankerfacez.com/emote/28136/1"
        );
    }

    #[test]
    fn ffz_handles_missing_sets() {
        let json: serde_json::Value = serde_json::from_str("{}").unwrap();
        assert!(parse_ffz(&json).is_empty());
    }

    #[test]
    fn seventv_search_returns_hit_on_exact_case_sensitive_match() {
        let json: serde_json::Value = serde_json::from_str(
            r#"{ "data": { "emotes": { "items": [
                { "name": "TANIMURA",
                  "host": { "url": "//cdn.7tv.app/emote/abc" },
                  "owner": { "display_name": "tanimuraXYZ",
                    "connections": [{ "platform": "TWITCH", "display_name": "TanimuraTV" }] } }
            ]}}}"#,
        )
        .unwrap();
        let r = parse_seventv_search(&json, "TANIMURA").unwrap();
        assert_eq!(r.url, "https://cdn.7tv.app/emote/abc/1x.webp");
        assert_eq!(r.owner.as_deref(), Some("TanimuraTV"));
    }

    #[test]
    fn seventv_search_returns_none_on_case_mismatch() {
        let json: serde_json::Value = serde_json::from_str(
            r#"{ "data": { "emotes": { "items": [
                { "name": "tanimura", "host": { "url": "//x" } }
            ]}}}"#,
        )
        .unwrap();
        assert!(parse_seventv_search(&json, "TANIMURA").is_none());
    }

    #[test]
    fn bttv_search_returns_hit_on_exact_code_match() {
        let json: serde_json::Value = serde_json::from_str(
            r#"[
                { "id": "5f1b0186cf6d2144653d2970", "code": "catJAM",
                  "user": { "displayName": "OmegaPepega" } },
                { "id": "other", "code": "catjam" }
            ]"#,
        )
        .unwrap();
        let r = parse_bttv_search(&json, "catJAM").unwrap();
        assert_eq!(
            r.url,
            "https://cdn.betterttv.net/emote/5f1b0186cf6d2144653d2970/1x"
        );
        assert_eq!(r.owner.as_deref(), Some("OmegaPepega"));
    }

    #[test]
    fn bttv_search_returns_none_on_case_mismatch() {
        let json: serde_json::Value =
            serde_json::from_str(r#"[{ "id": "1", "code": "catjam" }]"#).unwrap();
        assert!(parse_bttv_search(&json, "catJAM").is_none());
    }

    #[test]
    fn ffz_search_returns_hit_on_exact_match() {
        let json: serde_json::Value = serde_json::from_str(
            r#"{ "emoticons": [
                { "id": 28138, "name": "ZreknarF",
                  "urls": { "1": "//cdn.frankerfacez.com/emote/28138/1" },
                  "owner": { "display_name": "Zreknarf" } }
            ]}"#,
        )
        .unwrap();
        let r = parse_ffz_search(&json, "ZreknarF").unwrap();
        assert_eq!(r.url, "https://cdn.frankerfacez.com/emote/28138/1");
        assert_eq!(r.owner.as_deref(), Some("Zreknarf"));
    }

    #[test]
    fn ffz_search_returns_none_when_no_exact_match() {
        let json: serde_json::Value = serde_json::from_str(
            r#"{ "emoticons": [{ "id": 1, "name": "zreknarf", "urls": {"1":"//x"} }] }"#,
        )
        .unwrap();
        assert!(parse_ffz_search(&json, "ZreknarF").is_none());
    }

    #[test]
    fn vod_snapshot_parses_each_provider_with_direct_cdn_urls() {
        let json = load_fixture("tests/fixtures/emotes/vod_snapshot.json");
        let map = parse_vod_emote_snapshot(&json["data"]);

        let frog = map.get("FROG4").expect("FROG4 present");
        assert_eq!(frog.provider, EmoteProvider::SevenTv);
        assert_eq!(
            frog.url,
            "https://cdn.7tv.app/emote/01KG6N0PJSDP7GPCB542CCB979/1x.webp"
        );
        assert_eq!(frog.owner, None);

        let soulful = map.get("SOULFUL").expect("SOULFUL present");
        assert_eq!(soulful.provider, EmoteProvider::Bttv);
        assert_eq!(
            soulful.url,
            "https://cdn.betterttv.net/emote/566ca04265dbbdab32ec054a/1x"
        );
    }

    #[test]
    fn vod_snapshot_7tv_wins_name_collision() {
        let json = load_fixture("tests/fixtures/emotes/vod_snapshot.json");
        let map = parse_vod_emote_snapshot(&json["data"]);
        assert_eq!(map.get(":tf:").unwrap().provider, EmoteProvider::SevenTv);
    }

    #[test]
    fn vod_snapshot_skips_entries_missing_id_or_name() {
        let json = load_fixture("tests/fixtures/emotes/vod_snapshot.json");
        let map = parse_vod_emote_snapshot(&json["data"]);
        assert!(!map.values().any(|r| r.url.contains("missingname7tv")));
    }

    #[test]
    fn vod_snapshot_handles_missing_arrays() {
        let empty: serde_json::Value = serde_json::from_str("{}").unwrap();
        assert!(parse_vod_emote_snapshot(&empty).is_empty());
    }
}
