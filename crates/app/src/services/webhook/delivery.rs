use crate::error::AppError;
use base64::Engine;
use hmac::{Hmac, Mac};
use reqwest::Client;
use serde_json::Value;
use sha2::Sha256;
use std::collections::HashMap;

type HmacSha256 = Hmac<Sha256>;

/// Compute Shopify webhook HMAC (`base64(HMAC-SHA256(secret, body))`).
pub fn compute_webhook_hmac(secret: &str, body: &[u8]) -> String {
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key length");
    mac.update(body);
    let result = mac.finalize().into_bytes();
    base64::engine::general_purpose::STANDARD.encode(result)
}

pub fn build_webhook_headers(
    headers_json: &str,
    body: &str,
    shared_secret: Option<&str>,
) -> Result<HashMap<String, String>, AppError> {
    let mut headers: HashMap<String, String> = HashMap::new();
    headers.insert("Content-Type".into(), "application/json".into());

    if !headers_json.trim().is_empty() && headers_json.trim() != "{}" {
        let parsed: Value = serde_json::from_str(headers_json)
            .map_err(|e| AppError::message(format!("Invalid webhook headers JSON: {e}")))?;
        if let Some(obj) = parsed.as_object() {
            for (k, v) in obj {
                if let Some(s) = v.as_str() {
                    headers.insert(k.clone(), s.to_string());
                } else {
                    headers.insert(k.clone(), v.to_string());
                }
            }
        }
    }

    if let Some(secret) = shared_secret.filter(|s| !s.is_empty()) {
        let hmac = compute_webhook_hmac(secret, body.as_bytes());
        headers.insert("X-Shopify-Hmac-SHA256".into(), hmac);
    }

    Ok(headers)
}

#[derive(Debug, Clone)]
pub struct DeliverWebhookOptions {
    pub address: String,
    pub body: String,
    pub headers_json: String,
    pub shared_secret: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliverWebhookResult {
    pub success: bool,
    pub status: Option<u16>,
}

pub async fn deliver_webhook_http(
    options: DeliverWebhookOptions,
) -> Result<DeliverWebhookResult, AppError> {
    let headers = build_webhook_headers(
        &options.headers_json,
        &options.body,
        options.shared_secret.as_deref(),
    )?;

    let client = Client::new();
    let mut req = client.post(&options.address).body(options.body.clone());
    for (k, v) in headers {
        req = req.header(k, v);
    }

    let response = req
        .send()
        .await
        .map_err(|e| AppError::message(format!("Failed to deliver webhook: {e}")))?;
    let status = response.status().as_u16();
    Ok(DeliverWebhookResult {
        success: (200..300).contains(&status),
        status: Some(status),
    })
}

/// POST a webhook payload to a local endpoint (upstream `triggerLocalWebhook`).
pub async fn trigger_local_webhook(
    address: &str,
    body: &str,
    headers_json: &str,
) -> Result<bool, AppError> {
    let result = deliver_webhook_http(DeliverWebhookOptions {
        address: address.into(),
        body: body.into(),
        headers_json: headers_json.into(),
        shared_secret: None,
    })
    .await?;
    Ok(result.success)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hmac_is_stable_base64() {
        let hmac = compute_webhook_hmac("secret", b"{\"hello\":true}");
        assert_eq!(hmac, "D7P9kCpOAsqxJo4K3DF5YMFW2Q+5rIs+0FG/9+OkpOo=");
    }

    #[test]
    fn builds_headers_with_hmac() {
        let headers =
            build_webhook_headers(r#"{"X-Shopify-Topic":"orders/create"}"#, "{}", Some("sec"))
                .unwrap();
        assert_eq!(
            headers.get("Content-Type").map(String::as_str),
            Some("application/json")
        );
        assert_eq!(
            headers.get("X-Shopify-Topic").map(String::as_str),
            Some("orders/create")
        );
        assert!(headers.contains_key("X-Shopify-Hmac-SHA256"));
        assert_eq!(
            headers.get("X-Shopify-Hmac-SHA256").unwrap(),
            &compute_webhook_hmac("sec", b"{}")
        );
    }

    #[test]
    fn omits_hmac_without_secret() {
        let headers = build_webhook_headers("{}", "{}", None).unwrap();
        assert!(!headers.contains_key("X-Shopify-Hmac-SHA256"));
    }

    #[tokio::test]
    async fn delivers_http_post_with_hmac() {
        use wiremock::matchers::{body_string, header, method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        let body = r#"{"id":1}"#;
        let hmac = compute_webhook_hmac("sec", body.as_bytes());

        Mock::given(method("POST"))
            .and(path("/hooks"))
            .and(body_string(body))
            .and(header("X-Shopify-Hmac-SHA256", hmac.as_str()))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let result = deliver_webhook_http(DeliverWebhookOptions {
            address: format!("{}/hooks", server.uri()),
            body: body.into(),
            headers_json: "{}".into(),
            shared_secret: Some("sec".into()),
        })
        .await
        .unwrap();

        assert!(result.success);
        assert_eq!(result.status, Some(200));
    }

    #[tokio::test]
    async fn trigger_local_webhook_delivers_to_port() {
        use wiremock::matchers::{body_string, header, method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        let body = r#"{ "sampleField": "SampleValue" }"#;
        let headers = r#"{ "header": "Header Value" }"#;

        Mock::given(method("POST"))
            .and(path("/a/url/path"))
            .and(body_string(body))
            .and(header("header", "Header Value"))
            .and(header("Content-Type", "application/json"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let got = trigger_local_webhook(
            &format!("{}/a/url/path", server.uri()),
            body,
            headers,
        )
        .await
        .unwrap();
        assert!(got);
    }

    #[tokio::test]
    async fn trigger_local_webhook_notifies_failure() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/webhooks"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let got = trigger_local_webhook(
            &format!("{}/api/webhooks", server.uri()),
            r#"{ "sampleField": "SampleValue" }"#,
            r#"{ "header": "Header Value" }"#,
        )
        .await
        .unwrap();
        assert!(!got);
    }
}
