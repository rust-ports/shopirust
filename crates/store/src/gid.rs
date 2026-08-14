/// Trailing numeric id from a plain GID (`gid://shopify/Shop/123` → `123`).
pub fn numeric_id_from_gid(gid: &str) -> Option<String> {
    let (_, digits) = gid.rsplit_once('/')?;
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(digits.to_string())
}

/// Decode a standard-base64 GID and return the trailing numeric id.
pub fn numeric_id_from_encoded_gid(encoded: &str) -> Option<String> {
    use base64::Engine;
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .ok()?;
    let decoded_str = String::from_utf8(decoded).ok()?;
    numeric_id_from_gid(&decoded_str)
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;

    #[test]
    fn plain_gid() {
        assert_eq!(
            numeric_id_from_gid("gid://organization/Organization/123"),
            Some("123".into())
        );
    }

    #[test]
    fn encoded_gid() {
        let encoded = base64::engine::general_purpose::STANDARD
            .encode("gid://organization/Organization/123");
        assert_eq!(numeric_id_from_encoded_gid(&encoded), Some("123".into()));
    }
}
