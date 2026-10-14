//! Shared hashing, token, and time helpers.

use sha2::{Digest, Sha256};

/// Lowercase hex SHA-256 digest of `data`.
pub fn sha256_hex(data: &[u8]) -> String {
    let digest = Sha256::digest(data);
    hex::encode(digest)
}

/// Cryptographically random 32-byte token, hex encoded.
pub fn random_token() -> String {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).expect("OS randomness unavailable");
    hex::encode(bytes)
}

/// Constant-time byte equality.
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (left, right) in a.iter().zip(b.iter()) {
        diff |= left ^ right;
    }
    diff == 0
}

/// Current UTC time as an RFC 3339 string.
pub fn now_rfc3339() -> String {
    format_rfc3339(time::OffsetDateTime::now_utc())
}

/// RFC 3339 string for `now + hours`.
pub fn rfc3339_plus_hours(hours: i64) -> String {
    let now = time::OffsetDateTime::now_utc();
    let later = now.checked_add(time::Duration::hours(hours)).unwrap_or(now);
    format_rfc3339(later)
}

/// Parses an RFC 3339 timestamp, returning None on malformed input.
pub fn parse_rfc3339(value: &str) -> Option<time::OffsetDateTime> {
    time::OffsetDateTime::parse(value, &time::format_description::well_known::Rfc3339).ok()
}

fn format_rfc3339(value: time::OffsetDateTime) -> String {
    value
        .format(
            &time::format_description::parse_borrowed::<2>(
                "[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:9]Z",
            )
            .expect("timestamp format"),
        )
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_is_stable() {
        assert_eq!(
            sha256_hex(b"voidreg"),
            "cf9e6ffb81b1f1eb30f76433b383e076b8af76b07c64a2478dfd4225d3506150"
        );
    }

    #[test]
    fn tokens_are_unique_and_64_chars() {
        let a = random_token();
        let b = random_token();
        assert_eq!(a.len(), 64);
        assert_ne!(a, b);
    }

    #[test]
    fn constant_time_eq_basic() {
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"abc", b"abcd"));
    }

    #[test]
    fn round_trips_timestamps() {
        let now = now_rfc3339();
        let parsed = parse_rfc3339(&now).expect("parses");
        assert!(parse_rfc3339("not-a-time").is_none());
        let later = rfc3339_plus_hours(1);
        assert!(parse_rfc3339(&later).unwrap() > parsed);
    }
}
