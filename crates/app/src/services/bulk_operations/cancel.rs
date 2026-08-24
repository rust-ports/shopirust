use crate::error::AppError;
use crate::services::bulk_operations::client::{graphql_user_errors, BulkAdminClient};
use crate::services::bulk_operations::status::{
    format_bulk_operation_cancellation_result, format_bulk_operation_user_errors,
    get_bulk_operation_status, normalize_bulk_operation_id, parse_bulk_operation_status,
    BulkOperationCancellationResult, BulkOperationStatus,
};
use serde_json::Value;

pub async fn cancel_bulk_operation(
    client: &dyn BulkAdminClient,
    id: &str,
) -> Result<(Value, BulkOperationCancellationResult), AppError> {
    let gid = normalize_bulk_operation_id(id);
    let value = client.cancel(&gid).await?;
    let errs = graphql_user_errors(&value);
    if !errs.is_empty() {
        return Err(AppError::message(format_bulk_operation_user_errors(
            &errs,
            "Error cancelling bulk operation.",
        )));
    }

    let operation = value
        .pointer("/bulkOperationCancel/bulkOperation")
        .or_else(|| value.pointer("/data/bulkOperationCancel/bulkOperation"))
        .map(parse_bulk_operation_status)
        .unwrap_or(BulkOperationStatus {
            id: gid.clone(),
            status: "CANCELING".into(),
            error_code: None,
            created_at: None,
            completed_at: None,
            object_count: None,
            file_size: None,
            url: None,
            partial_data_url: None,
            type_name: None,
        });

    // Prefer a follow-up status fetch when the cancel payload is thin.
    let operation = get_bulk_operation_status(client, &operation.id)
        .await
        .unwrap_or(operation);
    let formatted = format_bulk_operation_cancellation_result(&operation);
    Ok((value, formatted))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::bulk_operations::client::MockBulkAdminClient;

    #[tokio::test]
    async fn formats_canceling_result() {
        let mock = MockBulkAdminClient::default();
        *mock.cancel_response.lock().unwrap() = Some(serde_json::json!({
            "bulkOperationCancel": {
                "bulkOperation": { "id": "gid://shopify/BulkOperation/1", "status": "CANCELING" },
                "userErrors": []
            }
        }));
        *mock.get_by_id_queue.lock().unwrap() = vec![serde_json::json!({
            "bulkOperation": { "id": "gid://shopify/BulkOperation/1", "status": "CANCELING" }
        })];
        let (_, formatted) = cancel_bulk_operation(&mock, "1").await.unwrap();
        assert_eq!(formatted.render_type, "success");
        assert!(formatted.headline.contains("cancelled"));
    }
}
