use crate::api::graphql::{CacheOptions, GraphqlClient, GraphqlRequestError, UnauthorizedHandler};
use crate::api::rate_limiter::ApiRateLimiter;
use crate::api::utilities::add_cursor_and_filters_to_app_logs_url;
use crate::constants::app_management_fqdn;
use crate::http::build_headers;
use reqwest::header::HeaderMap;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

fn app_management_rate_limiter() -> ApiRateLimiter {
    static LIMITER: OnceLock<ApiRateLimiter> = OnceLock::new();
    LIMITER.get_or_init(ApiRateLimiter::shopify_default).clone()
}

/// Build the standard set of HTTP headers for App Management API requests.
pub fn app_management_headers(token: &str) -> HeaderMap {
    build_headers(Some(token))
}

/// Build the App Management app logs polling URL for an organization.
///
/// Combines the resolved FQDN, organization ID, and optional cursor/filter query
/// parameters into a complete polling endpoint URL.
pub fn app_management_app_logs_url(
    organization_id: &str,
    cursor: Option<&str>,
    filters: Option<HashMap<String, String>>,
) -> String {
    let base = format!(
        "https://{}/app_management/unstable/organizations/{organization_id}/app_logs/poll",
        app_management_fqdn(None),
    );
    add_cursor_and_filters_to_app_logs_url(&base, cursor, filters)
}

/// Client for the Shopify App Management GraphQL API.
///
/// Wraps [`GraphqlClient`] with App Management–specific rate limiting, URL
/// resolution, and deprecation tracking. Every request goes through the
/// shared [`app_management_rate_limiter`] (150 ms minimum interval).
pub struct AppManagementClient {
    /// The authentication token used for every request.
    pub token: String,
    /// Optional environment overrides (used for FQDN resolution).
    pub env: Option<HashMap<String, String>>,
}

impl AppManagementClient {
    /// Create a new client with the given auth token and optional env map.
    pub fn new(token: String, env: Option<HashMap<String, String>>) -> Self {
        Self { token, env }
    }

    /// Execute a GraphQL query against the App Management API.
    ///
    /// The request is rate-limited, automatically retried on transient errors,
    /// cached if `cache_options` is provided, and transparently re-authenticated
    /// if `unauthorized_handler` is supplied and a 401 response is received.
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
            "https://{}/app_management/unstable/graphql.json",
            app_management_fqdn(self.env.as_ref()),
        );
        let mut client = GraphqlClient::new(url, Some(self.token.clone()))
            .with_rate_limiter(app_management_rate_limiter());

        if let Some(opts) = cache_options {
            client = client.with_cache_options(opts);
        }
        if let Some(handler) = unauthorized_handler {
            client = client.with_token_refresh_handler(handler);
        }

        client.query_with_variables(query, variables).await
    }
}

/// A single deprecation entry returned in a GraphQL response's `extensions`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Deprecation {
    /// ISO-8601 date when the feature expires.
    pub expired_at: Option<String>,
    /// Upgrade path or notice tacked onto the deprecation.
    pub tacked_on: Option<String>,
    /// API path or field that is deprecated.
    pub path: Option<String>,
}

/// A GraphQL response wrapper that preserves deprecation metadata alongside
/// the decoded data payload.
///
/// Use this when the caller needs to inspect deprecations rather than
/// discarding [`crate::api::graphql::GraphqlResponse::extensions`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WithDeprecations<T> {
    /// The actual response data.
    pub data: T,
    /// Deprecation entries extracted from the response extensions.
    pub deprecations: Vec<Deprecation>,
}

/// Parse deprecation entries from a `serde_json::Value` representing the
/// `extensions` object of a GraphQL response.
///
/// Returns an empty vector when `extensions` is `None`, missing the
/// `deprecations` key, or when the value is not an array.
pub fn handle_deprecations(extensions: Option<&serde_json::Value>) -> Vec<Deprecation> {
    let Some(ext) = extensions else {
        return vec![];
    };
    let Some(deprecations) = ext.get("deprecations") else {
        return vec![];
    };
    let Some(arr) = deprecations.as_array() else {
        return vec![];
    };
    arr.iter()
        .filter_map(|d| serde_json::from_value(d.clone()).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headers_contains_auth() {
        let headers = app_management_headers("shpat_test");
        assert!(headers.get("authorization").is_some());
    }

    #[test]
    fn app_logs_url_has_org_id() {
        let url = app_management_app_logs_url("org-123", None, None);
        assert!(url.contains("org-123"));
    }

    #[test]
    fn app_logs_url_contains_fqdn() {
        let fqdn = app_management_fqdn(None);
        assert_eq!(fqdn, "app.shopify.com");
        let url = app_management_app_logs_url("org-1", None, None);
        assert!(url.contains(&fqdn));
    }

    #[test]
    fn app_logs_url_includes_cursor() {
        let url = app_management_app_logs_url("org-1", Some("cursor_abc"), None);
        assert!(url.contains("cursor_abc"));
    }

    #[test]
    fn app_logs_url_includes_filters() {
        let mut filters = HashMap::new();
        filters.insert("status".to_string(), "active".to_string());
        let url = app_management_app_logs_url("org-1", None, Some(filters));
        assert!(url.contains("status") || url.contains("active"));
    }

    #[test]
    fn client_new_sets_token() {
        let client = AppManagementClient::new("shpat_test".into(), None);
        assert_eq!(client.token, "shpat_test");
    }

    #[test]
    fn client_new_sets_env() {
        let mut env = HashMap::new();
        env.insert("SHOPIFY_SERVICE_ENV".to_string(), "production".to_string());
        let client = AppManagementClient::new("t".into(), Some(env.clone()));
        assert_eq!(client.env, Some(env));
    }

    #[test]
    fn client_new_env_none() {
        let client = AppManagementClient::new("t".into(), None);
        assert!(client.env.is_none());
    }

    #[test]
    fn deprecation_deserialize_all_fields() {
        let json = serde_json::json!({
            "expired_at": "2025-06-01",
            "tacked_on": "Use v2 instead",
            "path": "mutation.createApp",
        });
        let d: Deprecation = serde_json::from_value(json).unwrap();
        assert_eq!(d.expired_at.as_deref(), Some("2025-06-01"));
        assert_eq!(d.tacked_on.as_deref(), Some("Use v2 instead"));
        assert_eq!(d.path.as_deref(), Some("mutation.createApp"));
    }

    #[test]
    fn deprecation_deserialize_partial() {
        let json = serde_json::json!({"expired_at": "2025-06-01"});
        let d: Deprecation = serde_json::from_value(json).unwrap();
        assert_eq!(d.expired_at.as_deref(), Some("2025-06-01"));
        assert!(d.tacked_on.is_none());
        assert!(d.path.is_none());
    }

    #[test]
    fn handle_deprecations_returns_empty_for_none() {
        let result = handle_deprecations(None);
        assert!(result.is_empty());
    }

    #[test]
    fn handle_deprecations_returns_empty_for_no_deprecations_key() {
        let val = serde_json::json!({"cost": {"actualQueryCost": 1.0}});
        let result = handle_deprecations(Some(&val));
        assert!(result.is_empty());
    }

    #[test]
    fn handle_deprecations_parses_array() {
        let val = serde_json::json!({
            "deprecations": [
                {"expired_at": "2025-06-01", "path": "mutation.createApp"},
                {"expired_at": "2025-07-01", "path": "query.shop"},
            ]
        });
        let result = handle_deprecations(Some(&val));
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].path.as_deref(), Some("mutation.createApp"));
    }

    #[test]
    fn handle_deprecations_allows_entries_with_empty_fields() {
        let val = serde_json::json!({
            "deprecations": [
                {"expired_at": "2025-06-01"},
                {"invalid": true},
            ]
        });
        let result = handle_deprecations(Some(&val));
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn with_deprecations_struct() {
        let wd = WithDeprecations {
            data: 42u32,
            deprecations: vec![Deprecation {
                expired_at: None,
                tacked_on: None,
                path: Some("/test".into()),
            }],
        };
        assert_eq!(wd.data, 42);
        assert_eq!(wd.deprecations.len(), 1);
    }

    #[tokio::test]
    async fn rate_limiter_acquires_permit() {
        let limiter = app_management_rate_limiter();
        let permit = limiter.acquire().await;
        drop(permit);
    }

    #[tokio::test]
    async fn rate_limiter_static_returns_same() {
        let a = app_management_rate_limiter();
        let b = app_management_rate_limiter();
        let pa = a.acquire().await;
        let pb = b.acquire().await;
        drop(pa);
        drop(pb);
    }
}
