use crate::error::AppError;
use crate::services::bulk_operations::status::normalize_bulk_operation_id;
use serde_json::Value;

pub async fn cancel_bulk_operation(
    admin_graphql_url: &str,
    token: &str,
    id: &str,
) -> Result<Value, AppError> {
    let gid = normalize_bulk_operation_id(id);
    let mutation = r#"
      mutation BulkOperationCancel($id: ID!) {
        bulkOperationCancel(id: $id) {
          bulkOperation { id status }
          userErrors { field message }
        }
      }
    "#;
    let client = reqwest::Client::new();
    let resp = client
        .post(admin_graphql_url)
        .header("X-Shopify-Access-Token", token)
        .json(&serde_json::json!({ "query": mutation, "variables": { "id": gid } }))
        .send()
        .await
        .map_err(|e| AppError::message(e.to_string()))?;
    let value: Value = resp
        .json()
        .await
        .map_err(|e| AppError::message(e.to_string()))?;

    if let Some(errors) = value
        .pointer("/data/bulkOperationCancel/userErrors")
        .and_then(|v| v.as_array())
    {
        if !errors.is_empty() {
            let msg = errors
                .iter()
                .filter_map(|e| e.get("message").and_then(|m| m.as_str()))
                .collect::<Vec<_>>()
                .join("; ");
            return Err(AppError::message(msg));
        }
    }
    Ok(value)
}
