pub mod fetch;
pub mod parse;
pub mod store;

use serde::{Deserialize, Serialize};

pub use fetch::load_prefetched;
pub use store::EmoteIndex;

// Consumed by the lookup handler in Task 10.
#[allow(unused_imports)]
pub use store::{Lookup, ResolvedEntry};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum EmoteProvider {
    #[serde(rename = "7TV")]
    SevenTv,
    Bttv,
    Ffz,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmoteRecord {
    pub url: String,
    pub provider: EmoteProvider,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_serializes_as_canonical_string() {
        assert_eq!(
            serde_json::to_string(&EmoteProvider::SevenTv).unwrap(),
            "\"7TV\""
        );
        assert_eq!(
            serde_json::to_string(&EmoteProvider::Bttv).unwrap(),
            "\"BTTV\""
        );
        assert_eq!(
            serde_json::to_string(&EmoteProvider::Ffz).unwrap(),
            "\"FFZ\""
        );
    }

    #[test]
    fn record_round_trips_through_json() {
        let r = EmoteRecord {
            url: "https://cdn.7tv.app/emote/abc/1x.webp".into(),
            provider: EmoteProvider::SevenTv,
            owner: Some("MOONMOON".into()),
        };
        let json = serde_json::to_string(&r).unwrap();
        let parsed: EmoteRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, r);
    }

    #[test]
    fn record_omits_null_owner() {
        let r = EmoteRecord {
            url: "https://x/1".into(),
            provider: EmoteProvider::Bttv,
            owner: None,
        };
        let json = serde_json::to_string(&r).unwrap();
        assert!(
            !json.contains("owner"),
            "expected null owner skipped, got: {json}"
        );
    }
}
