use crate::api::graphql::{GraphqlClient, GraphqlRequestError};
use crate::api::rest_api_throttler::{RestClient, RestError, RestResponse};
use crate::error::{abort_error, bug_error, FatalError};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;

const THEME_KIT_ACCESS_DOMAIN: &str = "theme-kit-access.shopifyapps.com";

const PUBLIC_API_VERSIONS_QUERY: &str =
    "query publicApiVersions { publicApiVersions { handle supported } }";

/// A session bound to a specific store with its FQDN and auth token.
#[derive(Debug, Clone)]
pub struct AdminSession {
    pub store_fqdn: String,
    pub token: String,
}

/// Errors that can occur during Admin API operations.
#[derive(Debug)]
pub enum AdminError {
    /// Recoverable error with an optional try-message.
    Abort(String, Option<String>),
    /// Internal bug that should be reported.
    Bug(String),
}

impl std::fmt::Display for AdminError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AdminError::Abort(msg, _) => write!(f, "{msg}"),
            AdminError::Bug(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for AdminError {}

impl From<AdminError> for FatalError {
    fn from(e: AdminError) -> Self {
        match e {
            AdminError::Abort(msg, try_msg) => abort_error(msg, try_msg, vec![]),
            AdminError::Bug(msg) => bug_error(msg, None::<String>),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct ApiVersionsResponse {
    public_api_versions: Vec<ApiVersion>,
}

/// A single API version returned by the public API versions endpoint.
#[derive(Debug, Deserialize, Serialize)]
pub struct ApiVersion {
    pub handle: String,
    pub supported: bool,
}

/// Client for the Shopify Admin GraphQL and REST APIs.
///
/// Supports both normal store tokens and theme access tokens (`shptka_*`).
/// When using theme access, requests are routed through
/// `theme-kit-access.shopifyapps.com` with `x-shopify-shop` and
/// `x-shopify-access-token` headers injected automatically.
pub struct AdminClient {
    session: AdminSession,
    client: reqwest::Client,
    latest_version: Mutex<HashMap<String, String>>,
}

impl AdminClient {
    /// Create a new client from an [`AdminSession`].
    pub fn new(session: AdminSession) -> Self {
        let client = crate::http::build_client(None).expect("failed to build HTTP client");
        Self {
            session,
            client,
            latest_version: Mutex::new(HashMap::new()),
        }
    }

    /// Create a client with a pre-built `reqwest::Client`.
    pub fn with_client(session: AdminSession, client: reqwest::Client) -> Self {
        Self {
            session,
            client,
            latest_version: Mutex::new(HashMap::new()),
        }
    }

    /// Whether the session token is a theme access token (`shptka_*`).
    pub fn is_theme_access_session(&self) -> bool {
        self.session.token.starts_with("shptka_")
    }

    /// Access the underlying session.
    pub fn session(&self) -> &AdminSession {
        &self.session
    }

    fn admin_graphql_url(&self, version: &str) -> String {
        if self.is_theme_access_session() {
            format!("https://{THEME_KIT_ACCESS_DOMAIN}/cli/admin/api/{version}/graphql.json")
        } else {
            format!(
                "https://{}/admin/api/{version}/graphql.json",
                self.session.store_fqdn
            )
        }
    }

    fn admin_rest_base_url(&self, version: &str) -> String {
        if self.is_theme_access_session() {
            format!("https://{THEME_KIT_ACCESS_DOMAIN}/cli/admin/api/{version}")
        } else {
            format!("https://{}/admin/api/{version}", self.session.store_fqdn)
        }
    }

    fn graphql_client_for_version(&self, version: &str) -> GraphqlClient {
        let url = self.admin_graphql_url(version);
        let mut client =
            GraphqlClient::with_client(url, Some(self.session.token.clone()), self.client.clone());
        if self.is_theme_access_session() {
            let mut extra = HeaderMap::new();
            extra.insert(
                HeaderName::from_static("x-shopify-shop"),
                HeaderValue::from_str(&self.session.store_fqdn).unwrap(),
            );
            extra.insert(
                HeaderName::from_static("x-shopify-access-token"),
                HeaderValue::from_str(&self.session.token).unwrap(),
            );
            client = client.with_extra_headers(extra);
        }
        client
    }

    fn rest_client_for_version(&self, version: &str) -> RestClient {
        let base_url = self.admin_rest_base_url(version);
        let mut client =
            RestClient::with_client(base_url, self.session.token.clone(), self.client.clone());
        if self.is_theme_access_session() {
            let mut extra = HeaderMap::new();
            extra.insert(
                HeaderName::from_static("x-shopify-shop"),
                HeaderValue::from_str(&self.session.store_fqdn).unwrap(),
            );
            extra.insert(
                HeaderName::from_static("x-shopify-access-token"),
                HeaderValue::from_str(&self.session.token).unwrap(),
            );
            client = client.with_extra_headers(extra);
        }
        client
    }

    /// Fetch the latest supported API version, caching the result per store.
    pub async fn fetch_latest_api_version(&self) -> Result<String, AdminError> {
        {
            let cache = self.latest_version.lock().unwrap();
            if let Some(version) = cache.get(&self.session.store_fqdn) {
                return Ok(version.clone());
            }
        }

        let versions = self.fetch_api_versions().await?;
        let latest = versions
            .into_iter()
            .filter(|v| v.supported)
            .map(|v| v.handle)
            .next_back()
            .unwrap_or_else(|| "unstable".to_string());

        let mut cache = self.latest_version.lock().unwrap();
        cache.insert(self.session.store_fqdn.clone(), latest.clone());
        Ok(latest)
    }

    /// Fetch the list of public API versions from the store.
    pub async fn fetch_api_versions(&self) -> Result<Vec<ApiVersion>, AdminError> {
        let client = self.graphql_client_for_version("unstable");
        let result: Result<ApiVersionsResponse, GraphqlRequestError> =
            client.query(PUBLIC_API_VERSIONS_QUERY).await;

        match result {
            Ok(response) => Ok(response.public_api_versions),
            Err(err) => match &err {
                GraphqlRequestError::ApiError(_, 403) => {
                    let store_name = self.session.store_fqdn.replace(".myshopify.com", "");
                    Err(AdminError::Abort(
                        format!("Looks like you don't have access to this dev store: {store_name}"),
                        Some("If you're not the owner, create a dev store staff account for yourself".into()),
                    ))
                }
                GraphqlRequestError::ApiError(_, 401 | 404) => Err(AdminError::Abort(
                    format!(
                        "Error connecting to your store {}: {}",
                        self.session.store_fqdn, err
                    ),
                    None,
                )),
                GraphqlRequestError::Network(msg) => Err(AdminError::Abort(
                    format!(
                        "Network error connecting to your store {}: {msg}",
                        self.session.store_fqdn
                    ),
                    Some("Check your internet connection and try again.".into()),
                )),
                _ => Err(AdminError::Bug(format!(
                    "Unknown error connecting to your store {}: {err}",
                    self.session.store_fqdn
                ))),
            },
        }
    }

    /// Execute a GraphQL query against the Admin API, auto-resolving the
    /// latest API version.
    pub async fn query<T: DeserializeOwned + serde::Serialize>(
        &self,
        query: &str,
        variables: Option<serde_json::Value>,
    ) -> Result<T, GraphqlRequestError> {
        let version = self
            .fetch_latest_api_version()
            .await
            .map_err(|e| GraphqlRequestError::ApiError(e.to_string(), 0))?;
        let client = self.graphql_client_for_version(&version);
        client.query_with_variables(query, variables).await
    }

    /// Execute a REST request (GET, POST, PUT, DELETE) against the Admin API.
    pub async fn rest_request<T: DeserializeOwned>(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<serde_json::Value>,
        search_params: Option<HashMap<String, String>>,
        api_version: Option<&str>,
    ) -> Result<RestResponse<T>, RestError> {
        let version = api_version.unwrap_or("unstable");
        let client = self.rest_client_for_version(version);

        match method {
            reqwest::Method::GET => client.get(path, search_params).await,
            reqwest::Method::POST => {
                client
                    .post(path, body.unwrap_or(serde_json::Value::Null))
                    .await
            }
            reqwest::Method::PUT => {
                client
                    .put(path, body.unwrap_or(serde_json::Value::Null))
                    .await
            }
            reqwest::Method::DELETE => {
                let result: RestResponse<serde_json::Value> = client.delete(path).await?;
                let body: T = serde_json::from_value(result.body)
                    .map_err(|e| RestError::Parse(e.to_string()))?;
                Ok(RestResponse {
                    status: result.status,
                    headers: result.headers,
                    body,
                })
            }
            _ => Err(RestError::Network(format!("Unsupported method: {method}"))),
        }
    }

    /// Execute a GET request against the Admin REST API.
    pub async fn get<T: DeserializeOwned>(
        &self,
        path: &str,
        query: Option<HashMap<String, String>>,
    ) -> Result<RestResponse<T>, RestError> {
        self.rest_request(reqwest::Method::GET, path, None, query, None)
            .await
    }

    /// Execute a POST request against the Admin REST API.
    pub async fn post<T: DeserializeOwned>(
        &self,
        path: &str,
        body: serde_json::Value,
    ) -> Result<RestResponse<T>, RestError> {
        self.rest_request(reqwest::Method::POST, path, Some(body), None, None)
            .await
    }

    /// Execute a PUT request against the Admin REST API.
    pub async fn put<T: DeserializeOwned>(
        &self,
        path: &str,
        body: serde_json::Value,
    ) -> Result<RestResponse<T>, RestError> {
        self.rest_request(reqwest::Method::PUT, path, Some(body), None, None)
            .await
    }

    /// Execute a DELETE request against the Admin REST API.
    pub async fn delete(&self, path: &str) -> Result<RestResponse<serde_json::Value>, RestError> {
        self.rest_request(reqwest::Method::DELETE, path, None, None, None)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admin_graphql_url_built_correctly() {
        let session = AdminSession {
            store_fqdn: "test-store.myshopify.com".into(),
            token: "shpat_test".into(),
        };
        let client = AdminClient::new(session);
        assert_eq!(
            client.admin_graphql_url("2024-01"),
            "https://test-store.myshopify.com/admin/api/2024-01/graphql.json"
        );
    }

    #[test]
    fn theme_access_url_uses_theme_kit_domain() {
        let session = AdminSession {
            store_fqdn: "test-store.myshopify.com".into(),
            token: "shptka_test".into(),
        };
        let client = AdminClient::new(session);
        assert_eq!(
            client.admin_graphql_url("unstable"),
            "https://theme-kit-access.shopifyapps.com/cli/admin/api/unstable/graphql.json"
        );
    }

    #[test]
    fn theme_access_rest_url_uses_theme_kit_domain() {
        let session = AdminSession {
            store_fqdn: "test-store.myshopify.com".into(),
            token: "shptka_test".into(),
        };
        let client = AdminClient::new(session);
        assert_eq!(
            client.admin_rest_base_url("2024-01"),
            "https://theme-kit-access.shopifyapps.com/cli/admin/api/2024-01"
        );
    }

    #[test]
    fn is_theme_access_session_detects_shptka() {
        let session = AdminSession {
            store_fqdn: "store.myshopify.com".into(),
            token: "shptka_abc".into(),
        };
        let client = AdminClient::new(session);
        assert!(client.is_theme_access_session());
    }

    #[test]
    fn is_theme_access_session_false_for_shpat() {
        let session = AdminSession {
            store_fqdn: "store.myshopify.com".into(),
            token: "shpat_abc".into(),
        };
        let client = AdminClient::new(session);
        assert!(!client.is_theme_access_session());
    }

    #[test]
    fn is_theme_access_session_false_for_bearer() {
        let session = AdminSession {
            store_fqdn: "store.myshopify.com".into(),
            token: "abc123".into(),
        };
        let client = AdminClient::new(session);
        assert!(!client.is_theme_access_session());
    }

    #[test]
    fn version_cache_initially_empty() {
        let session = AdminSession {
            store_fqdn: "store.myshopify.com".into(),
            token: "shpat_test".into(),
        };
        let client = AdminClient::new(session);
        let cache = client.latest_version.lock().unwrap();
        assert!(cache.is_empty());
    }

    #[test]
    fn admin_error_to_fatal_abort() {
        let err = AdminError::Abort("test error".into(), Some("try this".into()));
        let fatal: FatalError = err.into();
        assert_eq!(fatal.message, "test error");
        assert_eq!(fatal.r#type, crate::error::FatalErrorType::Abort);
    }

    #[test]
    fn admin_error_to_fatal_bug() {
        let err = AdminError::Bug("bug".into());
        let fatal: FatalError = err.into();
        assert_eq!(fatal.r#type, crate::error::FatalErrorType::Bug);
    }
}
