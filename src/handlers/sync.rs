//! Sync endpoints. Tokens are base32 (RFC 4648 alphabet) so they survive
//! double-click selection, copy-paste between browsers, and QR codes.

// Removed in Task 7 once main.rs registers the routes.
#![allow(dead_code)]

const TOKEN_MIN_LEN: usize = 26;
const TOKEN_MAX_LEN: usize = 32;

/// Accept only uppercase base32: `A-Z` and `2-7`, length 26..=32.
/// 26 = ceil(16 bytes * 8 / 5). Upper bound leaves slack for clients that
/// emit padding or longer tokens later.
pub(crate) fn is_valid_token(token: &str) -> bool {
    let len = token.len();
    if !(TOKEN_MIN_LEN..=TOKEN_MAX_LEN).contains(&len) {
        return false;
    }
    token
        .chars()
        .all(|c| c.is_ascii_uppercase() || ('2'..='7').contains(&c))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_26_char_base32() {
        assert!(is_valid_token("ABCDEFGHIJKLMNOPQRSTUVWXYZ"));
        assert!(is_valid_token("AAAAAAAAAAAAAAAAAAAAAAAA23"));
    }

    #[test]
    fn accepts_up_to_32_chars() {
        assert!(is_valid_token("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAA23"));
    }

    #[test]
    fn rejects_too_short() {
        assert!(!is_valid_token(""));
        assert!(!is_valid_token("ABC"));
        assert!(!is_valid_token(&"A".repeat(25)));
    }

    #[test]
    fn rejects_too_long() {
        assert!(!is_valid_token(&"A".repeat(33)));
    }

    #[test]
    fn rejects_lowercase() {
        assert!(!is_valid_token("abcdefghijklmnopqrstuvwxyz"));
    }

    #[test]
    fn rejects_invalid_base32_digits() {
        // base32 doesn't include 0, 1, 8, or 9
        assert!(!is_valid_token("AAAAAAAAAAAAAAAAAAAAAAAAA0"));
        assert!(!is_valid_token("AAAAAAAAAAAAAAAAAAAAAAAAA1"));
        assert!(!is_valid_token("AAAAAAAAAAAAAAAAAAAAAAAAA8"));
        assert!(!is_valid_token("AAAAAAAAAAAAAAAAAAAAAAAAA9"));
    }

    #[test]
    fn rejects_punctuation_and_path_traversal() {
        assert!(!is_valid_token("../etc/passwd"));
        assert!(!is_valid_token("AAAAAAAAAAAAAAAAAAAAAAAAA-"));
        assert!(!is_valid_token("AAAAAAAAAAAAAAAAAAAAAAAAA "));
    }
}
