use std::num::ParseIntError;

/// Prefix shared by all Shopify GID URIs.
const GID_PREFIX: &str = "gid://shopify/";

/// Internal parser: split `gid://shopify/{Type}/{NumericId}` into its parts.
///
/// Returns `None` for any malformed input (wrong prefix, missing type, missing
/// slash, or non-numeric ID).
fn split_gid(gid: &str) -> Option<(&str, u64)> {
    let rest = gid.strip_prefix(GID_PREFIX)?;
    let slash_pos = rest.find('/')?;
    let type_name = &rest[..slash_pos];
    let id_str = &rest[slash_pos + 1..];
    let id: u64 = id_str.parse().ok()?;
    Some((type_name, id))
}

/// Extract the numeric ID from a Shopify GID URI.
///
/// For example, `gid://shopify/Product/123456` returns `123456`.
///
/// # Errors
/// Returns [`GidError::InvalidFormat`] when the input is not a valid GID.
pub fn parse_gid(gid: &str) -> Result<u64, GidError> {
    split_gid(gid)
        .map(|(_, id)| id)
        .ok_or_else(|| GidError::InvalidFormat(gid.to_string()))
}

/// Extract the resource type name from a Shopify GID URI.
///
/// For example, `gid://shopify/Product/123456` returns `"Product"`.
///
/// # Errors
/// Returns [`GidError::InvalidFormat`] when the input is not a valid GID.
pub fn gid_to_type(gid: &str) -> Result<String, GidError> {
    split_gid(gid)
        .map(|(type_name, _)| type_name.to_string())
        .ok_or_else(|| GidError::InvalidFormat(gid.to_string()))
}

/// Build a GID string from a resource type name and numeric ID.
pub fn compose_gid(type_name: &str, id: u64) -> String {
    format!("{GID_PREFIX}{type_name}/{id}")
}

/// Shorthand for [`compose_gid`] with `"OnlineStoreTheme"` as the type.
pub fn compose_theme_gid(id: u64) -> String {
    compose_gid("OnlineStoreTheme", id)
}

/// Shorthand for [`compose_gid`] with `"Shop"` as the type.
pub fn compose_shop_gid(id: u64) -> String {
    compose_gid("Shop", id)
}

/// Check whether a string is a syntactically valid Shopify GID.
///
/// This is equivalent to `parse_gid(gid).is_ok()` but avoids the `Result` overhead.
pub fn is_gid(s: &str) -> bool {
    split_gid(s).is_some()
}

/// Errors produced by GID parsing and composition.
#[derive(Debug, thiserror::Error)]
pub enum GidError {
    /// The input string does not match the `gid://shopify/{Type}/{Id}` pattern.
    #[error("Invalid GID format: {0}")]
    InvalidFormat(String),
    /// The ID portion of the GID could not be parsed as an integer.
    #[error("Failed to parse GID ID as integer: {0}")]
    ParseInt(#[from] ParseIntError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_gid_theme() {
        assert_eq!(
            parse_gid("gid://shopify/OnlineStoreTheme/123456").unwrap(),
            123456
        );
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
        assert_eq!(gid_to_type("gid://shopify/Shop/456").unwrap(), "Shop");
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
        assert_eq!(compose_theme_gid(123), "gid://shopify/OnlineStoreTheme/123");
    }

    #[test]
    fn test_compose_shop_gid() {
        assert_eq!(compose_shop_gid(456), "gid://shopify/Shop/456");
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
