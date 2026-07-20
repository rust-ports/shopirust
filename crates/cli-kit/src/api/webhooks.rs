use crate::api::graphql::{CacheOptions, GraphqlClient, GraphqlRequestError, UnauthorizedHandler};
use crate::api::rate_limiter::ApiRateLimiter;
use crate::constants::app_management_fqdn;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

fn webhooks_rate_limiter() -> ApiRateLimiter {
    static LIMITER: OnceLock<ApiRateLimiter> = OnceLock::new();
    LIMITER.get_or_init(ApiRateLimiter::shopify_default).clone()
}

/// Client for the Shopify App Management Webhooks API.
///
/// Wraps [`GraphqlClient`] with Webhooks-specific URL resolution and rate
/// limiting (150 ms minimum interval, 10 max concurrent). Every request is
/// scoped to an organization.
pub struct WebhooksClient {
    /// Organization identifier.
    pub organization_id: String,
    /// Authentication token.
    pub token: String,
    /// Optional environment overrides (used for FQDN resolution).
    pub env: Option<HashMap<String, String>>,
}

impl WebhooksClient {
    /// Create a new client with the given organization, token, and env.
    pub fn new(
        organization_id: String,
        token: String,
        env: Option<HashMap<String, String>>,
    ) -> Self {
        Self {
            organization_id,
            token,
            env,
        }
    }

    /// Execute a rate-limited GraphQL query against the Webhooks API.
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
        let url = format!(
            "https://{}/webhooks/unstable/organizations/{}/graphql.json",
            app_management_fqdn(self.env.as_ref()),
            self.organization_id,
        );
        let mut client = GraphqlClient::new(url, Some(self.token.clone()))
            .with_rate_limiter(webhooks_rate_limiter());

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
    fn request_url_contains_org_id() {
        let _client = WebhooksClient::new("org-42".into(), "t".into(), None);
        let fqdn = app_management_fqdn(None);
        let expected =
            format!("https://{fqdn}/webhooks/unstable/organizations/org-42/graphql.json");
        assert!(expected.contains("org-42"));
    }

    #[test]
    fn client_new_sets_fields() {
        let client = WebhooksClient::new("org-7".into(), "shpat_test".into(), None);
        assert_eq!(client.organization_id, "org-7");
        assert_eq!(client.token, "shpat_test");
    }

    #[test]
    fn client_new_sets_env() {
        let mut env = HashMap::new();
        env.insert("SHOPIFY_SERVICE_ENV".to_string(), "production".to_string());
        let client = WebhooksClient::new("o".into(), "t".into(), Some(env.clone()));
        assert_eq!(client.env, Some(env));
    }

    #[test]
    fn client_new_env_none() {
        let client = WebhooksClient::new("o".into(), "t".into(), None);
        assert!(client.env.is_none());
    }

    #[tokio::test]
    async fn rate_limiter_acquires_permit() {
        let limiter = webhooks_rate_limiter();
        let permit = limiter.acquire().await;
        drop(permit);
    }
}
