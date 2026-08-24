//! Request sample payloads, API versions, and topics from the webhooks GraphQL API.
//!
//! Upstream: `request-sample.ts`, `request-api-versions.ts`, `request-topics.ts`.

use crate::error::AppError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SampleWebhook {
    pub sample_payload: String,
    pub headers: String,
    pub success: bool,
    pub user_errors: Vec<UserError>,
}

impl Default for SampleWebhook {
    fn default() -> Self {
        Self {
            sample_payload: "{}".into(),
            headers: "{}".into(),
            success: false,
            user_errors: vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserError {
    pub message: String,
    #[serde(default)]
    pub fields: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct SendSampleWebhookVariables {
    pub topic: String,
    pub api_version: String,
    pub address: String,
    pub delivery_method: String,
    pub shared_secret: String,
    pub api_key: Option<String>,
}

/// Platform source for webhook samples / catalogs. CLI wires [`WebhooksClient`]; tests inject a mock.
#[async_trait]
pub trait WebhookSampleClient: Send + Sync {
    async fn api_versions(&self) -> Result<Vec<String>, AppError>;
    async fn topics(&self, api_version: &str) -> Result<Vec<String>, AppError>;
    async fn send_sample_webhook(
        &self,
        variables: &SendSampleWebhookVariables,
    ) -> Result<SampleWebhook, AppError>;
}

/// Sort API versions newest-first with `unstable` last (upstream `request-api-versions.ts`).
pub fn sort_api_versions(mut versions: Vec<String>) -> Vec<String> {
    let unstable = versions
        .iter()
        .position(|v| v == "unstable")
        .map(|idx| versions.remove(idx));
    versions.sort();
    versions.reverse();
    if let Some(unstable) = unstable {
        versions.push(unstable);
    }
    versions
}

/// Request available API versions and return them ordered.
pub async fn request_api_versions(
    client: &dyn WebhookSampleClient,
) -> Result<Vec<String>, AppError> {
    Ok(sort_api_versions(client.api_versions().await?))
}

/// Request topics for an API version.
pub async fn request_topics(
    client: &dyn WebhookSampleClient,
    api_version: &str,
) -> Result<Vec<String>, AppError> {
    client.topics(api_version).await
}

/// Request a sample payload (or enqueue remote delivery) from the platform.
pub async fn get_webhook_sample(
    client: &dyn WebhookSampleClient,
    variables: &SendSampleWebhookVariables,
) -> Result<SampleWebhook, AppError> {
    client.send_sample_webhook(variables).await
}

/// Deterministic local sample used when no platform client is available (dev uninstall fallback).
pub fn resolve_sample_payload(topic: &str, api_version: &str) -> SampleWebhook {
    let payload = serde_json::json!({
        "id": 1,
        "note": format!("sample webhook payload for {topic}"),
        "api_version": api_version,
    });
    let headers = serde_json::json!({
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

/// In-memory [`WebhookSampleClient`] for unit tests.
#[derive(Debug)]
pub struct MockWebhookClient {
    pub versions: Vec<String>,
    pub topics: Vec<String>,
    pub sample: SampleWebhook,
    pub send_calls: Mutex<Vec<SendSampleWebhookVariables>>,
    pub topics_calls: Mutex<Vec<String>>,
    pub versions_calls: Mutex<u32>,
}

impl MockWebhookClient {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_lists(versions: Vec<String>, topics: Vec<String>) -> Self {
        Self {
            versions,
            topics,
            ..Default::default()
        }
    }

    pub fn with_sample(mut self, sample: SampleWebhook) -> Self {
        self.sample = sample;
        self
    }

    pub fn success_direct(payload: &str, headers: &str) -> SampleWebhook {
        SampleWebhook {
            sample_payload: payload.into(),
            headers: headers.into(),
            success: true,
            user_errors: vec![],
        }
    }

    pub fn success_enqueued() -> SampleWebhook {
        SampleWebhook {
            sample_payload: "{}".into(),
            headers: "{}".into(),
            success: true,
            user_errors: vec![],
        }
    }
}

impl Default for MockWebhookClient {
    fn default() -> Self {
        Self {
            versions: vec!["2024-07".into()],
            topics: vec!["orders/create".into()],
            sample: Self::success_enqueued(),
            send_calls: Mutex::new(vec![]),
            topics_calls: Mutex::new(vec![]),
            versions_calls: Mutex::new(0),
        }
    }
}

#[async_trait]
impl WebhookSampleClient for MockWebhookClient {
    async fn api_versions(&self) -> Result<Vec<String>, AppError> {
        *self.versions_calls.lock().unwrap() += 1;
        Ok(self.versions.clone())
    }

    async fn topics(&self, api_version: &str) -> Result<Vec<String>, AppError> {
        self.topics_calls
            .lock()
            .unwrap()
            .push(api_version.to_string());
        Ok(self.topics.clone())
    }

    async fn send_sample_webhook(
        &self,
        variables: &SendSampleWebhookVariables,
    ) -> Result<SampleWebhook, AppError> {
        self.send_calls.lock().unwrap().push(variables.clone());
        Ok(self.sample.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sorts_versions_unstable_last() {
        assert_eq!(
            sort_api_versions(vec!["2022".into(), "unstable".into(), "2023".into()]),
            vec!["2023", "2022", "unstable"]
        );
        assert_eq!(
            sort_api_versions(vec!["2024-07".into(), "2024-10".into()]),
            vec!["2024-10", "2024-07"]
        );
        assert_eq!(sort_api_versions(vec!["unstable".into()]), vec!["unstable"]);
    }

    #[tokio::test]
    async fn request_api_versions_orders() {
        let client = MockWebhookClient::with_lists(
            vec!["2022".into(), "unstable".into(), "2023".into()],
            vec![],
        );
        let got = request_api_versions(&client).await.unwrap();
        assert_eq!(got, vec!["2023", "2022", "unstable"]);
    }

    #[tokio::test]
    async fn request_topics_returns_array() {
        let client = MockWebhookClient::with_lists(
            vec!["2024-07".into()],
            vec!["orders/create".into(), "shop/redact".into()],
        );
        let got = request_topics(&client, "SOME_VERSION").await.unwrap();
        assert_eq!(got, vec!["orders/create", "shop/redact"]);
        assert_eq!(
            client.topics_calls.lock().unwrap().as_slice(),
            &["SOME_VERSION".to_string()]
        );
    }

    #[tokio::test]
    async fn get_webhook_sample_without_api_key() {
        let sample = MockWebhookClient::success_direct(
            r#"{ "sampleField": "SampleValue" }"#,
            r#"{ "header": "Header Value" }"#,
        );
        let client = MockWebhookClient::new().with_sample(sample);
        let vars = SendSampleWebhookVariables {
            topic: "A_TOPIC".into(),
            api_version: "A_VERSION".into(),
            delivery_method: "A_DELIVERY_METHOD".into(),
            address: "https://example.org".into(),
            shared_secret: "A_SECRET".into(),
            api_key: None,
        };
        let got = get_webhook_sample(&client, &vars).await.unwrap();
        assert_eq!(got.sample_payload, r#"{ "sampleField": "SampleValue" }"#);
        assert_eq!(got.headers, r#"{ "header": "Header Value" }"#);
        assert!(got.success);
        assert!(got.user_errors.is_empty());
        assert_eq!(client.send_calls.lock().unwrap().len(), 1);
        assert!(client.send_calls.lock().unwrap()[0].api_key.is_none());
    }

    #[tokio::test]
    async fn get_webhook_sample_with_api_key() {
        let sample = MockWebhookClient::success_direct(
            r#"{ "sampleField": "SampleValue" }"#,
            r#"{ "header": "Header Value" }"#,
        );
        let client = MockWebhookClient::new().with_sample(sample);
        let vars = SendSampleWebhookVariables {
            topic: "A_TOPIC".into(),
            api_version: "A_VERSION".into(),
            delivery_method: "A_DELIVERY_METHOD".into(),
            address: "https://example.org".into(),
            shared_secret: "A_SECRET".into(),
            api_key: Some("api-key".into()),
        };
        let got = get_webhook_sample(&client, &vars).await.unwrap();
        assert!(got.success);
        assert_eq!(
            client.send_calls.lock().unwrap()[0].api_key.as_deref(),
            Some("api-key")
        );
    }

    #[test]
    fn sample_contains_topic() {
        let sample = resolve_sample_payload("orders/create", "2024-07");
        assert!(sample.success);
        assert!(sample.sample_payload.contains("orders/create"));
        assert!(sample.headers.contains("X-Shopify-Topic"));
    }
}
