//! App log streaming: subscribe JWT, poll HTTP endpoint, filter, render, write files.
//!
//! [`AppLogsPoller`] is shared by `shopify app logs` and (later) T7 `app_logs_polling`.

mod poll;
mod render;
mod sources;
mod write;

pub use poll::{
    filter_logs, handle_fetch_error, poll_app_logs_http, AppLogsPoller, PollBackend, PollFilters,
    PollOnceResult, PollOutcome, ResubscribeResult, MAX_CONSECUTIVE_RESUBSCRIBE_FAILURES,
    POLLING_ERROR_RETRY_INTERVAL_MS, POLLING_INTERVAL_MS, POLLING_THROTTLE_RETRY_INTERVAL_MS,
};
pub use render::{
    format_log_text, parse_app_log_payload, to_formatted_app_log_json, Format, ONE_MILLION,
    LOG_TYPE_FUNCTION_RUN, LOG_TYPE_REQUEST_EXECUTION, LOG_TYPE_REQUEST_EXECUTION_IN_BACKGROUND,
    LOG_TYPE_RESPONSE_FROM_CACHE,
};
pub use sources::{format_sources_output, sources_for_app};
pub use write::{write_app_logs_to_file, AppLogFile};

use crate::error::AppError;
use cli_api::{AppLogsSubscribeVariables, DeveloperPlatformClient};

/// Subscribe via GraphQL and return the JWT used for HTTP polling.
pub async fn subscribe_to_app_logs(
    client: &dyn DeveloperPlatformClient,
    shop_ids: &[i64],
    api_key: &str,
    organization_id: &str,
) -> Result<String, AppError> {
    let jwt = client
        .subscribe_to_app_logs(
            &AppLogsSubscribeVariables {
                shop_ids: shop_ids.to_vec(),
                api_key: api_key.to_string(),
            },
            organization_id,
        )
        .await
        .map_err(|e| AppError::message(e.to_string()))?;
    tracing::debug!(shop_ids = ?shop_ids, "Subscribed to app logs");
    Ok(jwt)
}
