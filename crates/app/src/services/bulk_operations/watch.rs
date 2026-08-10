use crate::error::AppError;
use crate::services::bulk_operations::status::{
    get_bulk_operation_status, BulkOperationStatus,
};
use std::time::Duration;

/// Poll a bulk operation until it reaches a terminal status.
pub async fn watch_bulk_operation(
    admin_graphql_url: &str,
    token: &str,
    id: &str,
) -> Result<BulkOperationStatus, AppError> {
    for _ in 0..120 {
        let status = get_bulk_operation_status(admin_graphql_url, token, id).await?;
        match status.status.as_str() {
            "COMPLETED" | "FAILED" | "CANCELED" | "EXPIRED" => return Ok(status),
            _ => {
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    }
    Err(AppError::message(
        "Timed out waiting for bulk operation to complete",
    ))
}
