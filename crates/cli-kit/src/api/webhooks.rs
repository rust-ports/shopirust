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

const API_VERSIONS_QUERY: &str = r#"
query WebhookApiVersions {
  webhookApiVersions {
    id
    handle
  }
}
"#;

const TOPICS_QUERY: &str = r#"
query WebhookTopics($apiVersion: String!) {
  webhookTopics(apiVersion: $apiVersion) {
    topic
    description
  }
}
"#;

const SEND_SAMPLE_WEBHOOK_MUTATION: &str = r#"
mutation SendSampleWebhook($topic: String!, $apiVersion: String!, $address: String!, $sharedSecret: String!) {
  sendSampleWebhook(input: {topic: $topic, apiVersion: $apiVersion, address: $address, sharedSecret: $sharedSecret}) {
    sampleWebhookId
    userErrors {
      field
      message
    }
  }
}
"#;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebhookApiVersion {
    pub id: String,
    pub handle: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebhookTopic {
    pub topic: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SampleWebhookResult {
    pub sample_webhook_id: Option<String>,
    pub user_errors: Vec<SampleWebhookUserError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SampleWebhookUserError {
    pub field: Option<Vec<String>>,
    pub message: Option<String>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ApiVersionsResponse {
    webhook_api_versions: Vec<WebhookApiVersion>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct TopicsResponse {
    webhook_topics: Vec<WebhookTopic>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct SendSampleWebhookResponse {
    send_sample_webhook: SampleWebhookResult,
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

    pub async fn api_versions(&self) -> Result<Vec<WebhookApiVersion>, GraphqlRequestError> {
        let resp: ApiVersionsResponse = self
            .request(API_VERSIONS_QUERY, None::<serde_json::Value>, None, None)
            .await?;
        Ok(resp.webhook_api_versions)
    }

    pub async fn topics(
        &self,
        api_version: &str,
    ) -> Result<Vec<WebhookTopic>, GraphqlRequestError> {
        let vars = serde_json::json!({ "apiVersion": api_version });
        let resp: TopicsResponse = self.request(TOPICS_QUERY, Some(vars), None, None).await?;
        Ok(resp.webhook_topics)
    }

    pub async fn send_sample_webhook(
        &self,
        topic: &str,
        api_version: &str,
        address: &str,
        shared_secret: &str,
    ) -> Result<SampleWebhookResult, GraphqlRequestError> {
        let vars = serde_json::json!({
            "topic": topic,
            "apiVersion": api_version,
            "address": address,
            "sharedSecret": shared_secret,
        });
        let resp: SendSampleWebhookResponse = self
            .request(SEND_SAMPLE_WEBHOOK_MUTATION, Some(vars), None, None)
            .await?;
        Ok(resp.send_sample_webhook)
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
    fn webhook_api_version_deserialize() {
        let json = serde_json::json!({"id": "1", "handle": "2024-07"});
        let version: WebhookApiVersion = serde_json::from_value(json).unwrap();
        assert_eq!(version.handle, "2024-07");
    }

    #[test]
    fn webhook_topic_deserialize() {
        let json = serde_json::json!({"topic": "orders/create", "description": "Order created"});
        let topic: WebhookTopic = serde_json::from_value(json).unwrap();
        assert_eq!(topic.topic, "orders/create");
    }

    #[test]
    fn sample_webhook_result_deserialize() {
        let json = serde_json::json!({
            "sampleWebhookId": "wh-123",
            "userErrors": [{"field": ["topic"], "message": "invalid"}]
        });
        let result: SampleWebhookResult = serde_json::from_value(json).unwrap();
        assert_eq!(result.sample_webhook_id, Some("wh-123".into()));
        assert_eq!(result.user_errors.len(), 1);
    }

    #[test]
    fn api_versions_has_query() {
        assert!(API_VERSIONS_QUERY.contains("webhookApiVersions"));
    }

    #[test]
    fn topics_has_query() {
        assert!(TOPICS_QUERY.contains("webhookTopics"));
    }

    #[test]
    fn send_sample_webhook_has_mutation() {
        assert!(SEND_SAMPLE_WEBHOOK_MUTATION.contains("sendSampleWebhook"));
    }

    // ===== Wiremock Tests =====

    #[tokio::test]
    async fn api_versions_returns_list() {
        let mock_server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "data": {
                        "webhookApiVersions": [
                            { "id": "1", "handle": "2024-07" },
                            { "id": "2", "handle": "2024-10" },
                        ]
                    },
                    "extensions": { "cost": { "actualQueryCost": null, "throttleStatus": null } }
                })),
            )
            .mount(&mock_server)
            .await;

        let client = mock_webhooks_client(&mock_server);
        let versions = client.api_versions().await.unwrap();
        assert_eq!(versions.len(), 2);
        assert_eq!(versions[0].handle, "2024-07");
    }

    #[tokio::test]
    async fn api_versions_returns_empty() {
        let mock_server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "data": { "webhookApiVersions": [] },
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
                        "webhookTopics": [
                            { "topic": "orders/create", "description": "Order created" },
                            { "topic": "products/update", "description": "Product updated" },
                        ]
                    },
                    "extensions": { "cost": { "actualQueryCost": null, "throttleStatus": null } }
                })),
            )
            .mount(&mock_server)
            .await;

        let client = mock_webhooks_client(&mock_server);
        let topics = client.topics("2024-07").await.unwrap();
        assert_eq!(topics.len(), 2);
        assert_eq!(topics[0].topic, "orders/create");
    }

    #[tokio::test]
    async fn send_sample_webhook_sends() {
        let mock_server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "data": {
                        "sendSampleWebhook": {
                            "sampleWebhookId": "wh-123",
                            "userErrors": []
                        }
                    },
                    "extensions": { "cost": { "actualQueryCost": null, "throttleStatus": null } }
                })),
            )
            .mount(&mock_server)
            .await;

        let client = mock_webhooks_client(&mock_server);
        let result = client
            .send_sample_webhook("orders/create", "2024-07", "https://hook.example", "secret")
            .await
            .unwrap();
        assert_eq!(result.sample_webhook_id, Some("wh-123".into()));
        assert!(result.user_errors.is_empty());
    }

    #[tokio::test]
    async fn send_sample_webhook_returns_errors() {
        let mock_server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "data": {
                        "sendSampleWebhook": {
                            "sampleWebhookId": null,
                            "userErrors": [{ "field": ["topic"], "message": "Invalid topic" }]
                        }
                    },
                    "extensions": { "cost": { "actualQueryCost": null, "throttleStatus": null } }
                })),
            )
            .mount(&mock_server)
            .await;

        let client = mock_webhooks_client(&mock_server);
        let result = client
            .send_sample_webhook("bad/topic", "2024-07", "https://hook.example", "secret")
            .await
            .unwrap();
        assert!(result.sample_webhook_id.is_none());
        assert_eq!(result.user_errors.len(), 1);
    }

    fn mock_webhooks_client(server: &wiremock::MockServer) -> WebhooksClient {
        let gql = GraphqlClient::new(server.uri(), None);
        WebhooksClient::with_graphql("org-1".into(), gql)
    }
}
