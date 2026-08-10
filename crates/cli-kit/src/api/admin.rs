use crate::api::generated::graphql::admin::public_api_versions::{
    PublicApiVersionsResponse, PUBLIC_API_VERSIONS_QUERY,
};
use crate::api::graphql::{GraphqlClient, GraphqlRequestError};
use crate::api::rest_api_throttler::{RestClient, RestError, RestResponse};
use crate::error::{abort_error, bug_error, FatalError};
pub use crate::session::AdminSession;
use crate::util::retry::RetryConfig;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;

const THEME_KIT_ACCESS_DOMAIN: &str = "theme-kit-access.shopifyapps.com";

const METAFIELD_DEFINITIONS_QUERY: &str = r#"
query MetafieldDefinitions($ownerType: String!) {
  metafieldDefinitions(ownerType: $ownerType) {
    id
    name
    namespace
    key
    type {
      name
    }
    description
    pinnedPosition
  }
}
"#;

const ONLINE_STORE_PASSWORD_PROTECTION_QUERY: &str = r#"
query OnlineStorePasswordProtection {
  onlineStorePasswordProtection {
    enabled
    password
  }
}
"#;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Theme {
    pub id: i64,
    pub name: String,
    pub role: Option<String>,
    pub previewable: Option<bool>,
    pub processing: Option<bool>,
    pub theme_store_id: Option<i64>,
    pub admin_graphql_api_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeAsset {
    pub key: String,
    pub value: Option<String>,
    pub attachment: Option<String>,
    pub checksum: Option<String>,
    pub content_type: Option<String>,
    pub size: Option<i64>,
    pub public_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetafieldDefinition {
    pub id: String,
    pub name: String,
    pub namespace: String,
    pub key: String,
    pub r#type: MetafieldTypeInfo,
    pub description: Option<String>,
    pub pinned_position: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetafieldTypeInfo {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PasswordProtection {
    pub enabled: bool,
    pub password: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateThemeInput {
    pub name: String,
    pub source: Option<String>,
    pub role: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateThemeInput {
    pub name: Option<String>,
    pub role: Option<String>,
}

#[derive(Deserialize)]
struct ThemesWrapper {
    themes: Vec<Theme>,
}

#[derive(Deserialize)]
struct ThemeWrapper {
    theme: Theme,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct AssetsWrapper {
    asset: Option<ThemeAsset>,
    assets: Option<Vec<ThemeAsset>>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct MetafieldDefinitionsResponse {
    metafield_definitions: Vec<MetafieldDefinition>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct PasswordProtectionResponse {
    online_store_password_protection: Option<PasswordProtection>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ApiVersion {
    pub handle: String,
    pub supported: bool,
}

#[derive(Debug)]
pub enum AdminError {
    Abort(String, Option<String>),
    Bug(String),
}

impl std::fmt::Display for AdminError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AdminError::Abort(msg, _) => write!(f, "{msg}"),
            AdminError::Bug(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for AdminError {}

impl From<AdminError> for FatalError {
    fn from(e: AdminError) -> Self {
        match e {
            AdminError::Abort(msg, try_msg) => abort_error(msg, try_msg, vec![]),
            AdminError::Bug(msg) => bug_error(msg, None::<String>),
        }
    }
}

/// Execute an Admin GraphQL operation with the same session-per-call shape as
/// the original TypeScript `adminRequest`.
pub async fn admin_request<T, V>(
    query: &str,
    session: &AdminSession,
    variables: Option<V>,
) -> Result<T, GraphqlRequestError>
where
    T: DeserializeOwned,
    V: Serialize,
{
    AdminClient::new(session.clone())
        .query(query, variables)
        .await
}

/// Execute a generated Admin GraphQL operation.
///
/// In the TypeScript CLI this accepts a `TypedDocumentNode`. The Rust codegen
/// currently emits string constants plus typed variables/response structs, so
/// this delegates to [`admin_request`] while preserving the generated-document
/// call site shape.
pub async fn admin_request_doc<T, V>(
    query: &str,
    session: &AdminSession,
    variables: Option<V>,
) -> Result<T, GraphqlRequestError>
where
    T: DeserializeOwned,
    V: Serialize,
{
    admin_request(query, session, variables).await
}

pub async fn admin_request_doc_with_retry<T, V>(
    query: &str,
    session: &AdminSession,
    variables: Option<V>,
    retry_config: RetryConfig,
) -> Result<T, GraphqlRequestError>
where
    T: DeserializeOwned,
    V: Serialize,
{
    AdminClient::new(session.clone())
        .query_with_retry(query, variables, retry_config)
        .await
}

/// Execute an Admin REST request with the same session-per-call shape as the
/// original TypeScript `restRequest`.
pub async fn rest_request<T: DeserializeOwned>(
    method: reqwest::Method,
    path: &str,
    session: &AdminSession,
    body: Option<serde_json::Value>,
    search_params: Option<HashMap<String, String>>,
    api_version: Option<&str>,
) -> Result<RestResponse<T>, RestError> {
    AdminClient::new(session.clone())
        .rest_request(method, path, body, search_params, api_version)
        .await
}

/// Fetch all Admin API versions using the generated `publicApiVersions`
/// operation, matching the original TypeScript `fetchApiVersions` helper.
pub async fn fetch_api_versions(session: &AdminSession) -> Result<Vec<ApiVersion>, AdminError> {
    AdminClient::new(session.clone()).fetch_api_versions().await
}

pub struct AdminClient {
    session: AdminSession,
    client: reqwest::Client,
    latest_version: Mutex<HashMap<String, String>>,
    graphql_client: Option<GraphqlClient>,
    rest_client: Option<RestClient>,
}

impl AdminClient {
    pub fn new(session: AdminSession) -> Self {
        let client = crate::http::build_client(None).expect("failed to build HTTP client");
        Self {
            session,
            client,
            latest_version: Mutex::new(HashMap::new()),
            graphql_client: None,
            rest_client: None,
        }
    }

    pub fn with_client(session: AdminSession, client: reqwest::Client) -> Self {
        Self {
            session,
            client,
            latest_version: Mutex::new(HashMap::new()),
            graphql_client: None,
            rest_client: None,
        }
    }

    pub fn with_clients(
        session: AdminSession,
        graphql_client: GraphqlClient,
        rest_client: RestClient,
    ) -> Self {
        Self {
            session,
            client: crate::http::build_client(None).expect("failed to build HTTP client"),
            latest_version: Mutex::new(HashMap::new()),
            graphql_client: Some(graphql_client),
            rest_client: Some(rest_client),
        }
    }

    pub fn is_theme_access_session(&self) -> bool {
        self.session.token.starts_with("shptka_")
    }

    pub fn session(&self) -> &AdminSession {
        &self.session
    }

    fn admin_graphql_url(&self, version: &str) -> String {
        if self.is_theme_access_session() {
            format!("https://{THEME_KIT_ACCESS_DOMAIN}/cli/admin/api/{version}/graphql.json")
        } else {
            format!(
                "https://{}/admin/api/{version}/graphql.json",
                self.session.store_fqdn
            )
        }
    }

    fn admin_rest_base_url(&self, version: &str) -> String {
        if self.is_theme_access_session() {
            format!("https://{THEME_KIT_ACCESS_DOMAIN}/cli/admin/api/{version}")
        } else {
            format!("https://{}/admin/api/{version}", self.session.store_fqdn)
        }
    }

    fn graphql_client_for_version(&self, _version: &str) -> GraphqlClient {
        if let Some(ref gql) = self.graphql_client {
            return gql.clone();
        }
        let url = self.admin_graphql_url(_version);
        let mut client =
            GraphqlClient::with_client(url, Some(self.session.token.clone()), self.client.clone());
        if self.is_theme_access_session() {
            let mut extra = HeaderMap::new();
            extra.insert(
                HeaderName::from_static("x-shopify-shop"),
                HeaderValue::from_str(&self.session.store_fqdn).unwrap(),
            );
            extra.insert(
                HeaderName::from_static("x-shopify-access-token"),
                HeaderValue::from_str(&self.session.token).unwrap(),
            );
            client = client.with_extra_headers(extra);
        }
        client
    }

    fn rest_client_for_version(&self, _version: &str) -> RestClient {
        if let Some(ref rest) = self.rest_client {
            return rest.clone();
        }
        let base_url = self.admin_rest_base_url(_version);
        let mut client =
            RestClient::with_client(base_url, self.session.token.clone(), self.client.clone());
        if self.is_theme_access_session() {
            let mut extra = HeaderMap::new();
            extra.insert(
                HeaderName::from_static("x-shopify-shop"),
                HeaderValue::from_str(&self.session.store_fqdn).unwrap(),
            );
            extra.insert(
                HeaderName::from_static("x-shopify-access-token"),
                HeaderValue::from_str(&self.session.token).unwrap(),
            );
            client = client.with_extra_headers(extra);
        }
        client
    }

    pub async fn fetch_latest_api_version(&self) -> Result<String, AdminError> {
        {
            let cache = self.latest_version.lock().unwrap();
            if let Some(version) = cache.get(&self.session.store_fqdn) {
                return Ok(version.clone());
            }
        }

        let versions = self.fetch_api_versions().await?;
        let latest = versions
            .into_iter()
            .filter(|v| v.supported)
            .map(|v| v.handle)
            .next_back()
            .unwrap_or_else(|| "unstable".to_string());

        let mut cache = self.latest_version.lock().unwrap();
        cache.insert(self.session.store_fqdn.clone(), latest.clone());
        Ok(latest)
    }

    pub async fn fetch_api_versions(&self) -> Result<Vec<ApiVersion>, AdminError> {
        let client = self.graphql_client_for_version("unstable");
        let result: Result<PublicApiVersionsResponse, GraphqlRequestError> =
            client.query(PUBLIC_API_VERSIONS_QUERY).await;

        match result {
            Ok(response) => Ok(response
                .public_api_versions
                .into_iter()
                .map(|version| ApiVersion {
                    handle: version.handle,
                    supported: version.supported,
                })
                .collect()),
            Err(err) => match &err {
                GraphqlRequestError::ApiError(_, 403) => {
                    let store_name = self.session.store_fqdn.replace(".myshopify.com", "");
                    Err(AdminError::Abort(
                        format!("Looks like you don't have access to this dev store: {store_name}"),
                        Some("If you're not the owner, create a dev store staff account for yourself".into()),
                    ))
                }
                GraphqlRequestError::ApiError(_, 401 | 404) => Err(AdminError::Abort(
                    format!(
                        "Error connecting to your store {}: {}",
                        self.session.store_fqdn, err
                    ),
                    None,
                )),
                GraphqlRequestError::Network(msg) => Err(AdminError::Abort(
                    format!(
                        "Network error connecting to your store {}: {msg}",
                        self.session.store_fqdn
                    ),
                    Some("Check your internet connection and try again.".into()),
                )),
                _ => Err(AdminError::Bug(format!(
                    "Unknown error connecting to your store {}: {err}",
                    self.session.store_fqdn
                ))),
            },
        }
    }

    pub async fn query<T: DeserializeOwned, V: serde::Serialize>(
        &self,
        query: &str,
        variables: Option<V>,
    ) -> Result<T, GraphqlRequestError> {
        let version = self
            .fetch_latest_api_version()
            .await
            .map_err(|e| GraphqlRequestError::ApiError(e.to_string(), 0))?;
        let client = self.graphql_client_for_version(&version);
        client.query_with_variables(query, variables).await
    }

    pub async fn query_with_retry<T: DeserializeOwned, V: serde::Serialize>(
        &self,
        query: &str,
        variables: Option<V>,
        retry_config: RetryConfig,
    ) -> Result<T, GraphqlRequestError> {
        let version = self
            .fetch_latest_api_version()
            .await
            .map_err(|e| GraphqlRequestError::ApiError(e.to_string(), 0))?;
        let client = self
            .graphql_client_for_version(&version)
            .with_retry(retry_config);
        client.query_with_variables(query, variables).await
    }

    pub async fn rest_request<T: DeserializeOwned>(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<serde_json::Value>,
        search_params: Option<HashMap<String, String>>,
        api_version: Option<&str>,
    ) -> Result<RestResponse<T>, RestError> {
        let version = api_version.unwrap_or("unstable");
        let client = self.rest_client_for_version(version);

        match method {
            reqwest::Method::GET => client.get(path, search_params).await,
            reqwest::Method::POST => {
                client
                    .post(path, body.unwrap_or(serde_json::Value::Null))
                    .await
            }
            reqwest::Method::PUT => {
                client
                    .put(path, body.unwrap_or(serde_json::Value::Null))
                    .await
            }
            reqwest::Method::DELETE => {
                let result: RestResponse<serde_json::Value> = client.delete(path).await?;
                let body: T = serde_json::from_value(result.body)
                    .map_err(|e| RestError::Parse(e.to_string()))?;
                Ok(RestResponse {
                    status: result.status,
                    headers: result.headers,
                    body,
                })
            }
            _ => Err(RestError::Network(format!("Unsupported method: {method}"))),
        }
    }

    pub async fn get<T: DeserializeOwned>(
        &self,
        path: &str,
        query: Option<HashMap<String, String>>,
    ) -> Result<RestResponse<T>, RestError> {
        self.rest_request(reqwest::Method::GET, path, None, query, None)
            .await
    }

    pub async fn post<T: DeserializeOwned>(
        &self,
        path: &str,
        body: serde_json::Value,
    ) -> Result<RestResponse<T>, RestError> {
        self.rest_request(reqwest::Method::POST, path, Some(body), None, None)
            .await
    }

    pub async fn put<T: DeserializeOwned>(
        &self,
        path: &str,
        body: serde_json::Value,
    ) -> Result<RestResponse<T>, RestError> {
        self.rest_request(reqwest::Method::PUT, path, Some(body), None, None)
            .await
    }

    pub async fn delete(&self, path: &str) -> Result<RestResponse<serde_json::Value>, RestError> {
        self.rest_request(reqwest::Method::DELETE, path, None, None, None)
            .await
    }

    // ===== Theme CRUD =====

    pub async fn list_themes(&self) -> Result<Vec<Theme>, AdminError> {
        let resp: RestResponse<ThemesWrapper> = self
            .get("/themes.json", None)
            .await
            .map_err(|e| AdminError::Abort(format!("Failed to list themes: {e}"), None))?;
        Ok(resp.body.themes)
    }

    pub async fn get_theme(&self, id: i64) -> Result<Option<Theme>, AdminError> {
        let resp: RestResponse<ThemeWrapper> = self
            .get(&format!("/themes/{id}.json"), None)
            .await
            .map_err(|e| AdminError::Abort(format!("Failed to get theme: {e}"), None))?;
        Ok(Some(resp.body.theme))
    }

    pub async fn create_theme(
        &self,
        name: &str,
        source: Option<&str>,
        role: Option<&str>,
    ) -> Result<Theme, AdminError> {
        let mut input = serde_json::json!({ "theme": { "name": name } });
        if let Some(s) = source {
            input["theme"]["source"] = serde_json::json!(s);
        }
        if let Some(r) = role {
            input["theme"]["role"] = serde_json::json!(r);
        }
        let resp: RestResponse<ThemeWrapper> = self
            .post("/themes.json", input)
            .await
            .map_err(|e| AdminError::Abort(format!("Failed to create theme: {e}"), None))?;
        Ok(resp.body.theme)
    }

    pub async fn update_theme(
        &self,
        id: i64,
        name: Option<&str>,
        role: Option<&str>,
    ) -> Result<Theme, AdminError> {
        let mut input = serde_json::json!({ "theme": {} });
        if let Some(n) = name {
            input["theme"]["name"] = serde_json::json!(n);
        }
        if let Some(r) = role {
            input["theme"]["role"] = serde_json::json!(r);
        }
        let resp: RestResponse<ThemeWrapper> = self
            .put(&format!("/themes/{id}.json"), input)
            .await
            .map_err(|e| AdminError::Abort(format!("Failed to update theme: {e}"), None))?;
        Ok(resp.body.theme)
    }

    pub async fn delete_theme(&self, id: i64) -> Result<(), AdminError> {
        self.delete(&format!("/themes/{id}.json"))
            .await
            .map_err(|e| AdminError::Abort(format!("Failed to delete theme: {e}"), None))?;
        Ok(())
    }

    pub async fn duplicate_theme(&self, source_id: i64, name: &str) -> Result<Theme, AdminError> {
        let input = serde_json::json!({
            "theme": { "name": name, "source_id": source_id }
        });
        let resp: RestResponse<ThemeWrapper> = self
            .post("/themes.json", input)
            .await
            .map_err(|e| AdminError::Abort(format!("Failed to duplicate theme: {e}"), None))?;
        Ok(resp.body.theme)
    }

    pub async fn publish_theme(&self, id: i64) -> Result<Theme, AdminError> {
        let input = serde_json::json!({ "theme": { "role": "main" } });
        let resp: RestResponse<ThemeWrapper> = self
            .put(&format!("/themes/{id}.json"), input)
            .await
            .map_err(|e| AdminError::Abort(format!("Failed to publish theme: {e}"), None))?;
        Ok(resp.body.theme)
    }

    // ===== Theme File Operations (Assets) =====

    pub async fn get_theme_file_bodies(
        &self,
        theme_id: i64,
        keys: Vec<String>,
    ) -> Result<HashMap<String, String>, AdminError> {
        let mut files = HashMap::new();
        for key in keys {
            let params = vec![("asset[key]", key.as_str())]
                .into_iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect::<HashMap<_, _>>();
            let resp: RestResponse<AssetsWrapper> = self
                .get(&format!("/themes/{theme_id}/assets.json"), Some(params))
                .await
                .map_err(|e| AdminError::Abort(format!("Failed to get asset: {e}"), None))?;
            if let Some(asset) = resp.body.asset {
                if let Some(value) = asset.value.or(asset.attachment) {
                    files.insert(asset.key, value);
                }
            }
        }
        Ok(files)
    }

    pub async fn get_theme_file_checksums(
        &self,
        theme_id: i64,
        keys: Vec<String>,
    ) -> Result<HashMap<String, String>, AdminError> {
        let mut checksums = HashMap::new();
        for key in keys {
            let params = vec![("asset[key]", key.as_str())]
                .into_iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect::<HashMap<_, _>>();
            let resp: RestResponse<AssetsWrapper> = self
                .get(&format!("/themes/{theme_id}/assets.json"), Some(params))
                .await
                .map_err(|e| {
                    AdminError::Abort(format!("Failed to get asset checksum: {e}"), None)
                })?;
            if let Some(asset) = resp.body.asset {
                if let Some(cs) = asset.checksum {
                    checksums.insert(asset.key, cs);
                }
            }
        }
        Ok(checksums)
    }

    pub async fn upsert_theme_files(
        &self,
        theme_id: i64,
        files: HashMap<String, String>,
    ) -> Result<(), AdminError> {
        for (key, value) in files {
            let input = serde_json::json!({
                "asset": { "key": key, "value": value }
            });
            self.put::<AssetsWrapper>(&format!("/themes/{theme_id}/assets.json"), input)
                .await
                .map_err(|e| {
                    AdminError::Abort(format!("Failed to upsert asset {key}: {e}"), None)
                })?;
        }
        Ok(())
    }

    pub async fn delete_theme_files(
        &self,
        theme_id: i64,
        keys: Vec<String>,
    ) -> Result<(), AdminError> {
        for key in keys {
            let params = vec![("asset[key]", key.as_str())]
                .into_iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect::<HashMap<_, _>>();
            self.delete(&format!(
                "/themes/{theme_id}/assets.json?{}",
                params
                    .iter()
                    .map(|(k, v)| format!("{k}={v}"))
                    .collect::<Vec<_>>()
                    .join("&")
            ))
            .await
            .map_err(|e| AdminError::Abort(format!("Failed to delete asset {key}: {e}"), None))?;
        }
        Ok(())
    }

    // ===== GraphQL-based Methods =====

    pub async fn public_api_versions(&self) -> Result<Vec<ApiVersion>, AdminError> {
        self.fetch_api_versions().await
    }

    pub async fn metafield_definitions_by_owner_type(
        &self,
        owner_type: &str,
    ) -> Result<Vec<MetafieldDefinition>, GraphqlRequestError> {
        let vars = serde_json::json!({ "ownerType": owner_type });
        let resp: MetafieldDefinitionsResponse =
            self.query(METAFIELD_DEFINITIONS_QUERY, Some(vars)).await?;
        Ok(resp.metafield_definitions)
    }

    pub async fn online_store_password_protection(
        &self,
    ) -> Result<Option<PasswordProtection>, GraphqlRequestError> {
        let resp: PasswordProtectionResponse = self
            .query(
                ONLINE_STORE_PASSWORD_PROTECTION_QUERY,
                None::<serde_json::Value>,
            )
            .await?;
        Ok(resp.online_store_password_protection)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admin_graphql_url_built_correctly() {
        let session = AdminSession {
            store_fqdn: "test-store.myshopify.com".into(),
            token: "shpat_test".into(),
        };
        let client = AdminClient::new(session);
        assert_eq!(
            client.admin_graphql_url("2024-01"),
            "https://test-store.myshopify.com/admin/api/2024-01/graphql.json"
        );
    }

    #[test]
    fn theme_access_url_uses_theme_kit_domain() {
        let session = AdminSession {
            store_fqdn: "test-store.myshopify.com".into(),
            token: "shptka_test".into(),
        };
        let client = AdminClient::new(session);
        assert_eq!(
            client.admin_graphql_url("unstable"),
            "https://theme-kit-access.shopifyapps.com/cli/admin/api/unstable/graphql.json"
        );
    }

    #[test]
    fn theme_access_rest_url_uses_theme_kit_domain() {
        let session = AdminSession {
            store_fqdn: "test-store.myshopify.com".into(),
            token: "shptka_test".into(),
        };
        let client = AdminClient::new(session);
        assert_eq!(
            client.admin_rest_base_url("2024-01"),
            "https://theme-kit-access.shopifyapps.com/cli/admin/api/2024-01"
        );
    }

    #[test]
    fn is_theme_access_session_detects_shptka() {
        let session = AdminSession {
            store_fqdn: "store.myshopify.com".into(),
            token: "shptka_abc".into(),
        };
        let client = AdminClient::new(session);
        assert!(client.is_theme_access_session());
    }

    #[test]
    fn is_theme_access_session_false_for_shpat() {
        let session = AdminSession {
            store_fqdn: "store.myshopify.com".into(),
            token: "shpat_abc".into(),
        };
        let client = AdminClient::new(session);
        assert!(!client.is_theme_access_session());
    }

    #[test]
    fn is_theme_access_session_false_for_bearer() {
        let session = AdminSession {
            store_fqdn: "store.myshopify.com".into(),
            token: "abc123".into(),
        };
        let client = AdminClient::new(session);
        assert!(!client.is_theme_access_session());
    }

    #[test]
    fn version_cache_initially_empty() {
        let session = AdminSession {
            store_fqdn: "store.myshopify.com".into(),
            token: "shpat_test".into(),
        };
        let client = AdminClient::new(session);
        let cache = client.latest_version.lock().unwrap();
        assert!(cache.is_empty());
    }

    #[test]
    fn admin_error_to_fatal_abort() {
        let err = AdminError::Abort("test error".into(), Some("try this".into()));
        let fatal: FatalError = err.into();
        assert_eq!(fatal.message, "test error");
        assert_eq!(fatal.r#type, crate::error::FatalErrorType::Abort);
    }

    #[test]
    fn admin_error_to_fatal_bug() {
        let err = AdminError::Bug("bug".into());
        let fatal: FatalError = err.into();
        assert_eq!(fatal.r#type, crate::error::FatalErrorType::Bug);
    }

    #[test]
    fn theme_deserialize() {
        let json = serde_json::json!({
            "id": 123,
            "name": "Default",
            "role": "main",
            "previewable": true,
            "processing": false,
        });
        let t: Theme = serde_json::from_value(json).unwrap();
        assert_eq!(t.name, "Default");
        assert_eq!(t.role, Some("main".into()));
    }

    #[test]
    fn theme_asset_deserialize() {
        let json = serde_json::json!({
            "key": "assets/theme.css",
            "value": "body { color: red }",
            "checksum": "abc123",
            "contentType": "text/css",
            "size": 128,
        });
        let a: ThemeAsset = serde_json::from_value(json).unwrap();
        assert_eq!(a.key, "assets/theme.css");
        assert_eq!(a.checksum, Some("abc123".into()));
    }

    #[test]
    fn metafield_definition_deserialize() {
        let json = serde_json::json!({
            "id": "gid://shopify/MetafieldDefinition/1",
            "name": "Brand Color",
            "namespace": "custom",
            "key": "brand_color",
            "type": { "name": "single_line_text_field" },
            "description": "Primary brand color",
            "pinnedPosition": 1,
        });
        let m: MetafieldDefinition = serde_json::from_value(json).unwrap();
        assert_eq!(m.name, "Brand Color");
        assert_eq!(m.r#type.name, "single_line_text_field");
    }

    #[test]
    fn password_protection_deserialize() {
        let json = serde_json::json!({ "enabled": true, "password": "secret123" });
        let p: PasswordProtection = serde_json::from_value(json).unwrap();
        assert!(p.enabled);
        assert_eq!(p.password, Some("secret123".into()));
    }

    #[test]
    fn themes_wrapper_deserialize() {
        let json = serde_json::json!({
            "themes": [
                { "id": 1, "name": "Default" },
                { "id": 2, "name": "Custom" },
            ]
        });
        let w: ThemesWrapper = serde_json::from_value(json).unwrap();
        assert_eq!(w.themes.len(), 2);
    }

    #[test]
    fn theme_wrapper_deserialize() {
        let json = serde_json::json!({ "theme": { "id": 1, "name": "Default" } });
        let w: ThemeWrapper = serde_json::from_value(json).unwrap();
        assert_eq!(w.theme.id, 1);
    }

    #[test]
    fn api_version_deserialize() {
        let json = serde_json::json!({ "handle": "2024-07", "supported": true });
        let v: ApiVersion = serde_json::from_value(json).unwrap();
        assert_eq!(v.handle, "2024-07");
        assert!(v.supported);
    }

    #[test]
    fn metafield_definitions_has_query() {
        assert!(METAFIELD_DEFINITIONS_QUERY.contains("metafieldDefinitions"));
    }

    #[test]
    fn password_protection_has_query() {
        assert!(ONLINE_STORE_PASSWORD_PROTECTION_QUERY.contains("onlineStorePasswordProtection"));
    }

    // ===== Wiremock Tests =====

    fn mock_admin_client(server: &wiremock::MockServer) -> AdminClient {
        let session = AdminSession {
            store_fqdn: "test-store.myshopify.com".into(),
            token: "shpat_test".into(),
        };
        let gql = GraphqlClient::new(server.uri(), Some("shpat_test".into()));
        let rest = RestClient::new(server.uri(), "shpat_test");
        AdminClient::with_clients(session, gql, rest)
    }

    #[tokio::test]
    async fn list_themes_returns_list() {
        let mock_server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/themes.json"))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "themes": [
                        { "id": 1, "name": "Default", "role": "main" },
                        { "id": 2, "name": "Custom", "role": null },
                    ]
                })),
            )
            .mount(&mock_server)
            .await;

        let client = mock_admin_client(&mock_server);
        let themes = client.list_themes().await.unwrap();
        assert_eq!(themes.len(), 2);
        assert_eq!(themes[0].name, "Default");
    }

    #[tokio::test]
    async fn list_themes_returns_empty() {
        let mock_server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/themes.json"))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "themes": []
                })),
            )
            .mount(&mock_server)
            .await;

        let client = mock_admin_client(&mock_server);
        let themes = client.list_themes().await.unwrap();
        assert!(themes.is_empty());
    }

    #[tokio::test]
    async fn get_theme_returns_theme() {
        let mock_server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/themes/1.json"))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "theme": { "id": 1, "name": "Default", "role": "main" }
                })),
            )
            .mount(&mock_server)
            .await;

        let client = mock_admin_client(&mock_server);
        let theme = client.get_theme(1).await.unwrap();
        assert!(theme.is_some());
        assert_eq!(theme.unwrap().name, "Default");
    }

    #[tokio::test]
    async fn get_theme_returns_none_when_not_found() {
        let mock_server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/themes/999.json"))
            .respond_with(wiremock::ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        let client = mock_admin_client(&mock_server);
        let result = client.get_theme(999).await;
        assert!(result.is_err());
    }

    fn mock_version_response() -> wiremock::MockBuilder {
        wiremock::Mock::given(wiremock::matchers::method("POST")).and(
            wiremock::matchers::body_string_contains("publicApiVersions"),
        )
    }

    fn mock_version_template() -> wiremock::ResponseTemplate {
        wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": {
                "publicApiVersions": [
                    { "handle": "2024-07", "supported": true },
                    { "handle": "unstable", "supported": true },
                ]
            },
            "extensions": { "cost": { "actualQueryCost": null, "throttleStatus": null } }
        }))
    }

    #[tokio::test]
    async fn fetch_api_versions_returns_versions() {
        let mock_server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .and(wiremock::matchers::body_string_contains(
                "publicApiVersions",
            ))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "data": {
                        "publicApiVersions": [
                            { "handle": "2024-07", "supported": true },
                            { "handle": "2024-10", "supported": true },
                        ]
                    },
                    "extensions": { "cost": { "actualQueryCost": null, "throttleStatus": null } }
                })),
            )
            .mount(&mock_server)
            .await;

        let client = mock_admin_client(&mock_server);
        let versions = client.fetch_api_versions().await.unwrap();
        assert_eq!(versions.len(), 2);
        assert_eq!(versions[0].handle, "2024-07");
    }

    #[tokio::test]
    async fn metafield_definitions_returns_list() {
        let mock_server = wiremock::MockServer::start().await;
        mock_version_response()
            .respond_with(mock_version_template())
            .mount(&mock_server)
            .await;

        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "data": {
                        "metafieldDefinitions": [
                            {
                                "id": "gid://shopify/MetafieldDefinition/1",
                                "name": "Brand Color",
                                "namespace": "custom",
                                "key": "brand_color",
                                "type": { "name": "single_line_text_field" },
                                "description": "Primary brand color",
                                "pinnedPosition": 1,
                            }
                        ]
                    },
                    "extensions": { "cost": { "actualQueryCost": null, "throttleStatus": null } }
                })),
            )
            .mount(&mock_server)
            .await;

        let client = mock_admin_client(&mock_server);
        let defs = client
            .metafield_definitions_by_owner_type("PRODUCT")
            .await
            .unwrap();
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].name, "Brand Color");
    }

    #[tokio::test]
    async fn online_store_password_protection_returns_status() {
        let mock_server = wiremock::MockServer::start().await;
        mock_version_response()
            .respond_with(mock_version_template())
            .mount(&mock_server)
            .await;

        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "data": {
                        "onlineStorePasswordProtection": {
                            "enabled": true,
                            "password": "secret123"
                        }
                    },
                    "extensions": { "cost": { "actualQueryCost": null, "throttleStatus": null } }
                })),
            )
            .mount(&mock_server)
            .await;

        let client = mock_admin_client(&mock_server);
        let protection = client.online_store_password_protection().await.unwrap();
        assert!(protection.is_some());
        assert!(protection.unwrap().enabled);
    }
}
