use crate::error::AppError;
use crate::services::bulk_operations::client::BulkAdminClient;
use crate::services::bulk_operations::status::{get_bulk_operation_status, BulkOperationStatus};
use std::time::Duration;
use tokio_util::sync::CancellationToken;

const TERMINAL: &[&str] = &["COMPLETED", "FAILED", "CANCELED", "EXPIRED"];
const INITIAL_POLL_INTERVAL: Duration = Duration::from_secs(1);
const REGULAR_POLL_INTERVAL: Duration = Duration::from_secs(5);
const INITIAL_POLL_COUNT: u32 = 10;
pub const QUICK_WATCH_TIMEOUT: Duration = Duration::from_millis(3000);
pub const QUICK_WATCH_POLL_INTERVAL: Duration = Duration::from_millis(300);

#[derive(Clone)]
pub struct WatchOptions {
    pub adaptive: bool,
    pub poll_interval: Duration,
    pub initial_poll_interval: Duration,
    pub initial_poll_count: u32,
    pub timeout: Option<Duration>,
    pub abort: CancellationToken,
}

impl WatchOptions {
    pub fn full(abort: CancellationToken) -> Self {
        Self {
            adaptive: true,
            poll_interval: REGULAR_POLL_INTERVAL,
            initial_poll_interval: INITIAL_POLL_INTERVAL,
            initial_poll_count: INITIAL_POLL_COUNT,
            timeout: None,
            abort,
        }
    }

    pub fn short() -> Self {
        Self {
            adaptive: false,
            poll_interval: QUICK_WATCH_POLL_INTERVAL,
            initial_poll_interval: QUICK_WATCH_POLL_INTERVAL,
            initial_poll_count: 0,
            timeout: Some(QUICK_WATCH_TIMEOUT),
            abort: CancellationToken::new(),
        }
    }
}

/// Poll until a terminal status, abort, or timeout. No hard 240s cap on full watch.
pub async fn watch_bulk_operation(
    client: &dyn BulkAdminClient,
    id: &str,
    options: WatchOptions,
) -> Result<BulkOperationStatus, AppError> {
    let started = std::time::Instant::now();
    let mut poll_count = 0u32;
    loop {
        let status = get_bulk_operation_status(client, id).await?;
        if TERMINAL.contains(&status.status.as_str()) || options.abort.is_cancelled() {
            return Ok(status);
        }
        if let Some(timeout) = options.timeout {
            if started.elapsed() >= timeout {
                return Ok(status);
            }
        }
        poll_count += 1;
        let interval = if options.adaptive && poll_count <= options.initial_poll_count {
            options.initial_poll_interval
        } else {
            options.poll_interval
        };
        tokio::select! {
            _ = options.abort.cancelled() => return Ok(status),
            _ = tokio::time::sleep(interval) => {}
        }
    }
}

/// ~3s poll used when `--watch` is not set.
pub async fn short_bulk_operation_poll(
    client: &dyn BulkAdminClient,
    id: &str,
) -> Result<BulkOperationStatus, AppError> {
    watch_bulk_operation(client, id, WatchOptions::short()).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::bulk_operations::client::MockBulkAdminClient;

    fn payload(status: &str) -> serde_json::Value {
        serde_json::json!({
            "bulkOperation": {
                "id": "gid://shopify/BulkOperation/1",
                "status": status
            }
        })
    }

    #[tokio::test]
    async fn returns_terminal_immediately() {
        let mock = MockBulkAdminClient::default();
        *mock.get_by_id_queue.lock().unwrap() = vec![payload("COMPLETED")];
        let op = watch_bulk_operation(
            &mock,
            "gid://shopify/BulkOperation/1",
            WatchOptions::full(CancellationToken::new()),
        )
        .await
        .unwrap();
        assert_eq!(op.status, "COMPLETED");
    }

    #[tokio::test]
    async fn abort_returns_latest_status() {
        let mock = MockBulkAdminClient::default();
        *mock.get_by_id_queue.lock().unwrap() = vec![payload("RUNNING")];
        let abort = CancellationToken::new();
        abort.cancel();
        let op = watch_bulk_operation(
            &mock,
            "gid://shopify/BulkOperation/1",
            WatchOptions::full(abort),
        )
        .await
        .unwrap();
        assert_eq!(op.status, "RUNNING");
    }

    #[tokio::test]
    async fn short_poll_returns_running() {
        let mock = MockBulkAdminClient::default();
        *mock.get_by_id_queue.lock().unwrap() = vec![payload("RUNNING")];
        let mut opts = WatchOptions::short();
        opts.timeout = Some(Duration::from_millis(0));
        let op = watch_bulk_operation(&mock, "1", opts).await.unwrap();
        assert_eq!(op.status, "RUNNING");
    }
}
