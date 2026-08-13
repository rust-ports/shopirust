//! HTTP poll + reusable [`AppLogsPoller`] (shared with T7 app_logs_polling).

use crate::error::AppError;
use cli_api::{AppLogData, AppLogsFetchResult, DeveloperPlatformClient};
use serde::Deserialize;
use std::collections::HashMap;
use std::future::Future;
use std::time::Duration;

pub const POLLING_INTERVAL_MS: u64 = 450;
pub const POLLING_ERROR_RETRY_INTERVAL_MS: u64 = 5_000;
pub const POLLING_THROTTLE_RETRY_INTERVAL_MS: u64 = 60_000;
pub const MAX_CONSECUTIVE_RESUBSCRIBE_FAILURES: u32 = 5;

#[derive(Debug, Clone, Default)]
pub struct PollFilters {
    pub status: Option<String>,
    pub sources: Option<Vec<String>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResubscribeResult {
    Succeeded,
    Failed,
    NotAttempted,
}

#[derive(Debug, Clone)]
pub enum PollOutcome {
    Success {
        app_logs: Vec<AppLogData>,
        cursor: Option<String>,
    },
    Error {
        status: u16,
        errors: Vec<String>,
    },
}

#[derive(Debug, Clone)]
pub struct PollOnceResult {
    pub outcome: PollOutcome,
    pub retry_interval_ms: u64,
    pub next_jwt_token: Option<String>,
    pub resubscribe_result: ResubscribeResult,
}

/// Where to fetch logs from.
pub enum PollBackend<'a> {
    /// Production path via [`DeveloperPlatformClient`].
    Platform {
        client: &'a dyn DeveloperPlatformClient,
        organization_id: String,
    },
    /// Direct HTTP against a base URL (wiremock / custom endpoints).
    Http { base_url: String },
}

/// Shared poller for `app logs` and T7 `app_logs_polling`.
pub struct AppLogsPoller {
    pub jwt_token: String,
    pub cursor: Option<String>,
    pub filters: PollFilters,
    consecutive_resubscribe_failures: u32,
}

impl AppLogsPoller {
    pub fn new(jwt_token: impl Into<String>, filters: PollFilters) -> Self {
        Self {
            jwt_token: jwt_token.into(),
            cursor: None,
            filters,
            consecutive_resubscribe_failures: 0,
        }
    }

    /// Single poll iteration (fetch + client-side filter + error handling).
    pub async fn poll_once<F, Fut>(
        &mut self,
        backend: &PollBackend<'_>,
        mut on_resubscribe: F,
    ) -> Result<PollOnceResult, AppError>
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = Result<String, AppError>>,
    {
        let fetched = match backend {
            PollBackend::Platform {
                client,
                organization_id,
            } => client
                .fetch_app_logs(
                    organization_id,
                    &self.jwt_token,
                    self.cursor.as_deref(),
                    None,
                )
                .await
                .map_err(|e| AppError::message(e.to_string())),
            PollBackend::Http { base_url } => {
                poll_app_logs_http(base_url, &self.jwt_token, self.cursor.as_deref(), None).await
            }
        };

        // Transport errors (timeouts, DNS, connection reset) retry — they must not abort the loop.
        let raw = match fetched {
            Ok(raw) => raw,
            Err(e) => {
                emit_poll_error_line(0, &[e.to_string()], POLLING_ERROR_RETRY_INTERVAL_MS);
                return Ok(PollOnceResult {
                    outcome: PollOutcome::Error {
                        status: 0,
                        errors: vec![e.to_string()],
                    },
                    retry_interval_ms: POLLING_ERROR_RETRY_INTERVAL_MS,
                    next_jwt_token: None,
                    resubscribe_result: ResubscribeResult::NotAttempted,
                });
            }
        };

        if raw.status != 200 {
            let error_response = AppLogsFetchResult {
                status: raw.status,
                app_logs: vec![],
                cursor: raw.cursor.clone(),
                errors: raw.errors.clone(),
            };
            let handled =
                handle_fetch_error(&error_response, || async { on_resubscribe().await }).await?;

            if handled.resubscribe_result == ResubscribeResult::Failed {
                self.consecutive_resubscribe_failures += 1;
            } else if handled.resubscribe_result == ResubscribeResult::Succeeded {
                self.consecutive_resubscribe_failures = 0;
            }

            if let Some(token) = &handled.next_jwt_token {
                self.jwt_token = token.clone();
            }

            emit_poll_error_line(raw.status, &raw.errors, handled.retry_interval_ms);
            return Ok(PollOnceResult {
                outcome: PollOutcome::Error {
                    status: raw.status,
                    errors: raw.errors,
                },
                retry_interval_ms: handled.retry_interval_ms,
                next_jwt_token: handled.next_jwt_token,
                resubscribe_result: handled.resubscribe_result,
            });
        }

        self.consecutive_resubscribe_failures = 0;
        if let Some(c) = raw.cursor.clone() {
            self.cursor = Some(c);
        }

        let filtered = filter_logs(raw.app_logs, &self.filters);
        Ok(PollOnceResult {
            outcome: PollOutcome::Success {
                app_logs: filtered,
                cursor: raw.cursor,
            },
            retry_interval_ms: POLLING_INTERVAL_MS,
            next_jwt_token: None,
            resubscribe_result: ResubscribeResult::NotAttempted,
        })
    }

    pub fn session_expired(&self) -> bool {
        self.consecutive_resubscribe_failures >= MAX_CONSECUTIVE_RESUBSCRIBE_FAILURES
    }

    /// Run the poll loop. Stops after `max_iterations` when set (tests / oneshot).
    pub async fn run_loop<F, Fut, H, HFut>(
        &mut self,
        backend: &PollBackend<'_>,
        max_iterations: Option<usize>,
        sleep_between: bool,
        mut on_resubscribe: F,
        mut on_logs: H,
    ) -> Result<(), AppError>
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = Result<String, AppError>>,
        H: FnMut(&[AppLogData]) -> HFut,
        HFut: Future<Output = Result<(), AppError>>,
    {
        let mut iterations = 0usize;
        loop {
            if let Some(max) = max_iterations {
                if iterations >= max {
                    break;
                }
            }
            iterations += 1;

            let result = self.poll_once(backend, &mut on_resubscribe).await?;

            match &result.outcome {
                PollOutcome::Success { app_logs, .. } => {
                    if !app_logs.is_empty() {
                        on_logs(app_logs).await?;
                    }
                }
                PollOutcome::Error { .. } => {
                    if self.session_expired() {
                        return Err(AppError::message(
                            "App log streaming session has expired. Please restart.",
                        ));
                    }
                }
            }

            if max_iterations.is_some() && iterations >= max_iterations.unwrap_or(0) {
                break;
            }

            if sleep_between {
                tokio::time::sleep(Duration::from_millis(result.retry_interval_ms)).await;
            }
        }
        Ok(())
    }
}

/// Client-side filter matching upstream `filterLogs`.
pub fn filter_logs(app_logs: Vec<AppLogData>, filters: &PollFilters) -> Vec<AppLogData> {
    if filters.status.is_none() && filters.sources.is_none() {
        return app_logs;
    }
    app_logs
        .into_iter()
        .filter(|log| {
            let status_match = filters
                .status
                .as_ref()
                .map(|s| &log.status == s)
                .unwrap_or(true);
            let source_match = filters
                .sources
                .as_ref()
                .map(|sources| {
                    let key = format!("{}.{}", log.source_namespace, log.source);
                    sources.iter().any(|s| s == &key)
                })
                .unwrap_or(true);
            status_match && source_match
        })
        .collect()
}

pub struct HandleFetchErrorResult {
    pub retry_interval_ms: u64,
    pub next_jwt_token: Option<String>,
    pub resubscribe_result: ResubscribeResult,
}

/// Upstream `handleFetchAppLogsError`.
pub async fn handle_fetch_error<F, Fut>(
    response: &AppLogsFetchResult,
    on_resubscribe: F,
) -> Result<HandleFetchErrorResult, AppError>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<String, AppError>>,
{
    let mut retry_interval_ms = POLLING_INTERVAL_MS;
    let mut next_jwt_token = None;
    let mut resubscribe_result = ResubscribeResult::NotAttempted;

    if response.errors.is_empty() && response.status == 200 {
        return Ok(HandleFetchErrorResult {
            retry_interval_ms,
            next_jwt_token,
            resubscribe_result,
        });
    }

    if response.status == 401 || response.errors.iter().any(|e| e.contains("401")) {
        match on_resubscribe().await {
            Ok(token) => {
                next_jwt_token = Some(token);
                resubscribe_result = ResubscribeResult::Succeeded;
            }
            Err(_) => {
                retry_interval_ms = POLLING_THROTTLE_RETRY_INTERVAL_MS;
                resubscribe_result = ResubscribeResult::Failed;
            }
        }
    } else if response.status == 429 {
        retry_interval_ms = POLLING_THROTTLE_RETRY_INTERVAL_MS;
    } else {
        retry_interval_ms = POLLING_ERROR_RETRY_INTERVAL_MS;
    }

    Ok(HandleFetchErrorResult {
        retry_interval_ms,
        next_jwt_token,
        resubscribe_result,
    })
}

fn emit_poll_error_line(status: u16, errors: &[String], retry_interval_ms: u64) {
    let line = serde_json::json!({
        "error": true,
        "status": status,
        "message": errors.join(", "),
        "retry_interval_ms": retry_interval_ms,
    });
    eprintln!("{line}");
}

/// Build poll URL with cursor (status/source URL filters unused — filtering is client-side).
pub fn build_poll_url(base_url: &str, cursor: Option<&str>) -> String {
    match cursor {
        Some(c) if !c.is_empty() => {
            let sep = if base_url.contains('?') { '&' } else { '?' };
            format!("{base_url}{sep}cursor={c}")
        }
        _ => base_url.to_string(),
    }
}

/// Low-level HTTP GET used by [`PollBackend::Http`] and unit tests.
pub async fn poll_app_logs_http(
    base_url: &str,
    jwt_token: &str,
    cursor: Option<&str>,
    _url_filters: Option<&HashMap<String, String>>,
) -> Result<AppLogsFetchResult, AppError> {
    let url = build_poll_url(base_url, cursor);
    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .header("Authorization", format!("Bearer {jwt_token}"))
        .header("Content-Type", "application/json")
        .send()
        .await
        .map_err(|e| AppError::message(e.to_string()))?;

    let status = response.status().as_u16();
    let body: serde_json::Value = response
        .json()
        .await
        .map_err(|e| AppError::message(format!("Failed to parse app logs response: {e}")))?;

    let errors = body
        .get("errors")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|e| e.as_str().map(str::to_string))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    if status != 200 {
        let errors = if errors.is_empty() {
            vec![format!("Request failed with status {status}")]
        } else {
            errors
        };
        return Ok(AppLogsFetchResult {
            status,
            app_logs: vec![],
            cursor: None,
            errors,
        });
    }

    #[derive(Deserialize)]
    struct SuccessBody {
        #[serde(default)]
        app_logs: Vec<AppLogData>,
        cursor: Option<String>,
    }

    let parsed: SuccessBody = serde_json::from_value(body).unwrap_or(SuccessBody {
        app_logs: vec![],
        cursor: None,
    });

    Ok(AppLogsFetchResult {
        status,
        app_logs: parsed.app_logs,
        cursor: parsed.cursor,
        errors: vec![],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn sample_log(status: &str, source: &str) -> AppLogData {
        AppLogData {
            shop_id: 1,
            api_client_id: 1830457,
            payload: r#"{"export":"run","fuel_consumed":1000000,"logs":"","input":null,"input_bytes":0,"output":null,"output_bytes":0,"function_id":"f","target":"","error_message":null,"error_type":null}"#.into(),
            log_type: "function_run".into(),
            source: source.into(),
            source_namespace: "extensions".into(),
            cursor: "c1".into(),
            status: status.into(),
            log_timestamp: "2024-05-23T19:17:00.240053Z".into(),
        }
    }

    #[test]
    fn filter_by_status_and_source() {
        let logs = vec![
            sample_log("success", "my-function"),
            sample_log("failure", "my-function"),
            sample_log("success", "other"),
        ];
        let filtered = filter_logs(
            logs,
            &PollFilters {
                status: Some("success".into()),
                sources: Some(vec!["extensions.my-function".into()]),
            },
        );
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].source, "my-function");
        assert_eq!(filtered[0].status, "success");
    }

    #[tokio::test]
    async fn poll_http_success_with_wiremock() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/app_logs/poll"))
            .and(header("Authorization", "Bearer test-jwt"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "app_logs": [{
                    "shop_id": 1,
                    "api_client_id": 2,
                    "payload": "{}",
                    "log_type": "function_run",
                    "source": "my-function",
                    "source_namespace": "extensions",
                    "cursor": "next",
                    "status": "success",
                    "log_timestamp": "2024-05-23T19:17:00.240053Z"
                }],
                "cursor": "next"
            })))
            .mount(&server)
            .await;

        let base = format!("{}/app_logs/poll", server.uri());
        let mut poller = AppLogsPoller::new("test-jwt", PollFilters::default());
        let backend = PollBackend::Http { base_url: base };
        let result = poller
            .poll_once(&backend, || async { Ok("new-jwt".into()) })
            .await
            .unwrap();

        match result.outcome {
            PollOutcome::Success { app_logs, cursor } => {
                assert_eq!(app_logs.len(), 1);
                assert_eq!(cursor.as_deref(), Some("next"));
                assert_eq!(poller.cursor.as_deref(), Some("next"));
            }
            PollOutcome::Error { .. } => panic!("expected success"),
        }
    }

    #[tokio::test]
    async fn poll_http_401_triggers_resubscribe() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/app_logs/poll"))
            .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
                "errors": ["unauthorized"]
            })))
            .mount(&server)
            .await;

        let base = format!("{}/app_logs/poll", server.uri());
        let mut poller = AppLogsPoller::new("old-jwt", PollFilters::default());
        let backend = PollBackend::Http { base_url: base };
        let result = poller
            .poll_once(&backend, || async { Ok("fresh-jwt".into()) })
            .await
            .unwrap();

        assert!(matches!(
            result.outcome,
            PollOutcome::Error { status: 401, .. }
        ));
        assert_eq!(result.resubscribe_result, ResubscribeResult::Succeeded);
        assert_eq!(result.next_jwt_token.as_deref(), Some("fresh-jwt"));
        assert_eq!(poller.jwt_token, "fresh-jwt");
    }

    #[tokio::test]
    async fn run_loop_oneshot() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/app_logs/poll"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "app_logs": [],
                "cursor": null
            })))
            .mount(&server)
            .await;

        let base = format!("{}/app_logs/poll", server.uri());
        let mut poller = AppLogsPoller::new("jwt", PollFilters::default());
        let backend = PollBackend::Http { base_url: base };
        let mut seen = 0;
        poller
            .run_loop(
                &backend,
                Some(1),
                false,
                || async { Ok("jwt".into()) },
                |_logs| {
                    seen += 1;
                    async { Ok(()) }
                },
            )
            .await
            .unwrap();
        // empty batch still counts as one iteration; on_logs not called
        assert_eq!(seen, 0);
    }

    #[tokio::test]
    async fn poll_http_429_throttles() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/app_logs/poll"))
            .respond_with(ResponseTemplate::new(429).set_body_json(serde_json::json!({
                "errors": ["rate limited"]
            })))
            .mount(&server)
            .await;

        let base = format!("{}/app_logs/poll", server.uri());
        let mut poller = AppLogsPoller::new("jwt", PollFilters::default());
        let backend = PollBackend::Http { base_url: base };
        let result = poller
            .poll_once(&backend, || async { Ok("jwt".into()) })
            .await
            .unwrap();
        assert!(matches!(
            result.outcome,
            PollOutcome::Error { status: 429, .. }
        ));
        assert_eq!(result.retry_interval_ms, POLLING_THROTTLE_RETRY_INTERVAL_MS);
    }

    #[tokio::test]
    async fn poll_http_5xx_retries_after_error_interval() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/app_logs/poll"))
            .respond_with(ResponseTemplate::new(503).set_body_json(serde_json::json!({
                "errors": ["unavailable"]
            })))
            .mount(&server)
            .await;

        let base = format!("{}/app_logs/poll", server.uri());
        let mut poller = AppLogsPoller::new("jwt", PollFilters::default());
        let backend = PollBackend::Http { base_url: base };
        let result = poller
            .poll_once(&backend, || async { Ok("jwt".into()) })
            .await
            .unwrap();
        assert!(matches!(
            result.outcome,
            PollOutcome::Error { status: 503, .. }
        ));
        assert_eq!(result.retry_interval_ms, POLLING_ERROR_RETRY_INTERVAL_MS);
    }

    #[tokio::test]
    async fn transport_error_retries_instead_of_aborting() {
        let mut poller = AppLogsPoller::new("jwt", PollFilters::default());
        let backend = PollBackend::Http {
            base_url: "http://127.0.0.1:1/app_logs/poll".into(),
        };
        let result = poller
            .poll_once(&backend, || async { Ok("jwt".into()) })
            .await
            .unwrap();
        match result.outcome {
            PollOutcome::Error { status, errors } => {
                assert_eq!(status, 0);
                assert!(!errors.is_empty());
            }
            PollOutcome::Success { .. } => panic!("expected transport error"),
        }
        assert_eq!(result.retry_interval_ms, POLLING_ERROR_RETRY_INTERVAL_MS);
    }

    #[tokio::test]
    async fn session_expired_after_five_resubscribe_failures() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/app_logs/poll"))
            .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
                "errors": ["unauthorized"]
            })))
            .mount(&server)
            .await;

        let base = format!("{}/app_logs/poll", server.uri());
        let mut poller = AppLogsPoller::new("jwt", PollFilters::default());
        let backend = PollBackend::Http { base_url: base };
        let err = poller
            .run_loop(
                &backend,
                Some(MAX_CONSECUTIVE_RESUBSCRIBE_FAILURES as usize),
                false,
                || async { Err(AppError::message("resubscribe failed")) },
                |_logs| async { Ok(()) },
            )
            .await
            .unwrap_err();
        assert!(err.to_string().contains("expired"));
        assert!(poller.session_expired());
    }
}
