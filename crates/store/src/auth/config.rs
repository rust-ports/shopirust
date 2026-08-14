pub const STORE_AUTH_APP_CLIENT_ID: &str = "7e9cb568cfd431c538f36d1ad3f2b4f6";
pub const DEFAULT_STORE_AUTH_PORT: u16 = 13387;
pub const STORE_AUTH_CALLBACK_PATH: &str = "/auth/callback";

pub fn store_auth_redirect_uri(port: u16) -> String {
    format!("http://127.0.0.1:{port}{STORE_AUTH_CALLBACK_PATH}")
}

pub fn escape_store_auth_session_key_segment(value: &str) -> String {
    value.replace('.', "\\.")
}

pub fn unescape_store_auth_session_key_segment(value: &str) -> String {
    value.replace("\\.", ".")
}

pub fn store_auth_session_key(store: &str) -> String {
    format!(
        "{STORE_AUTH_APP_CLIENT_ID}::{}",
        escape_store_auth_session_key_segment(store)
    )
}

/// True when `segment` is the escaped form of a store FQDN (rejects legacy nested keys).
pub fn is_escaped_store_key_segment(segment: &str) -> bool {
    let unescaped = unescape_store_auth_session_key_segment(segment);
    escape_store_auth_session_key_segment(&unescaped) == segment
}

pub fn mask_token(token: &str) -> String {
    if token.len() <= 10 {
        return "***".into();
    }
    format!("{}***", &token[..10])
}

pub fn normalize_store_fqdn(store: &str) -> String {
    let cleaned = store
        .trim()
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/')
        .trim_end_matches("/admin");
    if cleaned.contains("myshopify.com")
        || cleaned.contains("myshopify.io")
        || cleaned.contains("shop.dev")
        || cleaned.contains("shopify.com")
    {
        return cleaned.to_string();
    }
    format!("{cleaned}.myshopify.com")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redirect_uri() {
        assert_eq!(
            store_auth_redirect_uri(13387),
            "http://127.0.0.1:13387/auth/callback"
        );
    }

    #[test]
    fn session_key_escapes_dots() {
        assert_eq!(
            store_auth_session_key("shop.myshopify.com"),
            format!("{STORE_AUTH_APP_CLIENT_ID}::shop\\.myshopify\\.com")
        );
    }

    #[test]
    fn escaped_segment_rejects_legacy_unescaped() {
        assert!(is_escaped_store_key_segment("shop\\.myshopify\\.com"));
        assert!(!is_escaped_store_key_segment("legacy.myshopify.com"));
    }

    #[test]
    fn mask_short_and_long() {
        assert_eq!(mask_token("short"), "***");
        assert_eq!(mask_token("abcdefghijklmnop"), "abcdefghij***");
    }

    #[test]
    fn normalize_fqdn() {
        assert_eq!(
            normalize_store_fqdn("https://shop.myshopify.com/admin"),
            "shop.myshopify.com"
        );
        assert_eq!(normalize_store_fqdn("shop"), "shop.myshopify.com");
    }
}
