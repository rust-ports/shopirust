use super::delivery::{deliver_webhook_http, DeliverWebhookOptions};
use super::sample::{resolve_sample_payload, SampleWebhook};
use super::trigger_flags::{
    delivery_method_for_address, validate_address_method, DELIVERY_METHOD_HTTP,
    DELIVERY_METHOD_LOCALHOST,
};
use crate::error::AppError;

#[derive(Debug, Clone)]
pub struct WebhookTriggerOptions {
    pub topic: String,
    pub api_version: String,
    pub address: String,
    pub delivery_method: Option<String>,
    pub client_secret: Option<String>,
    pub api_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebhookTriggerResult {
    pub message: String,
    pub success: bool,
    pub sample: Option<SampleWebhook>,
}

/// Resolve a sample payload and deliver it.
///
/// Localhost (and HTTP when targeting a reachable URL) POSTs from the CLI with an optional
/// HMAC header. Pub/Sub and EventBridge are reported as remote enqueue (Partners API path
/// not required for local testing).
pub async fn webhook_trigger(options: WebhookTriggerOptions) -> Result<WebhookTriggerResult, AppError> {
    let method = options
        .delivery_method
        .clone()
        .or_else(|| {
            delivery_method_for_address(Some(&options.address)).map(|m| m.as_str().to_string())
        })
        .ok_or_else(|| {
            AppError::message(
                "Could not infer delivery method from address. Pass --delivery-method (http, google-pub-sub, event-bridge).",
            )
        })?;

    let (address, method) = validate_address_method(&options.address, &method)?;
    let sample = resolve_sample_payload(&options.topic, &options.api_version);

    if method == DELIVERY_METHOD_LOCALHOST || method == DELIVERY_METHOD_HTTP {
        let delivered = deliver_webhook_http(DeliverWebhookOptions {
            address,
            body: sample.sample_payload.clone(),
            headers_json: sample.headers.clone(),
            shared_secret: options.client_secret.clone(),
        })
        .await?;

        if delivered.success {
            let label = if method == DELIVERY_METHOD_LOCALHOST {
                "Localhost"
            } else {
                "HTTP"
            };
            return Ok(WebhookTriggerResult {
                message: format!("{label} delivery successful"),
                success: true,
                sample: Some(sample),
            });
        }
        return Ok(WebhookTriggerResult {
            message: format!(
                "Delivery failed{}",
                delivered
                    .status
                    .map(|s| format!(" (HTTP {s})"))
                    .unwrap_or_default()
            ),
            success: false,
            sample: Some(sample),
        });
    }

    // Remote destinations (pub/sub, event-bridge): Partners enqueues delivery.
    let _ = options.api_key;
    Ok(WebhookTriggerResult {
        message: format!(
            "Webhook sample prepared for remote delivery via '{method}' to {address}. \
             Use Partners/Developer Dashboard APIs to enqueue remote delivery in full CLI."
        ),
        success: true,
        sample: Some(sample),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn rejects_unknown_address_without_method() {
        let err = webhook_trigger(WebhookTriggerOptions {
            topic: "orders/create".into(),
            api_version: "2024-07".into(),
            address: "ftp://example.com".into(),
            delivery_method: None,
            client_secret: None,
            api_key: None,
        })
        .await
        .unwrap_err();
        assert!(err.to_string().contains("delivery method"));
    }

    #[tokio::test]
    async fn validates_pubsub_address() {
        let result = webhook_trigger(WebhookTriggerOptions {
            topic: "orders/create".into(),
            api_version: "2024-07".into(),
            address: "pubsub://project:topic".into(),
            delivery_method: Some("google-pub-sub".into()),
            client_secret: Some("sec".into()),
            api_key: None,
        })
        .await
        .unwrap();
        assert!(result.success);
        assert!(result.sample.is_some());
    }
}
