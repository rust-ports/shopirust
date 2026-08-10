use crate::api::rate_limiter::ApiRateLimiter;
use crate::error::{abort_error, FatalError};
use crate::http::{build_client, build_headers};
use crate::util::cache::CacheStore;
use crate::util::conf_store::{composite_cache_key, LocalStorage};
use crate::util::retry::{is_transient_network_error, RetryAction, RetryConfig};
use reqwest::header::HeaderMap;
use reqwest::StatusCode;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::Value;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Maximum fraction of a second to wait after a rate-limited query.
const MAX_RATE_LIMIT_RESTORE_SECONDS: f64 = 0.3;

/// Current CLI kit version, embedded in cache keys for cache invalidation
/// across releases.
const CLI_KIT_VERSION: &str = "3.94.3";

/// Standard GraphQL response envelope.
#[derive(Debug, Deserialize)]
pub struct GraphqlResponse<T> {
    pub data: Option<T>,
    pub errors: Option<Vec<GraphqlError>>,
    pub extensions: Option<GraphqlExtensions>,
}

/// A single error item in the `errors` array.
#[derive(Debug, Deserialize)]
pub struct GraphqlError {
    pub message: Option<String>,
    pub extensions: Option<GraphqlErrorExtensions>,
}

/// Per-error extensions containing a machine-readable code and (optionally)
/// user-facing app errors.
#[derive(Debug, Deserialize)]
pub struct GraphqlErrorExtensions {
    pub code: Option<Value>,
    #[serde(rename = "requiredAccess")]
    pub required_access: Option<Value>,
    pub app_errors: Option<GraphqlAppErrors>,
}

/// Container for application-level errors reported by the API.
#[derive(Debug, Deserialize)]
pub struct GraphqlAppErrors {
    pub errors: Option<Vec<GraphqlAppError>>,
}

/// A single application-level error.
#[derive(Debug, Deserialize)]
pub struct GraphqlAppError {
    pub message: Option<String>,
    pub category: Option<String>,
}

/// Rate-limit cost/throttle information returned in extensions.
#[derive(Debug, Deserialize)]
pub struct GraphqlExtensions {
    pub cost: Option<GraphqlCost>,
}

/// Actual and throttled cost of a query.
#[derive(Debug, Deserialize)]
pub struct GraphqlCost {
    pub actual_query_cost: Option<f64>,
    pub throttle_status: Option<GraphqlThrottleStatus>,
}

/// Throttle bucket state for the current client.
#[derive(Debug, Deserialize)]
pub struct GraphqlThrottleStatus {
    pub restore_rate: Option<f64>,
    pub currently_available: Option<f64>,
    pub maximum_available: Option<f64>,
}

/// Errors that can occur during a GraphQL operation.
#[derive(Debug)]
pub enum GraphqlRequestError {
    /// Transport-level failure (connection refused, DNS, TLS, timeout).
    Network(String),
    /// The API returned an error (HTTP status + message).
    ApiError(String, u16),
    /// The response body could not be parsed as the expected type.
    Parse(String, String),
}

impl std::fmt::Display for GraphqlRequestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GraphqlRequestError::Network(msg) => write!(f, "Network error: {msg}"),
            GraphqlRequestError::ApiError(msg, status) => {
                write!(f, "GraphQL API error (HTTP {status}): {msg}")
            }
            GraphqlRequestError::Parse(msg, _) => write!(f, "Parse error: {msg}"),
        }
    }
}

impl std::error::Error for GraphqlRequestError {}

impl From<GraphqlRequestError> for FatalError {
    fn from(e: GraphqlRequestError) -> Self {
        match e {
            GraphqlRequestError::ApiError(msg, _)
            | GraphqlRequestError::Network(msg)
            | GraphqlRequestError::Parse(msg, _) => abort_error(msg, None::<String>, vec![]),
        }
    }
}

/// Hook called when a 401 (Unauthorized) response is received.
///
/// Implementors should refresh the session token and return the new value.
/// The returned token replaces the stored token and the request is retried
/// automatically.
pub trait UnauthorizedHandler: Send + Sync {
    fn refresh_token(&self) -> Option<String>;
}

impl<F> UnauthorizedHandler for F
where
    F: Fn() -> Option<String> + Send + Sync,
{
    fn refresh_token(&self) -> Option<String> {
        self()
    }
}

/// Cache configuration for GraphQL queries.
///
/// Mirrors the upstream `CacheOptions` type. When both `CacheStore` and
/// `LocalStorage` are configured the client checks both on read and
/// writes to both on write.
#[derive(Debug, Clone)]
pub struct CacheOptions {
    pub cache_ttl_ms: u64,
    pub cache_extra_key: Option<String>,
    pub cache_store: Option<Arc<LocalStorage>>,
}

impl Default for CacheOptions {
    fn default() -> Self {
        Self {
            cache_ttl_ms: 60_000,
            cache_extra_key: None,
            cache_store: None,
        }
    }
}

/// Reusable GraphQL API client with retry, caching, rate limiting, and
/// automatic token refresh.
///
/// ## Features
/// - Exponential-backoff retry for transient errors and 5xx responses
/// - Composite cache keys via [`composite_cache_key`] (deterministic UUID v5)
/// - Dual cache backends: [`CacheStore`] (the legacy file-based store) and
///   [`LocalStorage`] (the new conf-store)
/// - 401 interception with [`UnauthorizedHandler`] for transparent token
///   refresh
/// - GraphQL cost-based rate-limit back-off (``wait_for_rate_limit_restore``)
/// - Optional [`ApiRateLimiter`] for concurrency control
#[derive(Clone)]
pub struct GraphqlClient {
    client: reqwest::Client,
    pub url: String,
    pub token: Option<String>,
    retry_config: RetryConfig,
    cache: Option<Arc<CacheStore>>,
    extra_headers: Option<HeaderMap>,
    rate_limiter: Option<ApiRateLimiter>,
    token_refresh_handler: Option<Arc<dyn UnauthorizedHandler>>,
    cache_options: Option<CacheOptions>,
}

impl GraphqlClient {
    /// Create a new GraphQL client for the given endpoint URL.
    ///
    /// The client is built with [`build_client`] (default timeout, no TLS
    /// enforcement). Use [`with_client`](Self::with_client) if a custom
    /// `reqwest::Client` is required.
    pub fn new(url: impl Into<String>, token: Option<String>) -> Self {
        let client = build_client(None).expect("failed to build HTTP client");
        Self {
            client,
            url: url.into(),
            token,
            retry_config: RetryConfig::new(),
            cache: None,
            extra_headers: None,
            rate_limiter: None,
            token_refresh_handler: None,
            cache_options: None,
        }
    }

    /// Create a client with a pre-built `reqwest::Client` (useful when
    /// custom TLS settings or timeouts are needed).
    pub fn with_client(
        url: impl Into<String>,
        token: Option<String>,
        client: reqwest::Client,
    ) -> Self {
        Self {
            client,
            url: url.into(),
            token,
            retry_config: RetryConfig::new(),
            cache: None,
            extra_headers: None,
            rate_limiter: None,
            token_refresh_handler: None,
            cache_options: None,
        }
    }

    /// Set the retry configuration (backoff, max time, etc.).
    pub fn with_retry(mut self, config: RetryConfig) -> Self {
        self.retry_config = config;
        self
    }

    /// Enable response caching via the legacy [`CacheStore`].
    pub fn with_cache(mut self, cache: Arc<CacheStore>) -> Self {
        self.cache = Some(cache);
        self
    }

    /// Set caching options (TTL, extra key component, optional
    /// [`LocalStorage`] backend).
    pub fn with_cache_options(mut self, options: CacheOptions) -> Self {
        self.cache_options = Some(options);
        self
    }

    /// Attach an [`UnauthorizedHandler`] for automatic token refresh on 401.
    pub fn with_token_refresh_handler(mut self, handler: Arc<dyn UnauthorizedHandler>) -> Self {
        self.token_refresh_handler = Some(handler);
        self
    }

    /// Attach extra HTTP headers to every request.
    pub fn with_extra_headers(mut self, headers: HeaderMap) -> Self {
        self.extra_headers = Some(headers);
        self
    }

    /// Attach a rate limiter for concurrency control.
    pub fn with_rate_limiter(mut self, limiter: ApiRateLimiter) -> Self {
        self.rate_limiter = Some(limiter);
        self
    }

    /// Execute a query without variables (delegates to
    /// [`query_with_variables`](Self::query_with_variables)).
    pub async fn query<T: DeserializeOwned>(&self, query: &str) -> Result<T, GraphqlRequestError> {
        self.query_with_variables::<T, Value>(query, None).await
    }

    /// Execute a GraphQL query with optional variables.
    ///
    /// ## Flow
    /// 1. Compute a composite cache key from `query`, `variables`,
    ///    `CLI_KIT_VERSION`, and (optionally) `cache_extra_key`.
    /// 2. Check both [`CacheStore`] and [`LocalStorage`] for a cached
    ///    response.
    /// 3. Build request headers from the stored token (or from the
    ///    refresh handler if the token was updated on a previous 401).
    /// 4. Send the request through the retry loop:
    ///    - 401 → call [`UnauthorizedHandler::refresh_token`] and retry
    ///    - 429 → parse `Retry-After` header and retry
    ///    - 5xx → retry (upstream retry config decides max time)
    ///    - 4xx → fail immediately
    ///    - 2xx → parse, apply rate-limit back-off, return
    /// 5. On success, write the result to both cache backends.
    pub async fn query_with_variables<T: DeserializeOwned, V: serde::Serialize>(
        &self,
        query: &str,
        variables: Option<V>,
    ) -> Result<T, GraphqlRequestError> {
        let extra_key = self
            .cache_options
            .as_ref()
            .and_then(|o| o.cache_extra_key.as_deref())
            .unwrap_or("");
        let variables_json = serde_json::to_string(&variables).unwrap_or_default();
        let composite_key = if extra_key.is_empty() {
            composite_cache_key(&[query, &variables_json, CLI_KIT_VERSION])
        } else {
            composite_cache_key(&[query, &variables_json, CLI_KIT_VERSION, extra_key])
        };

        if let Some(ref cache) = self.cache {
            if let Ok(Some(cached)) = cache.retrieve::<String>(&composite_key) {
                if let Ok(val) = serde_json::from_str(&cached) {
                    return Ok(val);
                }
            }
        }
        if let Some(ref opts) = self.cache_options {
            if let Some(ref ls) = opts.cache_store {
                if let Some(cached) = ls.get::<String>(&composite_key) {
                    if let Ok(val) = serde_json::from_str(&cached) {
                        return Ok(val);
                    }
                }
            }
        }

        let body = serde_json::json!({"query": query, "variables": variables});
        let mut headers = build_headers(self.token.as_deref());
        if let Some(ref extra) = self.extra_headers {
            for (key, val) in extra.iter() {
                headers.insert(key, val.clone());
            }
        }
        let url = self.url.clone();
        let client = self.client.clone();
        let cache = self.cache.clone();
        let cache_options = self.cache_options.clone();
        let composite_key_for_write = composite_key.clone();
        let rate_limiter = self.rate_limiter.clone();
        let token_refresh = self.token_refresh_handler.clone();
        let token_mutex = Arc::new(Mutex::new(self.token.clone()));

        let result: Result<T, GraphqlRequestError> = self
            .retry_config
            .execute(move || {
                let client = client.clone();
                let url = url.clone();
                let mut headers = headers.clone();
                let body = body.clone();
                let rate_limiter = rate_limiter.clone();
                let token_refresh = token_refresh.clone();
                let token_mutex = token_mutex.clone();
                let cache = cache.clone();
                let cache_options = cache_options.clone();
                let composite_key = composite_key_for_write.clone();

                async move {
                    if let Some(ref limiter) = rate_limiter {
                        limiter.acquire().await;
                    }

                    // Read the current token (possibly refreshed by a
                    // previous retry iteration).
                    let current_token = token_mutex.lock().unwrap().clone();

                    if let Some(ref token) = current_token {
                        let auth_val = if token.starts_with("shpat")
                            || token.starts_with("shpua")
                            || token.starts_with("shpca")
                            || token.starts_with("shptka")
                        {
                            token.clone()
                        } else {
                            format!("Bearer {token}")
                        };
                        headers.insert(
                            reqwest::header::AUTHORIZATION,
                            reqwest::header::HeaderValue::from_str(&auth_val).unwrap(),
                        );
                        headers.insert(
                            reqwest::header::HeaderName::from_static("x-shopify-access-token"),
                            reqwest::header::HeaderValue::from_str(&auth_val).unwrap(),
                        );
                    }

                    let response_result = client
                        .post(&url)
                        .headers(headers.clone())
                        .json(&body)
                        .send()
                        .await;

                    let response = match response_result {
                        Ok(r) => r,
                        Err(e) if is_transient_network_error(&e.to_string()) => {
                            return RetryAction::Retry(GraphqlRequestError::Network(e.to_string()));
                        }
                        Err(e) => {
                            return RetryAction::Err(GraphqlRequestError::Network(e.to_string()));
                        }
                    };

                    let status = response.status();

                    // 401 → refresh token and retry
                    if status == StatusCode::UNAUTHORIZED {
                        if let Some(ref handler) = token_refresh {
                            if let Some(new_token) = handler.refresh_token() {
                                *token_mutex.lock().unwrap() = Some(new_token);
                                return RetryAction::Retry(GraphqlRequestError::ApiError(
                                    "token_refreshed".into(),
                                    401,
                                ));
                            }
                        }
                        let text = response.text().await.unwrap_or_default();
                        return RetryAction::Err(GraphqlRequestError::ApiError(
                            format!("Unauthorized: {text}"),
                            401,
                        ));
                    }

                    // 429 → parse Retry-After and wait
                    if status == StatusCode::TOO_MANY_REQUESTS {
                        let retry_after = response
                            .headers()
                            .get("retry-after")
                            .and_then(|v| v.to_str().ok())
                            .and_then(|v| v.parse::<u64>().ok())
                            .unwrap_or(1000);
                        tokio::time::sleep(Duration::from_millis(retry_after)).await;
                        return RetryAction::Retry(GraphqlRequestError::ApiError(
                            "rate limited".into(),
                            status.as_u16(),
                        ));
                    }

                    let text = match response.text().await {
                        Ok(t) => t,
                        Err(e) => {
                            return RetryAction::Err(GraphqlRequestError::Network(e.to_string()));
                        }
                    };

                    let full_response: GraphqlResponse<Value> = match serde_json::from_str(&text) {
                        Ok(r) => r,
                        Err(e) => {
                            return RetryAction::Err(GraphqlRequestError::Parse(
                                e.to_string(),
                                text.clone(),
                            ));
                        }
                    };

                    if let Some(errors) = &full_response.errors {
                        if full_response.data.is_none() && status.is_server_error() {
                            let msg = format!(
                                "The {} GraphQL API responded with HTTP status {}: {}",
                                "shopify",
                                status.as_u16(),
                                extract_error_messages(errors),
                            );
                            return RetryAction::Err(GraphqlRequestError::ApiError(
                                msg,
                                status.as_u16(),
                            ));
                        }
                        if full_response.data.is_none() && !status.is_server_error() {
                            let msg = extract_error_messages(errors);
                            return RetryAction::Err(GraphqlRequestError::ApiError(
                                msg,
                                status.as_u16(),
                            ));
                        }
                    }

                    if let Some(data) = full_response.data {
                        if status.is_success() {
                            if let Some(cost) = full_response.extensions.and_then(|e| e.cost) {
                                wait_for_rate_limit_restore(&cost).await;
                            }
                            let raw_data = serde_json::to_string(&data).ok();
                            let result: T = match serde_json::from_value(data) {
                                Ok(val) => val,
                                Err(e) => {
                                    return RetryAction::Err(GraphqlRequestError::Parse(
                                        e.to_string(),
                                        text,
                                    ));
                                }
                            };
                            if let Some(json_str) = raw_data {
                                if let Some(ref cache) = cache {
                                    let _ = cache.store(&composite_key, &json_str);
                                }
                                if let Some(ref opts) = cache_options {
                                    if let Some(ref ls) = opts.cache_store {
                                        ls.set(&composite_key, &json_str);
                                    }
                                }
                            }
                            return RetryAction::Ok(result);
                        }
                    }

                    if status.is_client_error() {
                        return RetryAction::Err(GraphqlRequestError::ApiError(
                            extract_error_messages(&full_response.errors.unwrap_or_default()),
                            status.as_u16(),
                        ));
                    }

                    let msg = format!(
                        "GraphQL request failed with status {}: {}",
                        status.as_u16(),
                        extract_error_messages(&full_response.errors.unwrap_or_default()),
                    );
                    RetryAction::Err(GraphqlRequestError::ApiError(msg, status.as_u16()))
                }
            })
            .await;

        result
    }
}

/// Sleep for a short duration proportional to the GraphQL cost's
/// `actualQueryCost / restoreRate`, capped at
/// [`MAX_RATE_LIMIT_RESTORE_SECONDS`].
///
/// This lets the API's rate-limit bucket recover before the next query,
/// reducing 429 responses.
async fn wait_for_rate_limit_restore(cost: &GraphqlCost) {
    if let (Some(actual), Some(restore_rate)) = (
        cost.actual_query_cost,
        cost.throttle_status.as_ref().and_then(|t| t.restore_rate),
    ) {
        if actual > 0.0 && restore_rate > 0.0 {
            let seconds = (actual / restore_rate).min(MAX_RATE_LIMIT_RESTORE_SECONDS);
            tokio::time::sleep(Duration::from_secs_f64(seconds)).await;
        }
    }
}

/// Collate all user-facing error messages from a GraphQL `errors` array.
///
/// App-level errors with `category: "access_denied"` produce a standard
/// permission-denied message. Returns `"Unknown error"` if the array is
/// empty after processing.
fn extract_error_messages(errors: &[GraphqlError]) -> String {
    let mut messages: Vec<String> = Vec::new();
    for error in errors {
        if let Some(required_access) = error
            .extensions
            .as_ref()
            .and_then(missing_theme_access_requirement)
        {
            messages.push(format!(
                "The authenticated account or access token is missing {required_access}."
            ));
            continue;
        }
        if let Some(app_errors) = error
            .extensions
            .as_ref()
            .and_then(|ext| ext.app_errors.as_ref())
        {
            if let Some(app_errs) = &app_errors.errors {
                for ae in app_errs {
                    if ae.category.as_deref() == Some("access_denied") {
                        messages.push("You don't have the necessary permissions to perform this action. Check that you're using the correct account or token.".to_string());
                    } else if let Some(msg) = &ae.message {
                        messages.push(msg.clone());
                    }
                }
            }
        } else if let Some(msg) = &error.message {
            messages.push(msg.clone());
        }
    }

    if messages.is_empty() {
        "Unknown error".to_string()
    } else {
        messages.join("\n")
    }
}

fn missing_theme_access_requirement(extensions: &GraphqlErrorExtensions) -> Option<String> {
    if extensions.code.as_ref().and_then(Value::as_str) != Some("ACCESS_DENIED") {
        return None;
    }
    let required_access = extensions
        .required_access
        .as_ref()
        .and_then(Value::as_str)
        .map(|value| value.trim().trim_end_matches('.').to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "the required theme access scope".into());
    Some(required_access)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::FatalErrorType;
    use serde::Deserialize;
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn extract_error_messages_basic() {
        let errors = vec![GraphqlError {
            message: Some("field X not found".into()),
            extensions: None,
        }];
        assert_eq!(extract_error_messages(&errors), "field X not found");
    }

    #[tokio::test]
    async fn extract_error_messages_access_denied() {
        let errors = vec![GraphqlError {
            message: None,
            extensions: Some(GraphqlErrorExtensions {
                code: None,
                required_access: None,
                app_errors: Some(GraphqlAppErrors {
                    errors: Some(vec![GraphqlAppError {
                        message: Some("forbidden".into()),
                        category: Some("access_denied".into()),
                    }]),
                }),
            }),
        }];
        assert_eq!(
            extract_error_messages(&errors),
            "You don't have the necessary permissions to perform this action. Check that you're using the correct account or token."
        );
    }

    #[tokio::test]
    async fn extract_error_messages_app_error() {
        let errors = vec![GraphqlError {
            message: None,
            extensions: Some(GraphqlErrorExtensions {
                code: None,
                required_access: None,
                app_errors: Some(GraphqlAppErrors {
                    errors: Some(vec![GraphqlAppError {
                        message: Some("validation failed".into()),
                        category: Some("validation".into()),
                    }]),
                }),
            }),
        }];
        assert_eq!(extract_error_messages(&errors), "validation failed");
    }

    #[tokio::test]
    async fn extract_error_messages_theme_access_denied_with_required_scope() {
        let errors = vec![GraphqlError {
            message: Some("Access denied for themes field.".into()),
            extensions: Some(GraphqlErrorExtensions {
                code: Some(Value::String("ACCESS_DENIED".into())),
                required_access: Some(Value::String("`read_themes` access scope.".into())),
                app_errors: None,
            }),
        }];
        assert_eq!(
            extract_error_messages(&errors),
            "The authenticated account or access token is missing `read_themes` access scope."
        );
    }

    #[tokio::test]
    async fn extract_error_messages_theme_access_denied_default_scope() {
        let errors = vec![GraphqlError {
            message: Some("Access denied for themes field.".into()),
            extensions: Some(GraphqlErrorExtensions {
                code: Some(Value::String("ACCESS_DENIED".into())),
                required_access: None,
                app_errors: None,
            }),
        }];
        assert_eq!(
            extract_error_messages(&errors),
            "The authenticated account or access token is missing the required theme access scope."
        );
    }

    #[tokio::test]
    async fn extract_error_messages_empty() {
        let errors: Vec<GraphqlError> = vec![];
        assert_eq!(extract_error_messages(&errors), "Unknown error");
    }

    #[tokio::test]
    async fn extract_error_messages_multiple() {
        let errors = vec![
            GraphqlError {
                message: Some("first error".into()),
                extensions: None,
            },
            GraphqlError {
                message: Some("second error".into()),
                extensions: None,
            },
        ];
        assert_eq!(extract_error_messages(&errors), "first error\nsecond error");
    }

    #[test]
    fn graphql_request_error_display_network() {
        let err = GraphqlRequestError::Network("connection refused".into());
        assert_eq!(format!("{err}"), "Network error: connection refused");
    }

    #[test]
    fn graphql_request_error_display_api() {
        let err = GraphqlRequestError::ApiError("bad request".into(), 400);
        assert_eq!(
            format!("{err}"),
            "GraphQL API error (HTTP 400): bad request"
        );
    }

    #[test]
    fn graphql_request_error_display_parse() {
        let err = GraphqlRequestError::Parse("invalid json".into(), "raw text".into());
        assert_eq!(format!("{err}"), "Parse error: invalid json");
    }

    #[test]
    fn graphql_request_error_to_fatal_api_error() {
        let err = GraphqlRequestError::ApiError("not found".into(), 404);
        let fatal: FatalError = err.into();
        assert_eq!(fatal.message, "not found");
        assert_eq!(fatal.r#type, FatalErrorType::Abort);
    }

    #[test]
    fn graphql_request_error_to_fatal_network() {
        let err = GraphqlRequestError::Network("timeout".into());
        let fatal: FatalError = err.into();
        assert_eq!(fatal.message, "timeout");
    }

    #[tokio::test]
    async fn query_successful_response() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": { "value": 42 },
                "extensions": { "cost": { "actualQueryCost": null, "throttleStatus": null } }
            })))
            .mount(&mock_server)
            .await;

        let client = GraphqlClient::new(mock_server.uri(), None);
        let result: serde_json::Value = client.query("{ value }").await.unwrap();
        assert_eq!(result, json!({ "value": 42 }));
    }

    #[tokio::test]
    async fn query_parse_error_response() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
            .mount(&mock_server)
            .await;

        let client = GraphqlClient::new(mock_server.uri(), None);
        let result: Result<serde_json::Value, _> = client.query("{ value }").await;
        assert!(matches!(result, Err(GraphqlRequestError::Parse(_, _))));
    }

    #[tokio::test]
    async fn query_basic_ok() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": { "success": true },
                "extensions": { "cost": { "actualQueryCost": null, "throttleStatus": null } }
            })))
            .mount(&mock_server)
            .await;

        let client = GraphqlClient::new(mock_server.uri(), None);
        let result: serde_json::Value = client.query("{ success }").await.unwrap();
        assert_eq!(result, json!({ "success": true }));
    }

    #[tokio::test]
    async fn query_429_returns_api_error_after_retries_exhausted() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(429))
            .mount(&mock_server)
            .await;

        let client = GraphqlClient::new(mock_server.uri(), None);
        let result: Result<serde_json::Value, _> = client.query("{ value }").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn query_with_variables() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": { "name": "hello" },
                "extensions": { "cost": { "actualQueryCost": null, "throttleStatus": null } }
            })))
            .mount(&mock_server)
            .await;

        let client = GraphqlClient::new(mock_server.uri(), None);
        let result: serde_json::Value = client
            .query_with_variables::<_, serde_json::Value>(
                "query ($id: ID!) { name(id: $id) }",
                Some(json!({ "id": "123" })),
            )
            .await
            .unwrap();
        assert_eq!(result, json!({ "name": "hello" }));
    }

    #[tokio::test]
    async fn query_deserialize_only_response_type() {
        #[derive(Debug, Deserialize, PartialEq)]
        struct GeneratedLikeResponse {
            name: String,
        }

        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": { "name": "hello" },
                "extensions": { "cost": { "actualQueryCost": null, "throttleStatus": null } }
            })))
            .mount(&mock_server)
            .await;

        let client = GraphqlClient::new(mock_server.uri(), None);
        let result: GeneratedLikeResponse = client
            .query_with_variables(
                "query ($id: ID!) { name(id: $id) }",
                Some(json!({ "id": "123" })),
            )
            .await
            .unwrap();

        assert_eq!(
            result,
            GeneratedLikeResponse {
                name: "hello".into()
            }
        );
    }

    #[tokio::test]
    async fn query_api_error_response() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(500).set_body_json(json!({
                "errors": [{ "message": "internal server error" }]
            })))
            .mount(&mock_server)
            .await;

        let client = GraphqlClient::new(mock_server.uri(), None);
        let result: Result<serde_json::Value, _> = client.query("{ value }").await;
        match result {
            Err(GraphqlRequestError::ApiError(msg, 500)) => {
                assert!(msg.contains("internal server error"));
            }
            _ => panic!("expected ApiError with status 500"),
        }
    }

    #[tokio::test]
    async fn client_with_custom_reqwest_client() {
        let custom_client = build_client(Some(5000)).unwrap();
        let _client = GraphqlClient::with_client("http://example.com/graphql", None, custom_client);
    }

    #[tokio::test]
    async fn wait_for_rate_limit_restore_no_sleep_when_zero_cost() {
        let cost = GraphqlCost {
            actual_query_cost: Some(0.0),
            throttle_status: Some(GraphqlThrottleStatus {
                restore_rate: Some(50.0),
                currently_available: None,
                maximum_available: None,
            }),
        };
        wait_for_rate_limit_restore(&cost).await;
    }

    #[tokio::test]
    async fn wait_for_rate_limit_restore_no_sleep_when_no_restore_rate() {
        let cost = GraphqlCost {
            actual_query_cost: Some(10.0),
            throttle_status: None,
        };
        wait_for_rate_limit_restore(&cost).await;
    }

    #[tokio::test]
    async fn query_with_retry_config() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": { "ok": true },
                "extensions": { "cost": { "actualQueryCost": null, "throttleStatus": null } }
            })))
            .mount(&mock_server)
            .await;

        let client = GraphqlClient::new(mock_server.uri(), None).with_retry(RetryConfig::new());
        let result: serde_json::Value = client.query("{ ok }").await.unwrap();
        assert_eq!(result, json!({ "ok": true }));
    }

    #[tokio::test]
    async fn query_with_cache_caches_and_returns_cached() {
        use crate::util::cache::CacheStore;

        let mock_server = MockServer::start().await;
        let dir = tempfile::tempdir().unwrap();
        let cache = Arc::new(CacheStore::with_path(dir.path().join("cache.json")));

        let call_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let count = call_count.clone();

        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": { "value": "from-api" },
                "extensions": { "cost": { "actualQueryCost": null, "throttleStatus": null } }
            })))
            .mount(&mock_server)
            .await;

        let client = GraphqlClient::new(mock_server.uri(), None).with_cache(cache.clone());

        let result1: serde_json::Value = client.query("{ value }").await.unwrap();
        assert_eq!(result1, json!({ "value": "from-api" }));
        count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        let result2: serde_json::Value = client.query("{ value }").await.unwrap();
        assert_eq!(result2, json!({ "value": "from-api" }));

        assert_eq!(call_count.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn query_transient_retry_then_succeeds() {
        let mock_server = MockServer::start().await;
        let attempt = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let att = attempt.clone();

        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&mock_server)
            .await;

        let client = GraphqlClient::new(mock_server.uri(), None).with_retry(RetryConfig {
            max_retry_time: Duration::from_millis(500),
            initial_delay: Duration::from_millis(10),
            max_delay: Duration::from_millis(50),
            jitter: false,
            skip_env_var: None,
        });

        let result: Result<serde_json::Value, _> = client.query("{ value }").await;
        assert!(result.is_err());
        let _ = att.load(std::sync::atomic::Ordering::SeqCst);
    }
}
