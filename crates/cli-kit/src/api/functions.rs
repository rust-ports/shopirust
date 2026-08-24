use crate::api::graphql::{CacheOptions, GraphqlClient, GraphqlRequestError, UnauthorizedHandler};
use crate::api::rate_limiter::ApiRateLimiter;
use crate::constants::app_management_fqdn;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

fn functions_rate_limiter() -> ApiRateLimiter {
    static LIMITER: OnceLock<ApiRateLimiter> = OnceLock::new();
    LIMITER.get_or_init(ApiRateLimiter::shopify_default).clone()
}

const API_SCHEMA_DEFINITION_QUERY: &str = r#"
query ApiSchemaDefinition($apiKey: String!, $version: String!) {
  apiSchemaDefinition(apiKey: $apiKey, version: $version) {
    schema
    apiType
    hash
  }
}
"#;

const TARGET_SCHEMA_DEFINITION_QUERY: &str = r#"
query TargetSchemaDefinition($apiKey: String!, $version: String!, $target: String!) {
  targetSchemaDefinition(apiKey: $apiKey, version: $version, target: $target) {
    schema
    apiType
    hash
  }
}
"#;

const FUNCTION_ACTIVE_VERSION_QUERY: &str = r#"
query FunctionActiveVersion($apiKey: String!) {
  functionActiveVersion(apiKey: $apiKey) {
    id
    versionTag
    definition
    active
  }
}
"#;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaDefinition {
    pub schema: String,
    pub api_type: String,
    pub hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FunctionVersion {
    pub id: String,
    pub version_tag: Option<String>,
    pub definition: Option<serde_json::Value>,
    pub active: bool,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ApiSchemaDefinitionResponse {
    api_schema_definition: Option<SchemaDefinition>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct TargetSchemaDefinitionResponse {
    target_schema_definition: Option<SchemaDefinition>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct FunctionActiveVersionResponse {
    function_active_version: Option<FunctionVersion>,
}

pub struct FunctionsClient {
    pub organization_id: String,
    pub app_id: String,
    pub token: String,
    pub env: Option<HashMap<String, String>>,
    graphql: Option<GraphqlClient>,
}

impl FunctionsClient {
    pub fn new(
        organization_id: String,
        app_id: String,
        token: String,
        env: Option<HashMap<String, String>>,
    ) -> Self {
        Self {
            organization_id,
            app_id,
            token,
            env,
            graphql: None,
        }
    }

    pub fn with_graphql(organization_id: String, app_id: String, graphql: GraphqlClient) -> Self {
        Self {
            organization_id,
            app_id,
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
        T: DeserializeOwned,
        V: Serialize,
    {
        if let Some(ref gql) = self.graphql {
            return gql.query_with_variables(query, variables).await;
        }

        let url = format!(
            "https://{}/functions/unstable/organizations/{}/{}/graphql",
            app_management_fqdn(self.env.as_ref()),
            self.organization_id,
            self.app_id,
        );
        let mut client = GraphqlClient::new(url, Some(self.token.clone()))
            .with_rate_limiter(functions_rate_limiter());

        if let Some(opts) = cache_options {
            client = client.with_cache_options(opts);
        }
        if let Some(handler) = unauthorized_handler {
            client = client.with_token_refresh_handler(handler);
        }

        client.query_with_variables(query, variables).await
    }

    pub async fn api_schema_definition(
        &self,
        api_key: &str,
        version: &str,
    ) -> Result<Option<SchemaDefinition>, GraphqlRequestError> {
        let vars = serde_json::json!({ "apiKey": api_key, "version": version });
        let resp: ApiSchemaDefinitionResponse = self
            .request(API_SCHEMA_DEFINITION_QUERY, Some(vars), None, None)
            .await?;
        Ok(resp.api_schema_definition)
    }

    pub async fn target_schema_definition(
        &self,
        api_key: &str,
        version: &str,
        target: &str,
    ) -> Result<Option<SchemaDefinition>, GraphqlRequestError> {
        let vars = serde_json::json!({ "apiKey": api_key, "version": version, "target": target });
        let resp: TargetSchemaDefinitionResponse = self
            .request(TARGET_SCHEMA_DEFINITION_QUERY, Some(vars), None, None)
            .await?;
        Ok(resp.target_schema_definition)
    }

    pub async fn function_active_version(
        &self,
        api_key: &str,
    ) -> Result<Option<FunctionVersion>, GraphqlRequestError> {
        let vars = serde_json::json!({ "apiKey": api_key });
        let resp: FunctionActiveVersionResponse = self
            .request(FUNCTION_ACTIVE_VERSION_QUERY, Some(vars), None, None)
            .await?;
        Ok(resp.function_active_version)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_url_contains_org_and_app() {
        let _client = FunctionsClient::new("org-1".into(), "app-1".into(), "t".into(), None);
        let fqdn = app_management_fqdn(None);
        let expected =
            format!("https://{fqdn}/functions/unstable/organizations/org-1/app-1/graphql");
        assert!(expected.contains("org-1"));
        assert!(expected.contains("app-1"));
    }

    #[test]
    fn client_new_sets_fields() {
        let client =
            FunctionsClient::new("org-42".into(), "app-7".into(), "shpat_test".into(), None);
        assert_eq!(client.organization_id, "org-42");
        assert_eq!(client.app_id, "app-7");
        assert_eq!(client.token, "shpat_test");
    }

    #[test]
    fn client_new_sets_env() {
        let mut env = HashMap::new();
        env.insert("SHOPIFY_SERVICE_ENV".to_string(), "production".to_string());
        let client = FunctionsClient::new("o".into(), "a".into(), "t".into(), Some(env.clone()));
        assert_eq!(client.env, Some(env));
    }

    #[test]
    fn client_new_env_none() {
        let client = FunctionsClient::new("o".into(), "a".into(), "t".into(), None);
        assert!(client.env.is_none());
    }

    #[tokio::test]
    async fn rate_limiter_acquires_permit() {
        let limiter = functions_rate_limiter();
        let permit = limiter.acquire().await;
        drop(permit);
    }

    #[test]
    fn schema_definition_deserialize() {
        let json = serde_json::json!({
            "schema": "type Query { ping: String }",
            "apiType": "graphql",
            "hash": "abc123"
        });
        let sd: SchemaDefinition = serde_json::from_value(json).unwrap();
        assert_eq!(sd.hash, "abc123");
    }

    #[test]
    fn function_version_deserialize() {
        let json = serde_json::json!({
            "id": "fv-1",
            "versionTag": "v1",
            "definition": {"input": "String"},
            "active": true
        });
        let fv: FunctionVersion = serde_json::from_value(json).unwrap();
        assert!(fv.active);
        assert_eq!(fv.version_tag, Some("v1".into()));
    }

    #[test]
    fn api_schema_definition_has_query() {
        assert!(API_SCHEMA_DEFINITION_QUERY.contains("apiSchemaDefinition"));
    }

    #[test]
    fn target_schema_definition_has_query() {
        assert!(TARGET_SCHEMA_DEFINITION_QUERY.contains("targetSchemaDefinition"));
    }

    #[test]
    fn function_active_version_has_query() {
        assert!(FUNCTION_ACTIVE_VERSION_QUERY.contains("functionActiveVersion"));
    }

    // ===== Wiremock Tests =====

    #[tokio::test]
    async fn api_schema_definition_returns_schema() {
        let mock_server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "data": {
                        "apiSchemaDefinition": {
                            "schema": "type Query { ping: String }",
                            "apiType": "graphql",
                            "hash": "abc123"
                        }
                    },
                    "extensions": { "cost": { "actualQueryCost": null, "throttleStatus": null } }
                })),
            )
            .mount(&mock_server)
            .await;

        let client = mock_functions_client(&mock_server);
        let result = client
            .api_schema_definition("key-1", "2024-07")
            .await
            .unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().hash, "abc123");
    }

    #[tokio::test]
    async fn api_schema_definition_returns_none_when_not_found() {
        let mock_server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "data": { "apiSchemaDefinition": null },
                    "extensions": { "cost": { "actualQueryCost": null, "throttleStatus": null } }
                })),
            )
            .mount(&mock_server)
            .await;

        let client = mock_functions_client(&mock_server);
        let result = client
            .api_schema_definition("key-1", "2024-07")
            .await
            .unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn target_schema_definition_returns_schema() {
        let mock_server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "data": {
                        "targetSchemaDefinition": {
                            "schema": "type Query { ping: String }",
                            "apiType": "graphql",
                            "hash": "def456"
                        }
                    },
                    "extensions": { "cost": { "actualQueryCost": null, "throttleStatus": null } }
                })),
            )
            .mount(&mock_server)
            .await;

        let client = mock_functions_client(&mock_server);
        let result = client
            .target_schema_definition("key-1", "2024-07", "my-target")
            .await
            .unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().hash, "def456");
    }

    #[tokio::test]
    async fn function_active_version_returns_version() {
        let mock_server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "data": {
                        "functionActiveVersion": {
                            "id": "fv-1",
                            "versionTag": "v1",
                            "definition": {"input": "String"},
                            "active": true
                        }
                    },
                    "extensions": { "cost": { "actualQueryCost": null, "throttleStatus": null } }
                })),
            )
            .mount(&mock_server)
            .await;

        let client = mock_functions_client(&mock_server);
        let result = client.function_active_version("key-1").await.unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().id, "fv-1");
    }

    fn mock_functions_client(server: &wiremock::MockServer) -> FunctionsClient {
        let gql = GraphqlClient::new(server.uri(), None);
        FunctionsClient::with_graphql("org-1".into(), "app-1".into(), gql)
    }
}
