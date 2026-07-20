use crate::api::graphql::{CacheOptions, GraphqlClient, GraphqlRequestError, UnauthorizedHandler};
use crate::api::rate_limiter::ApiRateLimiter;
use crate::constants::{
    app_dev_fqdn, normalize_store_fqdn, service_environment, ServiceEnvironment,
};
use reqwest::header::HeaderMap;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

fn app_dev_rate_limiter() -> ApiRateLimiter {
    static LIMITER: OnceLock<ApiRateLimiter> = OnceLock::new();
    LIMITER.get_or_init(ApiRateLimiter::shopify_default).clone()
}

/// Client for the Shopify App Dev GraphQL API.
///
/// Wraps [`GraphqlClient`] with App Dev–specific URL resolution, rate
/// limiting, and header injection for local dev environments. The FQDN is
/// resolved via [`app_dev_fqdn`] which maps to the store domain in
/// production and to the App Management FQDN in local dev.
pub struct AppDevClient {
    /// The shop's FQDN (e.g. `test-store.myshopify.com`).
    pub shop_fqdn: String,
    /// Authentication token.
    pub token: String,
    /// Optional environment overrides (used for FQDN resolution).
    pub env: Option<HashMap<String, String>>,
}

impl AppDevClient {
    /// Create a new client with the given shop FQDN, token, and env.
    ///
    /// The `shop_fqdn` is normalized via [`normalize_store_fqdn`] so callers
    /// may pass raw store names, `https://` URLs, or full FQDNs.
    pub fn new(shop_fqdn: String, token: String, env: Option<HashMap<String, String>>) -> Self {
        Self {
            shop_fqdn,
            token,
            env,
        }
    }

    /// Execute a rate-limited GraphQL query against the App Dev API.
    ///
    /// In local dev environments a `x-forwarded-host` header is injected
    /// with the normalized shop domain.
    pub async fn request<T, V>(
        &self,
        query: &str,
        variables: Option<V>,
        cache_options: Option<CacheOptions>,
        unauthorized_handler: Option<Arc<dyn UnauthorizedHandler>>,
    ) -> Result<T, GraphqlRequestError>
    where
        T: DeserializeOwned + Serialize,
        V: Serialize,
    {
        let normalized = normalize_store_fqdn(&self.shop_fqdn);
        let fqdn = app_dev_fqdn(&normalized, self.env.as_ref());
        let url = format!("https://{fqdn}/app_dev/unstable/graphql.json");

        let mut headers = HeaderMap::new();
        if service_environment(self.env.as_ref()) == ServiceEnvironment::Local {
            headers.insert(
                "x-forwarded-host",
                normalized.parse().expect("valid header value"),
            );
        }

        let mut client = GraphqlClient::new(url, Some(self.token.clone()))
            .with_rate_limiter(app_dev_rate_limiter())
            .with_extra_headers(headers);

        if let Some(opts) = cache_options {
            client = client.with_cache_options(opts);
        }
        if let Some(handler) = unauthorized_handler {
            client = client.with_token_refresh_handler(handler);
        }

        client.query_with_variables(query, variables).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_url_uses_normalized_fqdn() {
        let _client = AppDevClient::new("test-store".into(), "t".into(), None);
        let normalized = normalize_store_fqdn("test-store");
        let fqdn = app_dev_fqdn(&normalized, None);
        let expected = format!("https://{fqdn}/app_dev/unstable/graphql.json");
        assert!(expected.contains(&normalized));
    }

    #[test]
    fn client_new_sets_fields() {
        let client = AppDevClient::new("my-shop.myshopify.com".into(), "shpat_test".into(), None);
        assert_eq!(client.shop_fqdn, "my-shop.myshopify.com");
        assert_eq!(client.token, "shpat_test");
    }

    #[test]
    fn client_new_sets_env() {
        let mut env = HashMap::new();
        env.insert("SHOPIFY_SERVICE_ENV".to_string(), "production".to_string());
        let client = AppDevClient::new("s".into(), "t".into(), Some(env.clone()));
        assert_eq!(client.env, Some(env));
    }

    #[test]
    fn client_new_env_none() {
        let client = AppDevClient::new("s".into(), "t".into(), None);
        assert!(client.env.is_none());
    }

    #[tokio::test]
    async fn rate_limiter_acquires_permit() {
        let limiter = app_dev_rate_limiter();
        let permit = limiter.acquire().await;
        drop(permit);
    }
}
