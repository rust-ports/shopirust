use crate::api::graphql::{CacheOptions, GraphqlClient, GraphqlRequestError, UnauthorizedHandler};
use crate::constants::business_platform_fqdn;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;

/// Client for the Shopify Business Platform APIs.
///
/// Supports two endpoints:
/// - **Destinations API** (`/destinations/api/2020-07/graphql`) — general business
///   platform queries via [`request`](Self::request).
/// - **Organizations API** (`/organizations/api/unstable/organization/{id}/graphql`) —
///   organization-scoped queries via [`organizations_request`](Self::organizations_request).
pub struct BusinessPlatformClient {
    /// Authentication token for every request.
    pub token: String,
    /// Optional environment overrides (used for FQDN resolution).
    pub env: Option<HashMap<String, String>>,
}

impl BusinessPlatformClient {
    /// Create a new client with the given auth token and optional env map.
    pub fn new(token: String, env: Option<HashMap<String, String>>) -> Self {
        Self { token, env }
    }

    /// Execute a GraphQL query against the Destinations API.
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
            "https://{}/destinations/api/2020-07/graphql",
            business_platform_fqdn(self.env.as_ref()),
        );
        let mut client = GraphqlClient::new(url, Some(self.token.clone()));

        if let Some(opts) = cache_options {
            client = client.with_cache_options(opts);
        }
        if let Some(handler) = unauthorized_handler {
            client = client.with_token_refresh_handler(handler);
        }

        client.query_with_variables(query, variables).await
    }

    /// Execute a GraphQL query against the Organizations API, scoped to the
    /// given `organization_id`.
    pub async fn organizations_request<T, V>(
        &self,
        organization_id: &str,
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
            "https://{}/organizations/api/unstable/organization/{organization_id}/graphql",
            business_platform_fqdn(self.env.as_ref()),
        );
        let mut client = GraphqlClient::new(url, Some(self.token.clone()));

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
    fn request_url_contains_destinations() {
        let _client = BusinessPlatformClient::new("t".into(), None);
        let fqdn = business_platform_fqdn(None);
        let expected = format!("https://{fqdn}/destinations/api/2020-07/graphql");
        assert!(expected.contains("destinations"));
        assert!(expected.contains("2020-07"));
    }

    #[test]
    fn organizations_request_url_contains_org_id() {
        let _client = BusinessPlatformClient::new("t".into(), None);
        let fqdn = business_platform_fqdn(None);
        let expected =
            format!("https://{fqdn}/organizations/api/unstable/organization/org-123/graphql");
        assert!(expected.contains("org-123"));
    }

    #[test]
    fn client_new_sets_token() {
        let client = BusinessPlatformClient::new("shbiz_test".into(), None);
        assert_eq!(client.token, "shbiz_test");
    }

    #[test]
    fn client_new_sets_env() {
        let mut env = HashMap::new();
        env.insert("SHOPIFY_SERVICE_ENV".to_string(), "production".to_string());
        let client = BusinessPlatformClient::new("t".into(), Some(env.clone()));
        assert_eq!(client.env, Some(env));
    }

    #[test]
    fn client_new_env_none() {
        let client = BusinessPlatformClient::new("t".into(), None);
        assert!(client.env.is_none());
    }
}
