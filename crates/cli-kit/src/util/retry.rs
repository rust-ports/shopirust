use rand::Rng;
use std::future::Future;
use std::time::Duration;
use tracing::debug;

const DEFAULT_INITIAL_DELAY_MS: u64 = 1000;
const DEFAULT_MAX_DELAY_MS: u64 = 3333;
const DEFAULT_MAX_RETRY_TIME_MS: u64 = 10000;
const SKIP_RETRY_ENV_VAR: &str = "SHOPIFY_CLI_SKIP_NETWORK_LEVEL_RETRY";

#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_retry_time: Duration,
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub jitter: bool,
    pub skip_env_var: Option<&'static str>,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retry_time: Duration::from_millis(DEFAULT_MAX_RETRY_TIME_MS),
            initial_delay: Duration::from_millis(DEFAULT_INITIAL_DELAY_MS),
            max_delay: Duration::from_millis(DEFAULT_MAX_DELAY_MS),
            jitter: true,
            skip_env_var: Some(SKIP_RETRY_ENV_VAR),
        }
    }
}

/// The result of a single operation attempt.
///
/// - `Ok(T)` — success, stop retrying
/// - `Retry(E)` — transient failure, retry with backoff
/// - `Err(E)` — fatal failure, stop immediately
pub enum RetryAction<T, E> {
    Ok(T),
    Retry(E),
    Err(E),
}

impl RetryConfig {
    pub fn new() -> Self {
        Self::default()
    }

    /// Check whether retries are disabled via the environment variable
    /// `SHOPIFY_CLI_SKIP_NETWORK_LEVEL_RETRY`.
    pub fn is_skipped(&self) -> bool {
        self.skip_env_var
            .and_then(|var| std::env::var(var).ok())
            .is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"))
    }

    /// Compute the backoff delay for a given attempt number.
    ///
    /// - attempt 0 returns 0ms
    /// - Each subsequent attempt doubles: `initial_delay * 2^(attempt-1)`
    /// - Capped at `max_delay`
    /// - When `jitter` is enabled, a random value in `[0, delay]` is used
    pub fn backoff_delay(&self, attempt: u32) -> Duration {
        if attempt == 0 {
            return Duration::from_millis(0);
        }
        let delay = self.initial_delay.as_millis() as u64 * 2u64.pow(attempt - 1);
        let delay = delay.min(self.max_delay.as_millis() as u64);
        if self.jitter {
            let jittered = rand::rngs::OsRng.gen_range(0..=delay);
            Duration::from_millis(jittered)
        } else {
            Duration::from_millis(delay)
        }
    }

    /// Execute an operation with retry logic.
    ///
    /// The `operation` closure returns a `RetryAction`:
    /// - `Ok(T)` — the operation succeeded
    /// - `Retry(E)` — the operation failed with a transient error; retry
    /// - `Err(E)` — the operation failed with a fatal error; stop
    pub async fn execute<F, Fut, T, E>(&self, operation: F) -> Result<T, E>
    where
        F: Fn() -> Fut,
        Fut: Future<Output = RetryAction<T, E>>,
    {
        if self.is_skipped() {
            return match operation().await {
                RetryAction::Ok(val) => Ok(val),
                RetryAction::Retry(e) | RetryAction::Err(e) => Err(e),
            };
        }

        let start = tokio::time::Instant::now();
        let mut attempt: u32 = 0;
        let mut last_error = None;

        loop {
            let result = operation().await;

            match result {
                RetryAction::Ok(val) => return Ok(val),
                RetryAction::Retry(e) => {
                    let _ = last_error.replace(e);
                }
                RetryAction::Err(e) => return Err(e),
            }

            let delay = self.backoff_delay(attempt);
            let elapsed = start.elapsed();

            if elapsed + delay > self.max_retry_time {
                debug!(
                    "max_retry_time reached (elapsed={:?}, next_delay={:?}), stopping retry",
                    elapsed, delay
                );
                return Err(last_error.expect("last_error must be set after a Retry"));
            }

            debug!(attempt, delay_ms = delay.as_millis(), "retrying operation");

            tokio::time::sleep(delay).await;
            attempt += 1;
        }
    }
}

/// Check whether an error message indicates a transient network error
/// that is likely to recover with retries.
pub fn is_transient_network_error(msg: &str) -> bool {
    let lower = msg.to_lowercase();
    let patterns = [
        "socket hang up",
        "econnreset",
        "econnaborted",
        "enotfound",
        "enetunreach",
        "network socket disconnected",
        "etimedout",
        "econnrefused",
        "eai_again",
        "epipe",
        "the operation was aborted",
        "timeout",
        "premature close",
        "getaddrinfo",
        "connection refused",
        "connection reset",
        "broken pipe",
    ];
    let missing_reason = lower.starts_with("request to ") && lower.ends_with("failed, reason:");
    patterns.iter().any(|p| lower.contains(p)) || missing_reason
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[test]
    fn backoff_delay_exponential() {
        let config = RetryConfig {
            jitter: false,
            ..Default::default()
        };
        assert_eq!(config.backoff_delay(0).as_millis(), 0);
        assert_eq!(config.backoff_delay(1).as_millis(), 1000);
        assert_eq!(config.backoff_delay(2).as_millis(), 2000);
        assert_eq!(config.backoff_delay(3).as_millis(), 3333);
        assert_eq!(config.backoff_delay(10).as_millis(), 3333);
    }

    #[test]
    fn backoff_delay_capped_at_max() {
        let config = RetryConfig {
            initial_delay: Duration::from_millis(5000),
            max_delay: Duration::from_millis(7000),
            jitter: false,
            ..Default::default()
        };
        assert_eq!(config.backoff_delay(1).as_millis(), 5000);
        assert_eq!(config.backoff_delay(2).as_millis(), 7000);
    }

    #[test]
    fn backoff_delay_with_jitter_differs() {
        let config = RetryConfig {
            jitter: true,
            ..Default::default()
        };
        let delays: Vec<u128> = (0..20)
            .map(|i| config.backoff_delay(i).as_millis())
            .collect();
        let are_all_same = delays.windows(2).all(|w| w[0] == w[1]);
        assert!(!are_all_same, "jitter should produce varying delays");
    }

    #[tokio::test]
    async fn execute_retries_on_retry_then_succeeds() {
        let config = RetryConfig {
            jitter: false,
            skip_env_var: None,
            ..Default::default()
        };
        let counter = Arc::new(AtomicUsize::new(0));
        let c = counter.clone();

        let result: Result<&str, &str> = config
            .execute(move || {
                let c = c.clone();
                async move {
                    let attempts = c.fetch_add(1, Ordering::SeqCst);
                    if attempts < 3 {
                        RetryAction::Retry("transient")
                    } else {
                        RetryAction::Ok("done")
                    }
                }
            })
            .await;

        assert_eq!(result.unwrap(), "done");
        assert_eq!(counter.load(Ordering::SeqCst), 4);
    }

    #[tokio::test]
    async fn execute_stops_on_action_err() {
        let config = RetryConfig {
            jitter: false,
            skip_env_var: None,
            ..Default::default()
        };
        let counter = Arc::new(AtomicUsize::new(0));
        let c = counter.clone();

        let result: Result<(), &str> = config
            .execute(move || {
                let c = c.clone();
                async move {
                    c.fetch_add(1, Ordering::SeqCst);
                    RetryAction::Err("fatal")
                }
            })
            .await;

        assert_eq!(result.unwrap_err(), "fatal");
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn execute_stops_after_max_retry_time() {
        let config = RetryConfig {
            max_retry_time: Duration::from_millis(50),
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_millis(100),
            jitter: false,
            skip_env_var: None,
        };
        let counter = Arc::new(AtomicUsize::new(0));
        let c = counter.clone();

        let result: Result<(), &str> = config
            .execute(move || {
                let c = c.clone();
                async move {
                    c.fetch_add(1, Ordering::SeqCst);
                    RetryAction::Retry("timeout")
                }
            })
            .await;

        assert_eq!(result.unwrap_err(), "timeout");
    }

    #[tokio::test]
    async fn execute_skipped_when_env_var_set() {
        std::env::set_var("TEST_SKIP_RETRY", "1");
        let config = RetryConfig {
            skip_env_var: Some("TEST_SKIP_RETRY"),
            ..Default::default()
        };
        let counter = Arc::new(AtomicUsize::new(0));
        let c = counter.clone();

        let result: Result<(), &str> = config
            .execute(move || {
                let c = c.clone();
                async move {
                    c.fetch_add(1, Ordering::SeqCst);
                    RetryAction::Retry("skipped")
                }
            })
            .await;

        assert!(result.is_err());
        assert_eq!(counter.load(Ordering::SeqCst), 1);
        std::env::remove_var("TEST_SKIP_RETRY");
    }

    #[tokio::test]
    async fn execute_logs_each_attempt() {
        let config = RetryConfig {
            jitter: false,
            skip_env_var: None,
            ..Default::default()
        };
        let counter = Arc::new(AtomicUsize::new(0));
        let c = counter.clone();

        let subscriber = tracing_subscriber::FmtSubscriber::builder()
            .with_test_writer()
            .with_max_level(tracing::Level::DEBUG)
            .finish();
        let _guard = tracing::subscriber::set_default(subscriber);

        let result: Result<&str, &str> = config
            .execute(move || {
                let c = c.clone();
                async move {
                    let attempts = c.fetch_add(1, Ordering::SeqCst);
                    if attempts < 2 {
                        RetryAction::Retry("transient")
                    } else {
                        RetryAction::Ok("done")
                    }
                }
            })
            .await;

        assert_eq!(result.unwrap(), "done");
    }

    #[test]
    fn is_transient_network_error_matches_socket_hang_up() {
        assert!(is_transient_network_error("socket hang up"));
    }

    #[test]
    fn is_transient_network_error_matches_econnreset() {
        assert!(is_transient_network_error("econnreset"));
    }

    #[test]
    fn is_transient_network_error_matches_timeout() {
        assert!(is_transient_network_error("timeout"));
    }

    #[test]
    fn is_transient_network_error_does_not_match_normal() {
        assert!(!is_transient_network_error("everything is fine"));
    }

    #[test]
    fn is_transient_network_error_matches_missing_reason() {
        assert!(is_transient_network_error(
            "request to https://example.com failed, reason:"
        ));
    }
}
