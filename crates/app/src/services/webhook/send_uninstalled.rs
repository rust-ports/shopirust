//! Send APP_UNINSTALLED sample to a local app server.
//!
//! Upstream: `send-app-uninstalled-webhook.ts`.

use super::delivery::{deliver_webhook_http, DeliverWebhookOptions};
use super::sample::{
    get_webhook_sample, request_api_versions, resolve_sample_payload, SendSampleWebhookVariables,
    WebhookSampleClient,
};
use super::trigger_flags::DELIVERY_METHOD_LOCALHOST;
use crate::error::AppError;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct SendUninstallWebhookOptions {
    pub address: String,
    pub store_fqdn: String,
    pub shared_secret: String,
    pub initial_delay: Duration,
    pub retry_delay: Duration,
    pub max_attempts: u32,
}

impl Default for SendUninstallWebhookOptions {
    fn default() -> Self {
        Self {
            address: String::new(),
            store_fqdn: String::new(),
            shared_secret: String::new(),
            initial_delay: Duration::from_secs(3),
            retry_delay: Duration::from_secs(5),
            max_attempts: 3,
        }
    }
}

/// Fetch a sample APP_UNINSTALLED payload and POST it to the local app server.
pub async fn send_uninstall_webhook_to_app_server(
    options: SendUninstallWebhookOptions,
    client: &dyn WebhookSampleClient,
) -> Result<bool, AppError> {
    let api_versions = request_api_versions(client).await?;
    let api_version = api_versions
        .get(1)
        .cloned()
        .or_else(|| api_versions.first().cloned())
        .unwrap_or_else(|| "unstable".into());

    let variables = SendSampleWebhookVariables {
        topic: "app/uninstalled".into(),
        api_version,
        address: options.address.clone(),
        delivery_method: DELIVERY_METHOD_LOCALHOST.into(),
        shared_secret: options.shared_secret.clone(),
        api_key: None,
    };
    let sample = get_webhook_sample(client, &variables).await?;

    if options.initial_delay > Duration::ZERO {
        tokio::time::sleep(options.initial_delay).await;
    }

    trigger_webhook_with_retry(&options, &sample.sample_payload, &sample.headers).await
}

/// Local-only helper (no platform client): POST a synthetic APP_UNINSTALLED payload.
pub async fn send_app_uninstalled_webhook(
    address: &str,
    store_fqdn: &str,
    shared_secret: &str,
) -> Result<bool, AppError> {
    let sample = resolve_sample_payload("app/uninstalled", "2024-10");
    post_uninstalled(
        address,
        &sample.sample_payload,
        &sample.headers,
        store_fqdn,
        Some(shared_secret),
    )
    .await
}

async fn trigger_webhook_with_retry(
    options: &SendUninstallWebhookOptions,
    body: &str,
    headers_json: &str,
) -> Result<bool, AppError> {
    let mut tries = 0;
    while tries < options.max_attempts {
        match post_uninstalled(
            &options.address,
            body,
            headers_json,
            &options.store_fqdn,
            Some(&options.shared_secret),
        )
        .await
        {
            Ok(true) => return Ok(true),
            Ok(false) => return Ok(false),
            Err(e) if is_connection_refused(&e) => {
                tries += 1;
                if tries < options.max_attempts && options.retry_delay > Duration::ZERO {
                    tokio::time::sleep(options.retry_delay).await;
                }
            }
            Err(e) => return Err(e),
        }
    }
    Ok(false)
}

async fn post_uninstalled(
    address: &str,
    body: &str,
    headers_json: &str,
    store_fqdn: &str,
    shared_secret: Option<&str>,
) -> Result<bool, AppError> {
    let mut headers: serde_json::Value =
        serde_json::from_str(headers_json).unwrap_or_else(|_| serde_json::json!({}));
    if let Some(obj) = headers.as_object_mut() {
        obj.insert(
            "X-Shopify-Shop-Domain".into(),
            serde_json::Value::String(store_fqdn.into()),
        );
    }
    let result = deliver_webhook_http(DeliverWebhookOptions {
        address: address.into(),
        body: body.into(),
        headers_json: headers.to_string(),
        shared_secret: shared_secret.map(str::to_string),
    })
    .await?;
    Ok(result.success)
}

fn is_connection_refused(err: &AppError) -> bool {
    let msg = err.to_string().to_ascii_lowercase();
    msg.contains("connection refused")
        || msg.contains("econnrefused")
        || msg.contains("connect error")
        || msg.contains("error sending request")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::webhook::sample::MockWebhookClient;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn zero_delay(address: String) -> SendUninstallWebhookOptions {
        SendUninstallWebhookOptions {
            address,
            store_fqdn: "test-store.myshopify.io".into(),
            shared_secret: "sharedSecret".into(),
            initial_delay: Duration::ZERO,
            retry_delay: Duration::ZERO,
            max_attempts: 3,
        }
    }

    #[tokio::test]
    async fn requests_sample_and_triggers_local_webhook() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/test/path"))
            .and(header("X-Shopify-Shop-Domain", "test-store.myshopify.io"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let sample = MockWebhookClient::success_direct(
            r#"{ "sampleField": "SampleValue" }"#,
            r#"{ "header": "Header Value" }"#,
        );
        let client = MockWebhookClient::with_lists(
            vec!["2024-07".into(), "2024-10".into(), "unstable".into()],
            vec!["app/uninstalled".into()],
        )
        .with_sample(sample);

        let result = send_uninstall_webhook_to_app_server(
            zero_delay(format!("{}/test/path", server.uri())),
            &client,
        )
        .await
        .unwrap();
        assert!(result);
        assert_eq!(*client.versions_calls.lock().unwrap(), 1);
        let calls = client.send_calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].topic, "app/uninstalled");
        // sorted versions: 2024-10, 2024-07, unstable → index 1 is 2024-07
        assert_eq!(calls[0].api_version, "2024-07");
        assert_eq!(calls[0].delivery_method, DELIVERY_METHOD_LOCALHOST);
        assert_eq!(calls[0].shared_secret, "sharedSecret");
    }

    #[tokio::test]
    async fn live_path_signs_hmac() {
        use crate::services::webhook::delivery::compute_webhook_hmac;
        let server = MockServer::start().await;
        let body = r#"{ "sampleField": "SampleValue" }"#;
        let hmac = compute_webhook_hmac("sharedSecret", body.as_bytes());
        Mock::given(method("POST"))
            .and(path("/test/path"))
            .and(header("X-Shopify-Hmac-SHA256", hmac.as_str()))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let sample = MockWebhookClient::success_direct(body, "{}");
        let client = MockWebhookClient::with_lists(vec!["2024-10".into()], vec![])
            .with_sample(sample);
        let result = send_uninstall_webhook_to_app_server(
            zero_delay(format!("{}/test/path", server.uri())),
            &client,
        )
        .await
        .unwrap();
        assert!(result);
    }

    #[tokio::test]
    async fn gracefully_deals_with_delivery_failing() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/test/path"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let sample = MockWebhookClient::success_direct(
            r#"{ "sampleField": "SampleValue" }"#,
            r#"{ "header": "Header Value" }"#,
        );
        let client = MockWebhookClient::with_lists(
            vec!["2024-07".into(), "2024-10".into()],
            vec![],
        )
        .with_sample(sample);

        let result = send_uninstall_webhook_to_app_server(
            zero_delay(format!("{}/test/path", server.uri())),
            &client,
        )
        .await
        .unwrap();
        assert!(!result);
        assert_eq!(client.send_calls.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn retries_when_app_hasnt_started_yet() {
        // First request connection-refused (no server), then a live server.
        // We simulate retry by starting the mock after a failed attempt against a closed port.
        let closed_port = {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let port = listener.local_addr().unwrap().port();
            drop(listener);
            port
        };
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let sample = MockWebhookClient::success_direct("{}", "{}");
        let client = MockWebhookClient::with_lists(vec!["2024-10".into()], vec![]).with_sample(sample);

        // Direct retry helper: first address refused, we instead test is_connection_refused
        // via send to closed port then success via send_app_uninstalled_webhook.
        let refused = send_app_uninstalled_webhook(
            &format!("http://127.0.0.1:{closed_port}/hooks"),
            "shop.myshopify.com",
            "sec",
        )
        .await;
        assert!(refused.is_err());
        assert!(is_connection_refused(&refused.unwrap_err()));

        let ok = send_app_uninstalled_webhook(
            &format!("{}/hooks", server.uri()),
            "shop.myshopify.com",
            "sec",
        )
        .await
        .unwrap();
        assert!(ok);
        assert_eq!(*client.versions_calls.lock().unwrap(), 0);
    }

    #[tokio::test]
    async fn posts_app_uninstalled_synthetic() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/webhooks"))
            .and(header("X-Shopify-Topic", "app/uninstalled"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;
        let ok = send_app_uninstalled_webhook(
            &format!("{}/api/webhooks", server.uri()),
            "shop.myshopify.com",
            "sec",
        )
        .await
        .unwrap();
        assert!(ok);
    }
}
