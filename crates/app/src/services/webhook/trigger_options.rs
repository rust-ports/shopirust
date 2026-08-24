//! Collect webhook trigger flags, prompting when omitted.
//!
//! Upstream: `trigger-options.ts`.

use super::sample::{request_api_versions, request_topics, WebhookSampleClient};
use super::trigger_flags::{
    delivery_method_for_address, parse_api_version_flag, parse_topic_flag, validate_address_method,
    DELIVERY_METHOD_EVENTBRIDGE,
};
use crate::error::AppError;
use crate::prompts::webhook::{
    prompt_address, prompt_api_version, prompt_delivery_method, prompt_topic,
};
use crate::prompts::Prompter;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AppCredentials {
    pub client_secret: String,
    pub api_key: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct CredentialSources {
    pub client_secret: Option<String>,
    pub client_id: Option<String>,
    pub remote_secret: Option<String>,
    pub remote_api_key: Option<String>,
    pub remote_app_title: Option<String>,
    pub config_file: Option<String>,
}

/// Collect a secret / api-key pair using flag fallback then remote app secrets.
pub fn collect_credentials(
    sources: &CredentialSources,
    delivery_method: &str,
) -> Result<(AppCredentials, Option<String>), AppError> {
    if let Some(secret) = sources.client_secret.as_deref().filter(|s| !s.is_empty()) {
        if sources.client_id.is_some() || delivery_method != DELIVERY_METHOD_EVENTBRIDGE {
            return Ok((
                AppCredentials {
                    client_secret: secret.to_string(),
                    api_key: sources.client_id.clone(),
                },
                None,
            ));
        }
    }

    let info = sources.config_file.as_ref().map(|file| {
        format!(
            "Using {file} for default values: App: {}",
            sources.remote_app_title.as_deref().unwrap_or("app")
        )
    });

    let client_secret = sources
        .remote_secret
        .clone()
        .filter(|s| !s.is_empty())
        .or_else(|| sources.client_secret.clone())
        .ok_or_else(|| {
            AppError::message(
                "Client secret is required. Pass --client-secret or link an app with a secret key.",
            )
        })?;

    Ok((
        AppCredentials {
            client_secret,
            api_key: sources
                .remote_api_key
                .clone()
                .or_else(|| sources.client_id.clone()),
        },
        info,
    ))
}

/// Returns passed api-version or prompts for an existing one.
pub async fn collect_api_version(
    client: &dyn WebhookSampleClient,
    api_version: Option<&str>,
    prompter: &dyn Prompter,
) -> Result<String, AppError> {
    let api_versions = request_api_versions(client).await?;
    if let Some(version) = api_version.filter(|v| !v.is_empty()) {
        return parse_api_version_flag(version, &api_versions);
    }
    prompt_api_version(prompter, &api_versions)
}

/// Returns passed topic if valid or prompts for an existing one.
pub async fn collect_topic(
    client: &dyn WebhookSampleClient,
    api_version: &str,
    topic: Option<&str>,
    prompter: &dyn Prompter,
) -> Result<String, AppError> {
    let topics = request_topics(client, api_version).await?;
    if let Some(topic) = topic.filter(|t| !t.is_empty()) {
        return parse_topic_flag(topic, api_version, &topics);
    }
    prompt_topic(prompter, &topics)
}

/// Expects either undefined deliveryMethod-address pairs, undefined address, or a valid pair.
pub fn collect_address_and_method(
    delivery_method: Option<&str>,
    address: Option<&str>,
    prompter: &dyn Prompter,
) -> Result<(String, String), AppError> {
    let actual_method = match delivery_method.filter(|m| !m.is_empty()) {
        Some(method) => method.to_string(),
        None => match delivery_method_for_address(address).map(|m| m.as_str().to_string()) {
            Some(method) => method,
            None => prompt_delivery_method(prompter)?,
        },
    };
    let actual_address = match address.filter(|a| !a.is_empty()) {
        Some(addr) => addr.to_string(),
        None => prompt_address(prompter, &actual_method)?,
    };
    validate_address_method(&actual_address, &actual_method)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prompts::InjectedPrompter;
    use crate::services::webhook::sample::MockWebhookClient;

    const SECRET: &str = "A_SECRET";
    const API_KEY: &str = "AN_API_KEY";
    const ORGANIZATION_APP_SECRET: &str = "remote-secret";
    const ORGANIZATION_APP_KEY: &str = "remote-key";

    fn sources_with_flags() -> CredentialSources {
        CredentialSources {
            client_secret: Some(SECRET.into()),
            client_id: Some(API_KEY.into()),
            remote_secret: Some(ORGANIZATION_APP_SECRET.into()),
            remote_api_key: Some(ORGANIZATION_APP_KEY.into()),
            remote_app_title: Some("app1".into()),
            config_file: Some("shopify.app.toml".into()),
        }
    }

    #[tokio::test]
    async fn collect_api_version_uses_passed() {
        let client =
            MockWebhookClient::with_lists(vec!["2023-01".into(), "unstable".into()], vec![]);
        let p = InjectedPrompter::new();
        let version = collect_api_version(&client, Some("2023-01"), &p)
            .await
            .unwrap();
        assert_eq!(version, "2023-01");
    }

    #[tokio::test]
    async fn collect_api_version_prompts_when_unset() {
        let client =
            MockWebhookClient::with_lists(vec!["2023-01".into(), "unstable".into()], vec![]);
        let p = InjectedPrompter::new();
        p.push_select("2023-01");
        let version = collect_api_version(&client, None, &p).await.unwrap();
        assert_eq!(version, "2023-01");
        assert_eq!(*client.versions_calls.lock().unwrap(), 1);
    }

    #[tokio::test]
    async fn collect_topic_uses_passed_if_present() {
        let client = MockWebhookClient::with_lists(
            vec!["2023-01".into()],
            vec!["shop/redact".into(), "orders/create".into()],
        );
        let p = InjectedPrompter::new();
        let topic = collect_topic(&client, "2023-01", Some("shop/redact"), &p)
            .await
            .unwrap();
        assert_eq!(topic, "shop/redact");
    }

    #[tokio::test]
    async fn collect_topic_fails_if_unknown() {
        let client = MockWebhookClient::with_lists(
            vec!["2023-01".into()],
            vec!["shop/redact".into(), "orders/create".into()],
        );
        let p = InjectedPrompter::new();
        let err = collect_topic(&client, "2023-01", Some("unknown/topic"), &p)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("does not exist"));
    }

    #[tokio::test]
    async fn collect_topic_prompts_when_unset() {
        let client = MockWebhookClient::with_lists(
            vec!["unstable".into()],
            vec!["shop/redact".into(), "orders/create".into()],
        );
        let p = InjectedPrompter::new();
        p.push_select("orders/create");
        let topic = collect_topic(&client, "unstable", None, &p).await.unwrap();
        assert_eq!(topic, "orders/create");
        assert_eq!(client.topics_calls.lock().unwrap().len(), 1);
    }

    #[test]
    fn collect_address_uses_passed_pair() {
        let p = InjectedPrompter::new();
        let (address, method) =
            collect_address_and_method(Some("http"), Some("http://localhost"), &p).unwrap();
        assert_eq!(method, "localhost");
        assert_eq!(address, "http://localhost");
    }

    #[test]
    fn collect_address_prompts_when_method_passed() {
        let p = InjectedPrompter::new();
        p.push_text("http://localhost");
        let (address, method) = collect_address_and_method(Some("http"), None, &p).unwrap();
        assert_eq!(method, "localhost");
        assert_eq!(address, "http://localhost");
    }

    #[test]
    fn collect_address_prompts_both_when_none_passed() {
        let p = InjectedPrompter::new();
        p.push_select("http");
        p.push_text("https://example.org");
        let (address, method) = collect_address_and_method(None, None, &p).unwrap();
        assert_eq!(method, "http");
        assert_eq!(address, "https://example.org");
    }

    #[test]
    fn collect_address_infers_method_from_address() {
        let p = InjectedPrompter::new();
        let (address, method) =
            collect_address_and_method(None, Some("https://example.org"), &p).unwrap();
        assert_eq!(method, "http");
        assert_eq!(address, "https://example.org");
    }

    #[test]
    fn collect_credentials_uses_flag_values() {
        let (creds, info) = collect_credentials(&sources_with_flags(), "http").unwrap();
        assert_eq!(creds.client_secret, SECRET);
        assert_eq!(creds.api_key.as_deref(), Some(API_KEY));
        assert!(info.is_none());
    }

    #[test]
    fn collect_credentials_retrieves_remote_when_flag_missing() {
        let mut sources = sources_with_flags();
        sources.client_secret = None;
        let (creds, info) = collect_credentials(&sources, "http").unwrap();
        assert_eq!(creds.client_secret, ORGANIZATION_APP_SECRET);
        assert_eq!(creds.api_key.as_deref(), Some(ORGANIZATION_APP_KEY));
        let info = info.expect("config info");
        assert!(info.contains("shopify.app.toml"));
        assert!(info.contains("app1"));
    }

    #[test]
    fn collect_credentials_shows_config_when_secret_missing() {
        let sources = CredentialSources {
            client_secret: None,
            client_id: None,
            remote_secret: Some(ORGANIZATION_APP_SECRET.into()),
            remote_api_key: Some(ORGANIZATION_APP_KEY.into()),
            remote_app_title: Some("app1".into()),
            config_file: Some("shopify.app.toml".into()),
        };
        let (_, info) = collect_credentials(&sources, "http").unwrap();
        let info = info.unwrap();
        assert!(info.contains("shopify.app.toml"));
        assert!(info.contains("app1"));
    }

    #[test]
    fn collect_credentials_eventbridge_without_client_id_loads_remote() {
        let mut sources = sources_with_flags();
        sources.client_id = None;
        let (creds, info) = collect_credentials(&sources, DELIVERY_METHOD_EVENTBRIDGE).unwrap();
        assert_eq!(creds.client_secret, ORGANIZATION_APP_SECRET);
        assert_eq!(creds.api_key.as_deref(), Some(ORGANIZATION_APP_KEY));
        assert!(info.is_some());
    }
}
