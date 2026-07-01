use reqwest::header::{self, HeaderMap, HeaderValue};
use std::time::Duration;

const CLI_KIT_VERSION: &str = "0.1.0";
const DEFAULT_TIMEOUT_MS: u64 = 30_000;

pub fn build_headers(token: Option<&str>) -> HeaderMap {
    let mut headers = HeaderMap::new();

    headers.insert(
        header::USER_AGENT,
        HeaderValue::from_str(&format!("Shopify CLI; v={CLI_KIT_VERSION}")).unwrap(),
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-cache"),
    );
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );

    if let Some(token) = token {
        let auth_str = if token.starts_with("shpat")
            || token.starts_with("shpua")
            || token.starts_with("shpca")
            || token.starts_with("shptka")
        {
            token.to_string()
        } else {
            format!("Bearer {token}")
        };
        headers.insert(header::AUTHORIZATION, HeaderValue::from_str(&auth_str).unwrap());
        headers.insert(
            header::HeaderName::from_static("x-shopify-access-token"),
            HeaderValue::from_str(&auth_str).unwrap(),
        );
    }

    headers
}

pub fn build_client(timeout_ms: Option<u64>) -> reqwest::Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(Duration::from_millis(timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS)))
        .pool_idle_timeout(Duration::from_secs(30))
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_headers_without_token() {
        let headers = build_headers(None);
        assert_eq!(
            headers.get(header::USER_AGENT).unwrap(),
            "Shopify CLI; v=0.1.0"
        );
        assert_eq!(
            headers.get(header::CACHE_CONTROL).unwrap(),
            "no-cache"
        );
        assert_eq!(
            headers.get(header::CONTENT_TYPE).unwrap(),
            "application/json"
        );
        assert!(headers.get(header::AUTHORIZATION).is_none());
        assert!(headers.get("x-shopify-access-token").is_none());
    }

    #[test]
    fn build_headers_with_bearer_token() {
        let headers = build_headers(Some("abc123"));
        assert_eq!(
            headers.get(header::AUTHORIZATION).unwrap(),
            "Bearer abc123"
        );
        assert_eq!(
            headers.get("x-shopify-access-token").unwrap(),
            "Bearer abc123"
        );
    }

    #[test]
    fn build_headers_with_shpat_token() {
        let headers = build_headers(Some("shpat_abc123"));
        assert_eq!(
            headers.get(header::AUTHORIZATION).unwrap(),
            "shpat_abc123"
        );
        assert_eq!(
            headers.get("x-shopify-access-token").unwrap(),
            "shpat_abc123"
        );
    }

    #[test]
    fn build_headers_with_shpua_token() {
        let headers = build_headers(Some("shpua_abc123"));
        assert_eq!(headers.get(header::AUTHORIZATION).unwrap(), "shpua_abc123");
    }

    #[test]
    fn build_headers_with_shpca_token() {
        let headers = build_headers(Some("shpca_abc123"));
        assert_eq!(headers.get(header::AUTHORIZATION).unwrap(), "shpca_abc123");
    }

    #[test]
    fn build_headers_with_shptka_token() {
        let headers = build_headers(Some("shptka_abc123"));
        assert_eq!(headers.get(header::AUTHORIZATION).unwrap(), "shptka_abc123");
    }

    #[test]
    fn build_client_default_timeout() {
        let client = build_client(None).unwrap();
        let client_inner: &reqwest::Client = &client;
        assert!(std::mem::size_of_val(client_inner) > 0); // just ensure it builds
    }

    #[test]
    fn build_client_custom_timeout() {
        let client = build_client(Some(5000)).unwrap();
        assert!(std::mem::size_of_val(&client) > 0);
    }
}
