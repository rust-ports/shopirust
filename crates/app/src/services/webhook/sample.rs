use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SampleWebhook {
    pub sample_payload: String,
    pub headers: String,
    pub success: bool,
    pub user_errors: Vec<UserError>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserError {
    pub message: String,
    pub fields: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct SendSampleWebhookVariables {
    pub topic: String,
    pub api_version: String,
    pub address: String,
    pub delivery_method: String,
    pub shared_secret: String,
    pub api_key: Option<String>,
}

/// Build a deterministic sample payload for local/HTTP delivery testing.
pub fn resolve_sample_payload(topic: &str, api_version: &str) -> SampleWebhook {
    let payload = json!({
        "id": 1,
        "note": format!("sample webhook payload for {topic}"),
        "api_version": api_version,
    });
    let headers = json!({
        "X-Shopify-Topic": topic,
        "X-Shopify-API-Version": api_version,
        "X-Shopify-Webhook-Id": "sample-webhook-id",
        "X-Shopify-Triggered-At": "1970-01-01T00:00:00.000000Z",
    });
    SampleWebhook {
        sample_payload: payload.to_string(),
        headers: headers.to_string(),
        success: true,
        user_errors: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_contains_topic() {
        let sample = resolve_sample_payload("orders/create", "2024-07");
        assert!(sample.success);
        assert!(sample.sample_payload.contains("orders/create"));
        assert!(sample.headers.contains("X-Shopify-Topic"));
    }
}
