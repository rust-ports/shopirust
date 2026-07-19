use crate::util::environment::{
    max_request_time_for_network_calls_ms, skip_network_level_retry,
};
use crate::util::retry::is_transient_network_error;
use std::fmt;
use reqwest::header::{self, HeaderMap, HeaderValue};
use reqwest::{Client, Response};
use std::collections::HashMap;
use std::time::Duration;
use tracing::debug;

const DEFAULT_TIMEOUT_MS: u64 = 30_000;
const DEFAULT_MAX_RETRY_TIME_MS: u64 = 10_000;
const USER_AGENT_STRING: &str = "Shopify CLI; v=3.94.3";

// ── Custom Error ────────────────────────────────────────────────────

#[derive(Debug)]
pub enum HttpError {
    Reqwest(reqwest::Error),
    Status(u16, String),
    Timeout,
    Io(std::io::Error),
}

impl fmt::Display for HttpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HttpError::Reqwest(e) => write!(f, "HTTP request error: {e}"),
            HttpError::Status(code, msg) => write!(f, "HTTP {code}: {msg}"),
            HttpError::Timeout => write!(f, "HTTP request timed out"),
            HttpError::Io(e) => write!(f, "I/O error: {e}"),
        }
    }
}

impl std::error::Error for HttpError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            HttpError::Reqwest(e) => Some(e),
            HttpError::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<reqwest::Error> for HttpError {
    fn from(e: reqwest::Error) -> Self {
        HttpError::Reqwest(e)
    }
}

impl From<std::io::Error> for HttpError {
    fn from(e: std::io::Error) -> Self {
        HttpError::Io(e)
    }
}

// ── Request Modes ───────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct NetworkRetryBehaviour {
    pub enabled: bool,
    pub max_retry_time_ms: u64,
}

#[derive(Debug, Clone)]
pub struct AutoCancelBehaviour {
    pub enabled: bool,
    pub timeout_ms: u64,
}

#[derive(Debug, Clone)]
pub struct RequestBehaviour {
    pub use_network_retry: NetworkRetryBehaviour,
    pub use_abort_signal: AutoCancelBehaviour,
}

#[derive(Debug, Clone)]
pub enum RequestMode {
    Default,
    NonBlocking,
    SlowRequest,
    Custom(RequestBehaviour),
}

impl From<RequestMode> for RequestBehaviour {
    fn from(mode: RequestMode) -> Self {
        match mode {
            RequestMode::Default => RequestBehaviour {
                use_network_retry: NetworkRetryBehaviour {
                    enabled: true,
                    max_retry_time_ms: DEFAULT_MAX_RETRY_TIME_MS,
                },
                use_abort_signal: AutoCancelBehaviour {
                    enabled: true,
                    timeout_ms: DEFAULT_TIMEOUT_MS,
                },
            },
            RequestMode::NonBlocking => RequestBehaviour {
                use_network_retry: NetworkRetryBehaviour {
                    enabled: false,
                    max_retry_time_ms: DEFAULT_MAX_RETRY_TIME_MS,
                },
                use_abort_signal: AutoCancelBehaviour {
                    enabled: true,
                    timeout_ms: DEFAULT_TIMEOUT_MS,
                },
            },
            RequestMode::SlowRequest => RequestBehaviour {
                use_network_retry: NetworkRetryBehaviour {
                    enabled: false,
                    max_retry_time_ms: DEFAULT_MAX_RETRY_TIME_MS,
                },
                use_abort_signal: AutoCancelBehaviour {
                    enabled: false,
                    timeout_ms: DEFAULT_TIMEOUT_MS,
                },
            },
            RequestMode::Custom(b) => b,
        }
    }
}

pub fn request_mode(
    preset: Option<RequestMode>,
    env: Option<&HashMap<String, String>>,
) -> RequestBehaviour {
    let network_retry_supported = !skip_network_level_retry(env);
    let timeout_ms = max_request_time_for_network_calls_ms(env);

    match preset.unwrap_or(RequestMode::NonBlocking) {
        RequestMode::Default => RequestBehaviour {
            use_network_retry: NetworkRetryBehaviour {
                enabled: network_retry_supported,
                max_retry_time_ms: DEFAULT_MAX_RETRY_TIME_MS,
            },
            use_abort_signal: AutoCancelBehaviour {
                enabled: true,
                timeout_ms,
            },
        },
        RequestMode::NonBlocking => RequestBehaviour {
            use_network_retry: NetworkRetryBehaviour {
                enabled: false,
                max_retry_time_ms: DEFAULT_MAX_RETRY_TIME_MS,
            },
            use_abort_signal: AutoCancelBehaviour {
                enabled: true,
                timeout_ms,
            },
        },
        RequestMode::SlowRequest => RequestBehaviour {
            use_network_retry: NetworkRetryBehaviour {
                enabled: false,
                max_retry_time_ms: DEFAULT_MAX_RETRY_TIME_MS,
            },
            use_abort_signal: AutoCancelBehaviour {
                enabled: false,
                timeout_ms: DEFAULT_TIMEOUT_MS,
            },
        },
        RequestMode::Custom(b) => RequestBehaviour {
            use_network_retry: if network_retry_supported {
                b.use_network_retry
            } else {
                NetworkRetryBehaviour {
                    enabled: false,
                    max_retry_time_ms: b.use_network_retry.max_retry_time_ms,
                }
            },
            ..b
        },
    }
}

// ── Header Building ─────────────────────────────────────────────────

pub fn build_headers(token: Option<&str>) -> HeaderMap {
    let mut headers = HeaderMap::new();

    headers.insert(
        header::USER_AGENT,
        HeaderValue::from_static(USER_AGENT_STRING),
    );
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
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
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_str(&auth_str).unwrap(),
        );
        headers.insert(
            header::HeaderName::from_static("x-shopify-access-token"),
            HeaderValue::from_str(&auth_str).unwrap(),
        );
    }

    headers
}

// ── Client Building ─────────────────────────────────────────────────

fn default_client() -> Client {
    Client::builder()
        .timeout(Duration::from_millis(DEFAULT_TIMEOUT_MS))
        .pool_idle_timeout(Duration::from_secs(30))
        .user_agent(USER_AGENT_STRING)
        .build()
        .expect("Failed to build HTTP client")
}

fn shopify_client() -> Client {
    Client::builder()
        .timeout(Duration::from_millis(DEFAULT_TIMEOUT_MS))
        .pool_idle_timeout(Duration::from_secs(30))
        .user_agent(USER_AGENT_STRING)
        .https_only(true)
        .build()
        .expect("Failed to build Shopify HTTP client")
}

pub fn build_client(timeout_ms: Option<u64>) -> reqwest::Result<Client> {
    Client::builder()
        .timeout(Duration::from_millis(timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS)))
        .pool_idle_timeout(Duration::from_secs(30))
        .user_agent(USER_AGENT_STRING)
        .build()
}

pub fn build_shopify_client(timeout_ms: Option<u64>) -> reqwest::Result<Client> {
    Client::builder()
        .timeout(Duration::from_millis(timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS)))
        .pool_idle_timeout(Duration::from_secs(30))
        .user_agent(USER_AGENT_STRING)
        .https_only(true)
        .build()
}

// ── Helpers ──────────────────────────────────────────────────────────

fn is_interesting_header(name: &str) -> bool {
    matches!(
        name,
        "cache-control" | "content-type" | "etag" | "x-request-id" | "server-timing" | "retry-after"
    )
}

fn sanitized_headers_output(headers: &HeaderMap) -> String {
    let mut out = String::new();
    for (name, value) in headers.iter() {
        if is_interesting_header(name.as_str()) {
            if let Ok(v) = value.to_str() {
                out.push_str(&format!("  {name}: {v}\n"));
            }
        }
    }
    out
}

pub fn extract_request_id(response: &Response) -> Option<&str> {
    response.headers().get("x-request-id")?.to_str().ok()
}

// ── Core Request Execution ──────────────────────────────────────────

/// Build a request and send it once.
async fn send_once(
    client: &Client,
    method: reqwest::Method,
    url: &str,
    headers: Option<&HeaderMap>,
    body: Option<&str>,
) -> Result<Response, HttpError> {
    let mut req = client.request(method, url);
    if let Some(h) = headers {
        req = req.headers(h.clone());
    }
    if let Some(b) = body {
        req = req.body(b.to_string());
    }
    req.send().await.map_err(HttpError::Reqwest)
}

/// Execute a single request, optionally with network-level retry.
async fn execute_request(
    client: &Client,
    method: reqwest::Method,
    url: &str,
    headers: Option<&HeaderMap>,
    body: Option<&str>,
    behaviour: &RequestBehaviour,
    log_request: bool,
) -> Result<Response, HttpError> {
    if log_request {
        debug!(
            "Sending {method} request to URL {url} With request headers: {}",
            sanitized_headers_output(headers.unwrap_or(&HeaderMap::new()))
        );
    }

    if !behaviour.use_network_retry.enabled {
        let response = send_once(client, method, url, headers, body).await?;
        if log_request {
            debug!("Request to {url} completed with status {}", response.status());
        }
        return Ok(response);
    }

    let max_retry = Duration::from_millis(behaviour.use_network_retry.max_retry_time_ms);
    let start = tokio::time::Instant::now();
    let mut attempt: u32 = 0;
    let mut error_occurred = false;
    let mut last_error = HttpError::Timeout;

    while start.elapsed() < max_retry {
        let result = send_once(client, method.clone(), url, headers, body).await;
        match result {
            Ok(response) => {
                if response.status().is_success() {
                    if log_request {
                        debug!(
                            "Request to {url} completed with status {}",
                            response.status()
                        );
                    }
                    return Ok(response);
                }
                if response.status().is_server_error() {
                    let code = response.status().as_u16();
                    let msg = format!("server error: {code}");
                    debug!("Server error on {url}: {msg}");
                    error_occurred = true;
                    last_error = HttpError::Status(code, msg);
                } else {
                    if log_request {
                        debug!(
                            "Request to {url} completed with non-retryable status {}",
                            response.status()
                        );
                    }
                    return Ok(response);
                }
            }
            Err(HttpError::Reqwest(e)) => {
                if !is_transient_network_error(&e.to_string()) {
                    return Err(HttpError::Reqwest(e));
                }
                debug!("Transient network error to {url}: {e}");
                error_occurred = true;
                last_error = HttpError::Reqwest(e);
            }
            Err(e) => {
                return Err(e);
            }
        }

        attempt += 1;
        let delay_ms = 1000 * 2u64.pow(attempt.saturating_sub(1)).min(5000);
        let remaining = max_retry.saturating_sub(start.elapsed()).as_millis() as u64;
        let delay = Duration::from_millis(delay_ms.min(remaining));
        tokio::time::sleep(delay).await;
    }

    if error_occurred { Err(last_error) } else { Err(HttpError::Timeout) }
}

// ── Public API ───────────────────────────────────────────────────────

/// Basic fetch (non-blocking by default).
pub async fn fetch(
    url: &str,
    method: Option<reqwest::Method>,
    headers: Option<HeaderMap>,
    body: Option<String>,
) -> Result<Response, HttpError> {
    let behaviour = request_mode(Some(RequestMode::NonBlocking), None);
    let client = default_client();
    execute_request(
        &client,
        method.unwrap_or(reqwest::Method::GET),
        url,
        headers.as_ref(),
        body.as_deref(),
        &behaviour,
        false,
    )
    .await
}

/// Shopify-specific fetch (default mode, TLS, logging).
pub async fn shopify_fetch(
    url: &str,
    method: Option<reqwest::Method>,
    headers: Option<HeaderMap>,
    body: Option<String>,
) -> Result<Response, HttpError> {
    let behaviour = request_mode(Some(RequestMode::Default), None);
    let client = shopify_client();
    execute_request(
        &client,
        method.unwrap_or(reqwest::Method::GET),
        url,
        headers.as_ref(),
        body.as_deref(),
        &behaviour,
        true,
    )
    .await
}

/// Fetch with explicit behaviour configuration.
pub async fn fetch_with_behaviour(
    url: &str,
    method: reqwest::Method,
    headers: Option<HeaderMap>,
    body: Option<String>,
    behaviour: &RequestBehaviour,
    log_request: bool,
) -> Result<Response, HttpError> {
    let client = default_client();
    execute_request(
        &client,
        method,
        url,
        headers.as_ref(),
        body.as_deref(),
        behaviour,
        log_request,
    )
    .await
}

/// Download a file from a URL to a local path.
pub async fn download_file(url: &str, to: &std::path::Path) -> Result<String, HttpError> {
    debug!("Downloading {url} to {}", to.display());

    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent).map_err(HttpError::Io)?;
    }

    let response = shopify_fetch(url, Some(reqwest::Method::GET), None, None).await?;

    let bytes = response
        .bytes()
        .await
        .map_err(HttpError::Reqwest)?;

    std::fs::write(to, &bytes).map_err(HttpError::Io)?;
    Ok(to.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_headers_without_token() {
        let headers = build_headers(None);
        assert_eq!(
            headers.get(header::USER_AGENT).unwrap(),
            "Shopify CLI; v=3.94.3"
        );
        assert_eq!(headers.get(header::CACHE_CONTROL).unwrap(), "no-cache");
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
        assert_eq!(headers.get(header::AUTHORIZATION).unwrap(), "Bearer abc123");
        assert_eq!(
            headers.get("x-shopify-access-token").unwrap(),
            "Bearer abc123"
        );
    }

    #[test]
    fn build_headers_with_shpat_token() {
        let headers = build_headers(Some("shpat_abc123"));
        assert_eq!(headers.get(header::AUTHORIZATION).unwrap(), "shpat_abc123");
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
        assert!(std::mem::size_of_val(&client) > 0);
    }

    #[test]
    fn build_client_custom_timeout() {
        let client = build_client(Some(5000)).unwrap();
        assert!(std::mem::size_of_val(&client) > 0);
    }

    #[test]
    fn test_request_mode_default() {
        let behaviour = request_mode(Some(RequestMode::Default), None);
        assert!(behaviour.use_network_retry.enabled);
        assert!(behaviour.use_abort_signal.enabled);
    }

    #[test]
    fn test_request_mode_non_blocking() {
        let behaviour = request_mode(Some(RequestMode::NonBlocking), None);
        assert!(!behaviour.use_network_retry.enabled);
        assert!(behaviour.use_abort_signal.enabled);
    }

    #[test]
    fn test_request_mode_slow() {
        let behaviour = request_mode(Some(RequestMode::SlowRequest), None);
        assert!(!behaviour.use_network_retry.enabled);
        assert!(!behaviour.use_abort_signal.enabled);
    }

    #[test]
    fn test_is_interesting_header() {
        assert!(is_interesting_header("x-request-id"));
        assert!(is_interesting_header("content-type"));
        assert!(!is_interesting_header("x-random"));
    }

    #[test]
    fn test_build_shopify_client() {
        let client = build_shopify_client(None).unwrap();
        assert!(std::mem::size_of_val(&client) > 0);
    }
}
