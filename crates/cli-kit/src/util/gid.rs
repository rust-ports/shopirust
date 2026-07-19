use std::num::ParseIntError;

/// GID prefix for Shopify resources.
const GID_PREFIX: &str = "gid://shopify/";

/// Regex-like parsing: `gid://shopify/{Type}/{NumericId}`
fn split_gid(gid: &str) -> Option<(&str, u64)> {
    let rest = gid.strip_prefix(GID_PREFIX)?;
    let slash_pos = rest.find('/')?;
    let type_name = &rest[..slash_pos];
    let id_str = &rest[slash_pos + 1..];
    let id: u64 = id_str.parse().ok()?;
    Some((type_name, id))
}

/// Extract the numeric ID from a GID string.
///
/// # Errors
/// Returns an error if the GID format is invalid or the ID is not a valid u64.
pub fn parse_gid(gid: &str) -> Result<u64, GidError> {
    split_gid(gid)
        .map(|(_, id)| id)
        .ok_or_else(|| GidError::InvalidFormat(gid.to_string()))
}

/// Extract the type name from a GID string.
///
/// # Errors
/// Returns an error if the GID format is invalid.
pub fn gid_to_type(gid: &str) -> Result<String, GidError> {
    split_gid(gid)
        .map(|(type_name, _)| type_name.to_string())
        .ok_or_else(|| GidError::InvalidFormat(gid.to_string()))
}

/// Create a GID string for a given type and numeric ID.
pub fn compose_gid(type_name: &str, id: u64) -> String {
    format!("{GID_PREFIX}{type_name}/{id}")
}

/// Create a GID for an OnlineStoreTheme.
pub fn compose_theme_gid(id: u64) -> String {
    compose_gid("OnlineStoreTheme", id)
}

/// Create a GID for a Shop.
pub fn compose_shop_gid(id: u64) -> String {
    compose_gid("Shop", id)
}

/// Check if a string is a valid GID.
pub fn is_gid(s: &str) -> bool {
    split_gid(s).is_some()
}

/// Errors that can occur during GID operations.
#[derive(Debug, thiserror::Error)]
pub enum GidError {
    #[error("Invalid GID format: {0}")]
    InvalidFormat(String),
    #[error("Failed to parse GID ID as integer: {0}")]
    ParseInt(#[from] ParseIntError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_gid_theme() {
        assert_eq!(parse_gid("gid://shopify/OnlineStoreTheme/123456").unwrap(), 123456);
    }

    #[test]
    fn test_parse_gid_shop() {
        assert_eq!(parse_gid("gid://shopify/Shop/789").unwrap(), 789);
    }

    #[test]
    fn test_parse_gid_invalid_format() {
        assert!(parse_gid("not-a-gid").is_err());
    }

    #[test]
    fn test_parse_gid_missing_id() {
        assert!(parse_gid("gid://shopify/OnlineStoreTheme/").is_err());
    }

    #[test]
    fn test_parse_gid_non_numeric_id() {
        assert!(parse_gid("gid://shopify/OnlineStoreTheme/abc").is_err());
    }

    #[test]
    fn test_gid_to_type_theme() {
        assert_eq!(
            gid_to_type("gid://shopify/OnlineStoreTheme/123").unwrap(),
            "OnlineStoreTheme"
        );
    }

    #[test]
    fn test_gid_to_type_shop() {
        assert_eq!(
            gid_to_type("gid://shopify/Shop/456").unwrap(),
            "Shop"
        );
    }

    #[test]
    fn test_compose_gid() {
        assert_eq!(
            compose_gid("OnlineStoreTheme", 123),
            "gid://shopify/OnlineStoreTheme/123"
        );
    }

    #[test]
    fn test_compose_theme_gid() {
        assert_eq!(
            compose_theme_gid(123),
            "gid://shopify/OnlineStoreTheme/123"
        );
    }

    #[test]
    fn test_compose_shop_gid() {
        assert_eq!(
            compose_shop_gid(456),
            "gid://shopify/Shop/456"
        );
    }

    #[test]
    fn test_is_gid_true() {
        assert!(is_gid("gid://shopify/OnlineStoreTheme/123"));
    }

    #[test]
    fn test_is_gid_false() {
        assert!(!is_gid("not-a-gid"));
        assert!(!is_gid(""));
    }

    #[test]
    fn test_parse_gid_large_number() {
        assert_eq!(
            parse_gid("gid://shopify/Product/9223372036854775807").unwrap(),
            9223372036854775807
        );
    }

    #[test]
    fn test_parse_gid_roundtrip() {
        let id = 4242u64;
        let gid = compose_theme_gid(id);
        assert_eq!(parse_gid(&gid).unwrap(), id);
    }
}
