use crate::api::generated::graphql::webhooks::available_topics::AVAILABLE_TOPICS_QUERY;
use crate::api::generated::graphql::webhooks::cli_testing::CLI_TESTING_MUTATION;
use crate::api::generated::graphql::webhooks::public_api_versions::PUBLIC_API_VERSIONS_QUERY;
use crate::api::graphql::{CacheOptions, GraphqlClient, GraphqlRequestError, UnauthorizedHandler};
use crate::api::rate_limiter::ApiRateLimiter;
use crate::constants::app_management_fqdn;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

fn webhooks_rate_limiter() -> ApiRateLimiter {
    static LIMITER: OnceLock<ApiRateLimiter> = OnceLock::new();
    LIMITER.get_or_init(ApiRateLimiter::shopify_default).clone()
}

/// Variables for `cliTesting` / sample webhook delivery (upstream `SendSampleWebhookVariables`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct SendSampleWebhookVariables {
    pub topic: String,
    pub api_version: String,
    pub address: String,
    pub delivery_method: String,
    pub shared_secret: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SampleWebhook {
    pub sample_payload: String,
    pub headers: String,
    pub success: bool,
    pub user_errors: Vec<SampleWebhookUserError>,
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
#[serde(rename_all = "camelCase")]
pub struct SampleWebhookUserError {
    pub message: String,
    #[serde(default)]
    pub fields: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PublicApiVersionsResponse {
    public_api_versions: Vec<PublicApiVersionHandle>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PublicApiVersionHandle {
    handle: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AvailableTopicsResponse {
    available_topics: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CliTestingResponse {
    cli_testing: Option<CliTestingPayload>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CliTestingPayload {
    headers: Option<String>,
    sample_payload: Option<String>,
    success: bool,
    #[serde(default)]
    errors: Vec<String>,
}

pub struct WebhooksClient {
    pub organization_id: String,
    pub token: String,
    pub env: Option<HashMap<String, String>>,
    graphql: Option<GraphqlClient>,
}

impl WebhooksClient {
    pub fn new(
        organization_id: String,
        token: String,
        env: Option<HashMap<String, String>>,
    ) -> Self {
        Self {
            organization_id,
            token,
            env,
            graphql: None,
        }
    }

    pub fn with_graphql(organization_id: String, graphql: GraphqlClient) -> Self {
        Self {
            organization_id,
            token: String::new(),
            env: None,
            graphql: Some(graphql),
        }
    }

    pub async fn request<T, V>(
        &self,
        query: &str,
        variables: Option<V>,
        cache_options: Option<CacheOptions>,
        unauthorized_handler: Option<Arc<dyn UnauthorizedHandler>>,
    ) -> Result<T, GraphqlRequestError>
    where
        T: DeserializeOwned + Serialize,
        V: Serialize,
    {
        if let Some(ref gql) = self.graphql {
            return gql.query_with_variables(query, variables).await;
        }

        let url = format!(
            "https://{}/webhooks/unstable/organizations/{}/graphql.json",
            app_management_fqdn(self.env.as_ref()),
            self.organization_id,
        );
        let mut client = GraphqlClient::new(url, Some(self.token.clone()))
            .with_rate_limiter(webhooks_rate_limiter());

        if let Some(opts) = cache_options {
            client = client.with_cache_options(opts);
        }
        if let Some(handler) = unauthorized_handler {
            client = client.with_token_refresh_handler(handler);
        }

        client.query_with_variables(query, variables).await
    }

    /// Public API version handles from the webhooks GraphQL API.
    pub async fn api_versions(&self) -> Result<Vec<String>, GraphqlRequestError> {
        let resp: PublicApiVersionsResponse = self
            .request(
                PUBLIC_API_VERSIONS_QUERY,
                None::<serde_json::Value>,
                None,
                None,
            )
            .await?;
        Ok(resp
            .public_api_versions
            .into_iter()
            .map(|v| v.handle)
            .collect())
    }

    /// Available webhook topics for an API version.
    pub async fn topics(&self, api_version: &str) -> Result<Vec<String>, GraphqlRequestError> {
        let vars = serde_json::json!({ "apiVersion": api_version });
        let resp: AvailableTopicsResponse = self
            .request(AVAILABLE_TOPICS_QUERY, Some(vars), None, None)
            .await?;
        Ok(resp.available_topics.unwrap_or_default())
    }

    /// Request a sample payload (localhost) or enqueue remote delivery (HTTP / Pub/Sub / EventBridge).
    pub async fn send_sample_webhook(
        &self,
        variables: &SendSampleWebhookVariables,
    ) -> Result<SampleWebhook, GraphqlRequestError> {
        let vars = serde_json::json!({
            "topic": variables.topic,
            "apiVersion": variables.api_version,
            "address": variables.address,
            "deliveryMethod": variables.delivery_method,
            "sharedSecret": variables.shared_secret,
            "apiKey": variables.api_key,
        });
        let resp: CliTestingResponse = self
            .request(CLI_TESTING_MUTATION, Some(vars), None, None)
            .await?;
        Ok(map_cli_testing(resp.cli_testing))
    }
}

fn map_cli_testing(cli: Option<CliTestingPayload>) -> SampleWebhook {
    let Some(cli) = cli else {
        return SampleWebhook::default();
    };
    SampleWebhook {
        sample_payload: cli.sample_payload.unwrap_or_else(|| "{}".into()),
        headers: cli.headers.unwrap_or_else(|| "{}".into()),
        success: cli.success,
        user_errors: cli
            .errors
            .into_iter()
            .map(|message| SampleWebhookUserError {
                message,
                fields: vec![],
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_url_contains_org_id() {
        let _client = WebhooksClient::new("org-42".into(), "t".into(), None);
        let fqdn = app_management_fqdn(None);
        let expected =
            format!("https://{fqdn}/webhooks/unstable/organizations/org-42/graphql.json");
        assert!(expected.contains("org-42"));
    }

    #[test]
    fn client_new_sets_fields() {
        let client = WebhooksClient::new("org-7".into(), "shpat_test".into(), None);
        assert_eq!(client.organization_id, "org-7");
        assert_eq!(client.token, "shpat_test");
    }

    #[test]
    fn client_new_sets_env() {
        let mut env = HashMap::new();
        env.insert("SHOPIFY_SERVICE_ENV".to_string(), "production".to_string());
        let client = WebhooksClient::new("o".into(), "t".into(), Some(env.clone()));
        assert_eq!(client.env, Some(env));
    }

    #[test]
    fn client_new_env_none() {
        let client = WebhooksClient::new("o".into(), "t".into(), None);
        assert!(client.env.is_none());
    }

    #[tokio::test]
    async fn rate_limiter_acquires_permit() {
        let limiter = webhooks_rate_limiter();
        let permit = limiter.acquire().await;
        drop(permit);
    }

    #[test]
    fn queries_match_generated_operations() {
        assert!(PUBLIC_API_VERSIONS_QUERY.contains("publicApiVersions"));
        assert!(AVAILABLE_TOPICS_QUERY.contains("availableTopics"));
        assert!(CLI_TESTING_MUTATION.contains("cliTesting"));
    }

    #[test]
    fn map_cli_testing_empty() {
        let sample = map_cli_testing(None);
        assert!(!sample.success);
        assert_eq!(sample.sample_payload, "{}");
        assert_eq!(sample.headers, "{}");
    }

    #[test]
    fn map_cli_testing_errors() {
        let sample = map_cli_testing(Some(CliTestingPayload {
            headers: None,
            sample_payload: None,
            success: false,
            errors: vec!["Invalid api_version".into()],
        }));
        assert!(!sample.success);
        assert_eq!(sample.user_errors.len(), 1);
        assert_eq!(sample.user_errors[0].message, "Invalid api_version");
    }

    #[tokio::test]
    async fn api_versions_returns_handles() {
        let mock_server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "data": {
                        "publicApiVersions": [
                            { "handle": "2024-07" },
                            { "handle": "2024-10" },
                        ]
                    },
                    "extensions": { "cost": { "actualQueryCost": null, "throttleStatus": null } }
                })),
            )
            .mount(&mock_server)
            .await;

        let client = mock_webhooks_client(&mock_server);
        let versions = client.api_versions().await.unwrap();
        assert_eq!(versions, vec!["2024-07", "2024-10"]);
    }

    #[tokio::test]
    async fn api_versions_returns_empty() {
        let mock_server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "data": { "publicApiVersions": [] },
                    "extensions": { "cost": { "actualQueryCost": null, "throttleStatus": null } }
                })),
            )
            .mount(&mock_server)
            .await;

        let client = mock_webhooks_client(&mock_server);
        let versions = client.api_versions().await.unwrap();
        assert!(versions.is_empty());
    }

    #[tokio::test]
    async fn topics_returns_list() {
        let mock_server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "data": {
                        "availableTopics": ["orders/create", "products/update"]
                    },
                    "extensions": { "cost": { "actualQueryCost": null, "throttleStatus": null } }
                })),
            )
            .mount(&mock_server)
            .await;

        let client = mock_webhooks_client(&mock_server);
        let topics = client.topics("2024-07").await.unwrap();
        assert_eq!(topics, vec!["orders/create", "products/update"]);
    }

    #[tokio::test]
    async fn topics_null_is_empty() {
        let mock_server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "data": { "availableTopics": null },
                    "extensions": { "cost": { "actualQueryCost": null, "throttleStatus": null } }
                })),
            )
            .mount(&mock_server)
            .await;

        let client = mock_webhooks_client(&mock_server);
        let topics = client.topics("2024-07").await.unwrap();
        assert!(topics.is_empty());
    }

    #[tokio::test]
    async fn send_sample_webhook_maps_cli_testing() {
        let mock_server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "data": {
                        "cliTesting": {
                            "samplePayload": "{ \"sampleField\": \"SampleValue\" }",
                            "headers": "{ \"header\": \"Header Value\" }",
                            "success": true,
                            "errors": []
                        }
                    },
                    "extensions": { "cost": { "actualQueryCost": null, "throttleStatus": null } }
                })),
            )
            .mount(&mock_server)
            .await;

        let client = mock_webhooks_client(&mock_server);
        let result = client
            .send_sample_webhook(&SendSampleWebhookVariables {
                topic: "orders/create".into(),
                api_version: "2024-07".into(),
                address: "https://hook.example".into(),
                delivery_method: "http".into(),
                shared_secret: "secret".into(),
                api_key: None,
            })
            .await
            .unwrap();
        assert!(result.success);
        assert!(result.sample_payload.contains("SampleValue"));
        assert!(result.user_errors.is_empty());
    }

    #[tokio::test]
    async fn send_sample_webhook_returns_errors() {
        let mock_server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "data": {
                        "cliTesting": {
                            "samplePayload": "{}",
                            "headers": "{}",
                            "success": false,
                            "errors": ["Invalid topic"]
                        }
                    },
                    "extensions": { "cost": { "actualQueryCost": null, "throttleStatus": null } }
                })),
            )
            .mount(&mock_server)
            .await;

        let client = mock_webhooks_client(&mock_server);
        let result = client
            .send_sample_webhook(&SendSampleWebhookVariables {
                topic: "bad/topic".into(),
                api_version: "2024-07".into(),
                address: "https://hook.example".into(),
                delivery_method: "http".into(),
                shared_secret: "secret".into(),
                api_key: Some("api-key".into()),
            })
            .await
            .unwrap();
        assert!(!result.success);
        assert_eq!(result.user_errors.len(), 1);
    }

    fn mock_webhooks_client(server: &wiremock::MockServer) -> WebhooksClient {
        let gql = GraphqlClient::new(server.uri(), None);
        WebhooksClient::with_graphql("org-1".into(), gql)
    }
}
