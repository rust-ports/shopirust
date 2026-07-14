use crate::api::graphql::{GraphqlClient, GraphqlRequestError};
use crate::api::rate_limiter::ApiRateLimiter;
use serde::de::DeserializeOwned;
use std::sync::OnceLock;

pub fn normalize_store_fqdn(store: &str) -> String {
    let cleaned = store
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/')
        .trim_end_matches("/admin");

    if cleaned.ends_with(".myshopify.com")
        || cleaned.ends_with("shopify.io")
        || cleaned.ends_with(".shop.dev")
    {
        cleaned.to_string()
    } else {
        format!("{cleaned}.myshopify.com")
    }
}

fn app_dev_rate_limiter() -> ApiRateLimiter {
    static LIMITER: OnceLock<ApiRateLimiter> = OnceLock::new();
    LIMITER.get_or_init(ApiRateLimiter::shopify_default).clone()
}

pub async fn app_dev_request<T: DeserializeOwned + serde::Serialize>(
    shop_fqdn: &str,
    token: &str,
    query: &str,
    variables: Option<serde_json::Value>,
) -> Result<T, GraphqlRequestError> {
    let fqdn = normalize_store_fqdn(shop_fqdn);
    let url = format!("https://{fqdn}/app_dev/unstable/graphql.json");
    let client =
        GraphqlClient::new(url, Some(token.into())).with_rate_limiter(app_dev_rate_limiter());
    client.query_with_variables(query, variables).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_adds_myshopify_domain() {
        assert_eq!(
            normalize_store_fqdn("test-store"),
            "test-store.myshopify.com"
        );
    }

    #[test]
    fn normalize_preserves_existing_domain() {
        assert_eq!(
            normalize_store_fqdn("test-store.myshopify.com"),
            "test-store.myshopify.com"
        );
    }

    #[test]
    fn normalize_removes_https_prefix() {
        assert_eq!(
            normalize_store_fqdn("https://test-store.myshopify.com"),
            "test-store.myshopify.com"
        );
    }

    #[test]
    fn normalize_removes_admin_path() {
        assert_eq!(
            normalize_store_fqdn("test-store.myshopify.com/admin"),
            "test-store.myshopify.com"
        );
    }

    #[test]
    fn normalize_handles_shopify_io() {
        assert_eq!(
            normalize_store_fqdn("test-store.shopify.io"),
            "test-store.shopify.io"
        );
    }
}
