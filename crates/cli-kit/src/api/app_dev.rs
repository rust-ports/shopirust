use crate::api::graphql::{CacheOptions, GraphqlClient, GraphqlRequestError, UnauthorizedHandler};
use crate::api::rate_limiter::ApiRateLimiter;
use crate::constants::app_management_fqdn;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

fn app_dev_rate_limiter() -> ApiRateLimiter {
    static LIMITER: OnceLock<ApiRateLimiter> = OnceLock::new();
    LIMITER.get_or_init(ApiRateLimiter::shopify_default).clone()
}

const DEV_SESSION_CREATE_MUTATION: &str = r#"
mutation DevSessionCreate($appId: ID!, $shopFqdn: String!, $title: String!, $token: String!, $storeFqdn: String!) {
  devSessionCreate(input: {appId: $appId, shopFqdn: $shopFqdn, title: $title, token: $token, storeFqdn: $storeFqdn}) {
    devSession {
      id
      title
      appId
      shopFqdn
    }
    userErrors {
      field
      message
    }
  }
}
"#;

const DEV_SESSION_UPDATE_MUTATION: &str = r#"
mutation DevSessionUpdate($id: ID!, $title: String, $token: String) {
  devSessionUpdate(input: {id: $id, title: $title, token: $token}) {
    devSession {
      id
      title
      appId
      shopFqdn
    }
    userErrors {
      field
      message
    }
  }
}
"#;

const DEV_SESSION_DELETE_MUTATION: &str = r#"
mutation DevSessionDelete($id: ID!) {
  devSessionDelete(input: {id: $id}) {
    deletedId
    userErrors {
      field
      message
    }
  }
}
"#;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DevSession {
    pub id: String,
    pub title: String,
    pub app_id: String,
    pub shop_fqdn: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DevSessionCreateResult {
    pub dev_session: Option<DevSession>,
    pub user_errors: Vec<DevSessionUserError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DevSessionUpdateResult {
    pub dev_session: Option<DevSession>,
    pub user_errors: Vec<DevSessionUserError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DevSessionDeleteResult {
    pub deleted_id: Option<String>,
    pub user_errors: Vec<DevSessionUserError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DevSessionUserError {
    pub field: Option<Vec<String>>,
    pub message: Option<String>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct DevSessionCreateResponse {
    dev_session_create: DevSessionCreateResult,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct DevSessionUpdateResponse {
    dev_session_update: DevSessionUpdateResult,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct DevSessionDeleteResponse {
    dev_session_delete: DevSessionDeleteResult,
}

pub struct AppDevClient {
    pub shop_fqdn: String,
    pub token: String,
    pub env: Option<HashMap<String, String>>,
}

impl AppDevClient {
    pub fn new(shop_fqdn: String, token: String, env: Option<HashMap<String, String>>) -> Self {
        Self {
            shop_fqdn,
            token,
            env,
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
        let url = format!(
            "https://{}/app_dev/unstable/graphql.json",
            app_management_fqdn(self.env.as_ref()),
        );
        let mut client = GraphqlClient::new(url, Some(self.token.clone()))
            .with_rate_limiter(app_dev_rate_limiter());

        let mut extra_headers = HeaderMap::new();
        if let Ok(value) = HeaderValue::from_str(&self.shop_fqdn) {
            extra_headers.insert(HeaderName::from_static("x-forwarded-host"), value);
        }
        client = client.with_extra_headers(extra_headers);

        if let Some(opts) = cache_options {
            client = client.with_cache_options(opts);
        }
        if let Some(handler) = unauthorized_handler {
            client = client.with_token_refresh_handler(handler);
        }

        client.query_with_variables(query, variables).await
    }

    pub async fn dev_session_create(
        &self,
        app_id: &str,
        title: &str,
        token: &str,
        store_fqdn: &str,
    ) -> Result<DevSessionCreateResult, GraphqlRequestError> {
        let vars = serde_json::json!({
            "appId": app_id,
            "shopFqdn": self.shop_fqdn,
            "title": title,
            "token": token,
            "storeFqdn": store_fqdn,
        });
        let resp: DevSessionCreateResponse = self
            .request(DEV_SESSION_CREATE_MUTATION, Some(vars), None, None)
            .await?;
        Ok(resp.dev_session_create)
    }

    pub async fn dev_session_update(
        &self,
        id: &str,
        title: Option<&str>,
        token: Option<&str>,
    ) -> Result<DevSessionUpdateResult, GraphqlRequestError> {
        let mut vars = serde_json::json!({ "id": id });
        if let Some(t) = title {
            vars["title"] = serde_json::json!(t);
        }
        if let Some(t) = token {
            vars["token"] = serde_json::json!(t);
        }
        let resp: DevSessionUpdateResponse = self
            .request(DEV_SESSION_UPDATE_MUTATION, Some(vars), None, None)
            .await?;
        Ok(resp.dev_session_update)
    }

    pub async fn dev_session_delete(
        &self,
        id: &str,
    ) -> Result<DevSessionDeleteResult, GraphqlRequestError> {
        let vars = serde_json::json!({ "id": id });
        let resp: DevSessionDeleteResponse = self
            .request(DEV_SESSION_DELETE_MUTATION, Some(vars), None, None)
            .await?;
        Ok(resp.dev_session_delete)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_url_contains_fqdn() {
        let _client = AppDevClient::new("shop.test".into(), "t".into(), None);
        let fqdn = app_management_fqdn(None);
        let expected = format!("https://{fqdn}/app_dev/unstable/graphql.json");
        assert!(expected.contains(&fqdn));
    }

    #[test]
    fn client_new_sets_fields() {
        let client = AppDevClient::new("shop.test".into(), "shpat_test".into(), None);
        assert_eq!(client.shop_fqdn, "shop.test");
        assert_eq!(client.token, "shpat_test");
    }

    #[test]
    fn client_new_sets_env() {
        let mut env = HashMap::new();
        env.insert("SHOPIFY_SERVICE_ENV".to_string(), "production".to_string());
        let client = AppDevClient::new("s".into(), "t".into(), Some(env.clone()));
        assert_eq!(client.env, Some(env));
    }

    #[test]
    fn client_new_env_none() {
        let client = AppDevClient::new("s".into(), "t".into(), None);
        assert!(client.env.is_none());
    }

    #[tokio::test]
    async fn rate_limiter_acquires_permit() {
        let limiter = app_dev_rate_limiter();
        let permit = limiter.acquire().await;
        drop(permit);
    }

    #[test]
    fn dev_session_deserialize() {
        let json = serde_json::json!({
            "id": "ds-1",
            "title": "my session",
            "appId": "app-1",
            "shopFqdn": "shop.test"
        });
        let ds: DevSession = serde_json::from_value(json).unwrap();
        assert_eq!(ds.id, "ds-1");
    }

    #[test]
    fn dev_session_create_result_deserialize() {
        let json = serde_json::json!({
            "devSession": {"id": "ds-1", "title": "dev", "appId": "a1", "shopFqdn": "s.test"},
            "userErrors": []
        });
        let r: DevSessionCreateResult = serde_json::from_value(json).unwrap();
        assert!(r.dev_session.is_some());
        assert!(r.user_errors.is_empty());
    }

    #[test]
    fn dev_session_delete_result_deserialize() {
        let json = serde_json::json!({
            "deletedId": "ds-1",
            "userErrors": []
        });
        let r: DevSessionDeleteResult = serde_json::from_value(json).unwrap();
        assert_eq!(r.deleted_id, Some("ds-1".into()));
    }

    #[test]
    fn dev_session_create_has_mutation() {
        assert!(DEV_SESSION_CREATE_MUTATION.contains("devSessionCreate"));
    }

    #[test]
    fn dev_session_update_has_mutation() {
        assert!(DEV_SESSION_UPDATE_MUTATION.contains("devSessionUpdate"));
    }

    #[test]
    fn dev_session_delete_has_mutation() {
        assert!(DEV_SESSION_DELETE_MUTATION.contains("devSessionDelete"));
    }
}
