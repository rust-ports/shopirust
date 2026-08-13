//! Orchestrate webhook trigger: collect flags, request sample, deliver locally if needed.
//!
//! Upstream: `trigger.ts`.

use super::delivery::{deliver_webhook_http, DeliverWebhookOptions};
use super::sample::{get_webhook_sample, SampleWebhook, SendSampleWebhookVariables, UserError, WebhookSampleClient};
use super::trigger_flags::DELIVERY_METHOD_LOCALHOST;
use super::trigger_options::{
    collect_address_and_method, collect_api_version, collect_credentials, collect_topic,
    CredentialSources,
};
use crate::error::AppError;
use crate::prompts::Prompter;

#[derive(Debug, Clone, Default)]
pub struct WebhookTriggerOptions {
    pub topic: Option<String>,
    pub api_version: Option<String>,
    pub address: Option<String>,
    pub delivery_method: Option<String>,
    pub client_secret: Option<String>,
    pub client_id: Option<String>,
    pub remote_secret: Option<String>,
    pub remote_api_key: Option<String>,
    pub remote_app_title: Option<String>,
    pub config_file: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebhookTriggerResult {
    pub message: String,
    pub success: bool,
    pub sample: Option<SampleWebhook>,
}

/// Resolve flags (prompting when omitted), request a sample, and deliver it.
///
/// Localhost POSTs from the CLI. HTTP / Pub/Sub / EventBridge are enqueued by the
/// webhooks GraphQL `cliTesting` mutation.
pub async fn webhook_trigger(
    options: WebhookTriggerOptions,
    client: &dyn WebhookSampleClient,
    prompter: &dyn Prompter,
) -> Result<WebhookTriggerResult, AppError> {
    let api_version =
        collect_api_version(client, options.api_version.as_deref(), prompter).await?;
    let topic = collect_topic(client, &api_version, options.topic.as_deref(), prompter).await?;
    let (address, delivery_method) = collect_address_and_method(
        options.delivery_method.as_deref(),
        options.address.as_deref(),
        prompter,
    )?;
    let (credentials, config_info) = collect_credentials(
        &CredentialSources {
            client_secret: options.client_secret.clone(),
            client_id: options.client_id.clone(),
            remote_secret: options.remote_secret.clone(),
            remote_api_key: options.remote_api_key.clone(),
            remote_app_title: options.remote_app_title.clone(),
            config_file: options.config_file.clone(),
        },
        &delivery_method,
    )?;

    let variables = SendSampleWebhookVariables {
        topic,
        api_version,
        address: address.clone(),
        delivery_method: delivery_method.clone(),
        shared_secret: credentials.client_secret,
        api_key: credentials.api_key,
    };
    let sample = get_webhook_sample(client, &variables).await?;

    if !sample.success {
        return Ok(WebhookTriggerResult {
            message: format!("Request errors:\n{}", format_errors(&sample.user_errors)),
            success: false,
            sample: Some(sample),
        });
    }

    if delivery_method == DELIVERY_METHOD_LOCALHOST {
        let delivered = deliver_webhook_http(DeliverWebhookOptions {
            address,
            body: sample.sample_payload.clone(),
            headers_json: sample.headers.clone(),
            shared_secret: None,
        })
        .await;

        return match delivered {
            Ok(result) if result.success => Ok(WebhookTriggerResult {
                message: prefixed(config_info, "Localhost delivery sucessful"),
                success: true,
                sample: Some(sample),
            }),
            Ok(_) | Err(_) => Ok(WebhookTriggerResult {
                message: prefixed(config_info, "Localhost delivery failed"),
                success: false,
                sample: Some(sample),
            }),
        };
    }

    if sample.sample_payload == "{}" {
        return Ok(WebhookTriggerResult {
            message: prefixed(config_info, "Webhook has been enqueued for delivery"),
            success: true,
            sample: Some(sample),
        });
    }

    Ok(WebhookTriggerResult {
        message: prefixed(config_info, "Webhook sample delivered"),
        success: true,
        sample: Some(sample),
    })
}

fn prefixed(info: Option<String>, message: &str) -> String {
    match info {
        Some(info) => format!("{info}\n{message}"),
        None => message.to_string(),
    }
}

fn format_errors(errors: &[UserError]) -> String {
    let mut lines = Vec::new();
    for element in errors {
        match serde_json::from_str::<Vec<String>>(&element.message) {
            Ok(msgs) => {
                lines.push(
                    msgs.into_iter()
                        .map(|msg| format!("  · {msg}"))
                        .collect::<Vec<_>>()
                        .join("\n"),
                );
            }
            Err(_) => {
                return serde_json::to_string(errors).unwrap_or_else(|_| format!("{errors:?}"));
            }
        }
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prompts::InjectedPrompter;
    use crate::services::webhook::sample::{MockWebhookClient, UserError};
    use wiremock::matchers::{body_string, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const SAMPLE_PAYLOAD: &str = r#"{ "sampleField": "SampleValue" }"#;
    const SAMPLE_HEADERS: &str = r#"{ "header": "Header Value" }"#;
    const A_TOPIC: &str = "A_TOPIC";
    const A_VERSION: &str = "A_VERSION";
    const A_SECRET: &str = "A_SECRET";
    const AN_API_KEY: &str = "AN_API_KEY";
    const AN_ADDRESS: &str = "https://example.org";
    const AN_EVENTBRIDGE_ADDRESS: &str =
        "arn:aws:events:us-east-3::event-source/aws.partner/shopify.com/12/source";

    fn sample_flags(address: &str, method: &str) -> WebhookTriggerOptions {
        WebhookTriggerOptions {
            topic: Some(A_TOPIC.into()),
            api_version: Some(A_VERSION.into()),
            delivery_method: Some(method.into()),
            client_secret: Some(A_SECRET.into()),
            address: Some(address.into()),
            ..Default::default()
        }
    }

    fn client_with_lists(sample: SampleWebhook) -> MockWebhookClient {
        MockWebhookClient::with_lists(vec![A_VERSION.into()], vec![A_TOPIC.into()]).with_sample(sample)
    }

    fn localhost_url(mock_uri: &str, path: &str) -> String {
        let url = url::Url::parse(mock_uri).expect("mock uri");
        let port = url.port().expect("mock port");
        format!("http://localhost:{port}{path}")
    }

    #[tokio::test]
    async fn notifies_about_request_errors() {
        let sample = SampleWebhook {
            sample_payload: "{}".into(),
            headers: "{}".into(),
            success: false,
            user_errors: vec![
                UserError {
                    message: r#"["Some error"]"#.into(),
                    fields: vec!["field1".into()],
                },
                UserError {
                    message: r#"["Another error"]"#.into(),
                    fields: vec!["field2".into()],
                },
            ],
        };
        let client = client_with_lists(sample);
        let p = InjectedPrompter::new();
        let result = webhook_trigger(sample_flags(AN_ADDRESS, "http"), &client, &p)
            .await
            .unwrap();
        assert!(!result.success);
        assert!(result.message.contains("Request errors:"));
        assert!(result.message.contains("  · Some error"));
        assert!(result.message.contains("  · Another error"));
        assert_eq!(*client.versions_calls.lock().unwrap(), 1);
        assert_eq!(client.topics_calls.lock().unwrap().as_slice(), &[A_VERSION]);
    }

    #[tokio::test]
    async fn safe_notification_for_unexpected_request_errors() {
        let sample = SampleWebhook {
            sample_payload: "{}".into(),
            headers: "{}".into(),
            success: false,
            user_errors: vec![
                UserError {
                    message: "Something not JSON".into(),
                    fields: vec!["field1".into()],
                },
                UserError {
                    message: "Another invalid JSON".into(),
                    fields: vec!["field2".into()],
                },
            ],
        };
        let client = client_with_lists(sample.clone());
        let p = InjectedPrompter::new();
        let result = webhook_trigger(sample_flags(AN_ADDRESS, "http"), &client, &p)
            .await
            .unwrap();
        assert!(!result.success);
        assert!(result.message.contains("Request errors:"));
        assert!(result.message.contains("Something not JSON"));
    }

    #[tokio::test]
    async fn notifies_about_real_delivery_being_sent() {
        let client = client_with_lists(MockWebhookClient::success_enqueued());
        let p = InjectedPrompter::new();
        let result = webhook_trigger(sample_flags(AN_ADDRESS, "http"), &client, &p)
            .await
            .unwrap();
        assert!(result.success);
        assert!(result.message.contains("Webhook has been enqueued for delivery"));
        let calls = client.send_calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].topic, A_TOPIC);
        assert_eq!(calls[0].delivery_method, "http");
        assert_eq!(calls[0].address, AN_ADDRESS);
        assert_eq!(calls[0].shared_secret, A_SECRET);
        assert_eq!(calls[0].api_version, A_VERSION);
        assert!(calls[0].api_key.is_none());
    }

    #[tokio::test]
    async fn retrieves_api_key_when_missing_for_event_bridge() {
        let client = client_with_lists(MockWebhookClient::success_enqueued());
        let p = InjectedPrompter::new();
        let mut flags = sample_flags(AN_EVENTBRIDGE_ADDRESS, "event-bridge");
        flags.remote_api_key = Some(AN_API_KEY.into());
        flags.remote_secret = Some(A_SECRET.into());
        let result = webhook_trigger(flags, &client, &p).await.unwrap();
        assert!(result.success);
        let calls = client.send_calls.lock().unwrap();
        assert_eq!(calls[0].api_key.as_deref(), Some(AN_API_KEY));
    }

    #[tokio::test]
    async fn uses_the_passed_api_key_for_event_bridge() {
        let client = client_with_lists(MockWebhookClient::success_enqueued());
        let p = InjectedPrompter::new();
        let mut flags = sample_flags(AN_EVENTBRIDGE_ADDRESS, "event-bridge");
        flags.client_id = Some("clientId".into());
        let result = webhook_trigger(flags, &client, &p).await.unwrap();
        assert!(result.success);
        let calls = client.send_calls.lock().unwrap();
        assert_eq!(calls[0].api_key.as_deref(), Some("clientId"));
    }

    #[tokio::test]
    async fn notifies_about_real_event_bridge_delivery() {
        let client = client_with_lists(MockWebhookClient::success_enqueued());
        let p = InjectedPrompter::new();
        let mut flags = sample_flags(AN_EVENTBRIDGE_ADDRESS, "event-bridge");
        flags.client_id = Some(AN_API_KEY.into());
        let result = webhook_trigger(flags, &client, &p).await.unwrap();
        assert!(result.success);
        assert!(result.message.contains("Webhook has been enqueued for delivery"));
        let calls = client.send_calls.lock().unwrap();
        assert_eq!(calls[0].delivery_method, "event-bridge");
        assert_eq!(calls[0].address, AN_EVENTBRIDGE_ADDRESS);
        assert_eq!(calls[0].api_key.as_deref(), Some(AN_API_KEY));
    }

    #[tokio::test]
    async fn delivers_to_localhost() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/a/url/path"))
            .and(body_string(SAMPLE_PAYLOAD))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let local = localhost_url(&server.uri(), "/a/url/path");
        let client = client_with_lists(MockWebhookClient::success_direct(
            SAMPLE_PAYLOAD,
            SAMPLE_HEADERS,
        ));
        let p = InjectedPrompter::new();
        let result = webhook_trigger(sample_flags(&local, "http"), &client, &p)
            .await
            .unwrap();
        assert!(result.success);
        assert!(result.message.contains("Localhost delivery sucessful"));
        let calls = client.send_calls.lock().unwrap();
        assert_eq!(calls[0].delivery_method, "localhost");
        assert_eq!(calls[0].address, local);
    }

    #[tokio::test]
    async fn shows_error_if_localhost_is_not_ready() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/a/url/path"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let local = localhost_url(&server.uri(), "/a/url/path");
        let client = client_with_lists(MockWebhookClient::success_direct(
            SAMPLE_PAYLOAD,
            SAMPLE_HEADERS,
        ));
        let p = InjectedPrompter::new();
        let result = webhook_trigger(sample_flags(&local, "http"), &client, &p)
            .await
            .unwrap();
        assert!(!result.success);
        assert!(result.message.contains("Localhost delivery failed"));
    }

    #[tokio::test]
    async fn rejects_unknown_address_without_method() {
        let client = client_with_lists(MockWebhookClient::success_enqueued());
        let p = InjectedPrompter::new();
        let err = webhook_trigger(
            WebhookTriggerOptions {
                topic: Some(A_TOPIC.into()),
                api_version: Some(A_VERSION.into()),
                address: Some("ftp://example.com".into()),
                delivery_method: None,
                client_secret: Some(A_SECRET.into()),
                ..Default::default()
            },
            &client,
            &p,
        )
        .await
        .unwrap_err();
        assert!(
            err.to_string().contains("delivery method")
                || err.to_string().contains("Non-interactive")
                || err.to_string().contains("Can't deliver")
        );
    }

    #[tokio::test]
    async fn pubsub_enqueues_remote_delivery() {
        let client = client_with_lists(MockWebhookClient::success_enqueued());
        let p = InjectedPrompter::new();
        let result = webhook_trigger(
            sample_flags("pubsub://project:topic", "google-pub-sub"),
            &client,
            &p,
        )
        .await
        .unwrap();
        assert!(result.success);
        assert!(result.message.contains("enqueued"));
        assert_eq!(
            client.send_calls.lock().unwrap()[0].delivery_method,
            "google-pub-sub"
        );
    }
}
