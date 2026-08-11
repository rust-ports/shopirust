use crate::api::graphql::{CacheOptions, GraphqlClient, GraphqlRequestError, UnauthorizedHandler};
use crate::api::rate_limiter::ApiRateLimiter;
use crate::api::utilities::add_cursor_and_filters_to_app_logs_url;
use crate::constants::app_management_fqdn;
use crate::http::build_headers;
use reqwest::header::HeaderMap;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

fn app_management_rate_limiter() -> ApiRateLimiter {
    static LIMITER: OnceLock<ApiRateLimiter> = OnceLock::new();
    LIMITER.get_or_init(ApiRateLimiter::shopify_default).clone()
}

pub fn app_management_headers(token: &str) -> HeaderMap {
    build_headers(Some(token))
}

pub fn app_management_app_logs_url(
    organization_id: &str,
    cursor: Option<&str>,
    filters: Option<HashMap<String, String>>,
) -> String {
    let base = format!(
        "https://{}/app_management/unstable/organizations/{organization_id}/app_logs/poll",
        app_management_fqdn(None),
    );
    add_cursor_and_filters_to_app_logs_url(&base, cursor, filters)
}

/// GET the app-logs poll endpoint and parse the JSON body.
pub async fn fetch_app_logs_http(
    url: &str,
    jwt_token: &str,
) -> Result<cli_api::AppLogsFetchResult, String> {
    let client = crate::http::build_client(None).map_err(|e| e.to_string())?;
    let headers = app_management_headers(jwt_token);
    let response = client
        .get(url)
        .headers(headers)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = response.status().as_u16();
    let body: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;

    let errors = body
        .get("errors")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|e| e.as_str().map(str::to_string))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    if status != 200 {
        let errors = if errors.is_empty() {
            vec![format!("Request failed with status {status}")]
        } else {
            errors
        };
        return Ok(cli_api::AppLogsFetchResult {
            status,
            app_logs: vec![],
            cursor: None,
            errors,
        });
    }

    let app_logs = body
        .get("app_logs")
        .cloned()
        .map(|v| serde_json::from_value(v).unwrap_or_default())
        .unwrap_or_default();
    let cursor = body
        .get("cursor")
        .and_then(|v| v.as_str())
        .map(str::to_string);

    Ok(cli_api::AppLogsFetchResult {
        status,
        app_logs,
        cursor,
        errors,
    })
}

// ===== GraphQL Query/Mutation Constants =====

const ORGANIZATIONS_QUERY: &str = r#"
query ListOrganizations {
  organizations(first: 200) {
    nodes {
      id
      name
    }
  }
}
"#;

const ORG_FROM_ID_QUERY: &str = r#"
query FindOrganization($id: ID!) {
  organizations(first: 1, id: $id) {
    nodes {
      id
      name
      apps(first: 25) {
        pageInfo { hasNextPage }
        nodes {
          id
          title
          key
        }
      }
    }
  }
}
"#;

const CREATE_APP_MUTATION: &str = r#"
mutation CreateApp($initialVersion: AppVersionInput!, $organizationId: ID!) {
  appCreate(initialVersion: $initialVersion, organizationId: $organizationId) {
    app {
      id
      key
      activeRoot {
        clientCredentials {
          secrets { key }
        }
      }
    }
    userErrors {
      category
      message
      on
    }
  }
}
"#;

const UPDATE_URLS_MUTATION: &str = r#"
mutation AppUpdate($apiKey: String!, $applicationUrl: Url!, $redirectUrlWhitelist: [Url]!) {
  appUpdate(input: {apiKey: $apiKey, applicationUrl: $applicationUrl, redirectUrlWhitelist: $redirectUrlWhitelist}) {
    userErrors {
      message
      field
    }
  }
}
"#;

const APP_FROM_KEY_QUERY: &str = r#"
query ActiveAppReleaseFromApiKey($apiKey: String!) {
  app: appByKey(key: $apiKey) {
    id
    key
    organizationId
    activeRoot {
      clientCredentials { secrets { key } }
      grantedShopifyApprovalScopes
    }
    activeRelease {
      id
      version {
        name
        appModules {
          uuid
          userIdentifier
          handle
          config
          target
          specification {
            identifier
            externalIdentifier
            name
            experience
            managementExperience
          }
        }
      }
    }
  }
}
"#;

const APP_FROM_ID_BASIC_QUERY: &str = r#"
query AppInstallCount($appId: ID!) {
  app(id: $appId) {
    id
    key
    installCount
  }
}
"#;

const APPS_LIST_QUERY: &str = r#"
query listApps($query: String) {
  appsConnection(query: $query, first: 50) {
    edges {
      node {
        id
        key
        activeRelease {
          id
          version { name }
        }
      }
    }
    pageInfo { hasNextPage }
  }
}
"#;

const SPECIFICATIONS_QUERY: &str = r#"
query fetchSpecifications($organizationId: ID!) {
  specifications(organizationId: $organizationId) {
    name
    identifier
    externalIdentifier
    experience
    features
    uidStrategy {
      __typename
      appModuleLimit
      isClientProvided
    }
    validationSchema {
      jsonSchema
    }
  }
}
"#;

const TEMPLATE_SPECIFICATIONS_QUERY: &str = r#"
query RemoteTemplateSpecifications {
  templateSpecifications {
    identifier
    name
    defaultName
    group
    sortPriority
    supportLinks
    types {
      url
      type
      extensionPoints
      supportedFlavors {
        name
        value
        path
      }
    }
  }
}
"#;

const CREATE_APP_VERSION_MUTATION: &str = r#"
mutation CreateAppVersion($appId: ID!, $version: AppVersionInput!, $metadata: VersionMetadataInput) {
  appVersionCreate(appId: $appId, version: $version, metadata: $metadata) {
    version {
      id
      appModules {
        uuid
        userIdentifier
        handle
        config
        target
        specification {
          identifier
          externalIdentifier
          name
          experience
          managementExperience
        }
      }
      metadata {
        versionTag
        message
      }
    }
    userErrors {
      field
      message
      category
      code
      on
    }
  }
}
"#;

const RELEASE_VERSION_MUTATION: &str = r#"
mutation ReleaseVersion($appId: ID!, $versionId: ID!) {
  appReleaseCreate(appId: $appId, versionId: $versionId) {
    release {
      version {
        id
        metadata {
          message
          versionTag
        }
      }
    }
    userErrors {
      field
      message
      category
      code
      on
    }
  }
}
"#;

const CREATE_ASSET_URL_MUTATION: &str = r#"
mutation CreateAssetURL($sourceExtension: SourceExtension!, $organizationId: ID!) {
  appRequestSourceUploadUrl(sourceExtension: $sourceExtension, organizationId: $organizationId) {
    sourceUploadUrl
    userErrors {
      field
      message
    }
  }
}
"#;

const ACTIVE_RELEASE_QUERY: &str = r#"
query activeAppRelease($appId: ID!) {
  app(id: $appId) {
    id
    key
    organizationId
    activeRoot {
      clientCredentials { secrets { key } }
      grantedShopifyApprovalScopes
    }
    activeRelease {
      id
      version {
        name
        appModules {
          uuid
          userIdentifier
          handle
          config
          target
          specification {
            identifier
            externalIdentifier
            name
            experience
            managementExperience
          }
        }
      }
    }
  }
}
"#;

const APP_VERSIONS_QUERY: &str = r#"
query AppVersions($appId: ID!) {
  app(id: $appId) {
    id
    activeRelease {
      id
      version { id }
    }
    versions(first: 20) {
      edges {
        node {
          id
          createdAt
          createdBy
          metadata {
            message
            versionTag
          }
        }
      }
    }
    versionsCount
  }
}
"#;

const APP_VERSION_BY_ID_QUERY: &str = r#"
query AppVersionById($versionId: ID!) {
  version(id: $versionId) {
    id
    metadata {
      message
      versionTag
    }
    appModules {
      uuid
      userIdentifier
      handle
      config
      target
      specification {
        identifier
        externalIdentifier
        name
        experience
        managementExperience
      }
    }
  }
}
"#;

const APP_VERSIONS_DIFF_QUERY: &str = r#"
query AppVersionsDiff($apiKey: String!, $versionId: ID!) {
  appByKey(key: $apiKey) {
    versionsDiff(versionId: $versionId) {
      added {
        uuid
        registrationTitle
        specification {
          identifier
          experience
        }
      }
      updated {
        uuid
        registrationTitle
        specification {
          identifier
          experience
        }
      }
      removed {
        uuid
        registrationTitle
        specification {
          identifier
          experience
        }
      }
    }
  }
}
"#;

// ===== Public Response Types =====

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Organization {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MinimalAppInfo {
    pub id: String,
    pub title: String,
    pub key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OrgDetail {
    pub id: String,
    pub name: String,
    pub apps: Vec<MinimalAppInfo>,
    pub apps_page_info: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OrganizationApp {
    pub id: String,
    pub key: String,
    pub organization_id: Option<String>,
    pub active_root: Option<ActiveRoot>,
    pub active_release: Option<ActiveRelease>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ActiveRoot {
    pub client_credentials: Option<ClientCredentials>,
    pub granted_shopify_approval_scopes: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ClientCredentials {
    pub secrets: Vec<ApiSecretKey>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ApiSecretKey {
    pub key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ActiveRelease {
    pub id: String,
    pub version: Option<ReleaseVersion>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseVersion {
    pub id: Option<String>,
    pub name: Option<String>,
    pub metadata: Option<VersionMetadata>,
    pub app_modules: Option<Vec<AppModule>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VersionMetadata {
    pub message: Option<String>,
    pub version_tag: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppModule {
    pub uuid: String,
    pub user_identifier: Option<String>,
    pub handle: Option<String>,
    pub config: Option<String>,
    pub target: Option<String>,
    pub specification: Option<ModuleSpecification>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModuleSpecification {
    pub identifier: String,
    pub external_identifier: Option<String>,
    pub name: Option<String>,
    pub experience: Option<String>,
    pub management_experience: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionRegistration {
    pub id: String,
    pub uuid: String,
    pub title: String,
    #[serde(rename = "type")]
    pub type_name: String,
    pub draft_version: Option<ExtensionVersion>,
    pub active_version: Option<ExtensionVersion>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionVersion {
    pub config: String,
    pub context: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Specification {
    pub name: String,
    pub identifier: String,
    pub external_identifier: Option<String>,
    pub experience: Option<String>,
    pub features: Option<Vec<String>>,
    pub uid_strategy: Option<UidStrategy>,
    pub validation_schema: Option<ValidationSchema>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UidStrategy {
    #[serde(rename = "__typename")]
    pub type_name: String,
    pub app_module_limit: Option<i64>,
    pub is_client_provided: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ValidationSchema {
    pub json_schema: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TemplateSpecification {
    pub identifier: String,
    pub name: String,
    pub default_name: Option<String>,
    pub group: Option<String>,
    pub sort_priority: Option<i64>,
    pub support_links: Option<Vec<String>>,
    pub types: Option<Vec<TemplateType>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TemplateType {
    pub url: Option<String>,
    pub r#type: Option<String>,
    pub extension_points: Option<Vec<String>>,
    pub supported_flavors: Option<Vec<SupportedFlavor>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SupportedFlavor {
    pub name: String,
    pub value: String,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DeployResult {
    pub version: Option<ReleaseVersion>,
    pub user_errors: Vec<DeployUserError>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseResult {
    pub release: Option<ReleaseData>,
    pub user_errors: Vec<DeployUserError>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseData {
    pub version: Option<ReleaseVersion>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DeployUserError {
    pub field: Option<Vec<String>>,
    pub message: Option<String>,
    pub category: Option<String>,
    pub code: Option<String>,
    pub on: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CreateAppData {
    pub app: Option<CreateAppInfo>,
    pub user_errors: Vec<CreateAppUserError>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CreateAppInfo {
    pub id: String,
    pub key: String,
    pub active_root: Option<CreateAppActiveRoot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CreateAppActiveRoot {
    pub client_credentials: Option<ClientCredentials>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CreateAppUserError {
    pub category: Option<String>,
    pub message: Option<String>,
    pub on: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UserError {
    pub field: Option<Vec<String>>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SignedUploadUrl {
    pub source_upload_url: Option<String>,
    pub user_errors: Vec<UserError>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BasicAppInfo {
    pub id: String,
    pub key: Option<String>,
    pub install_count: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppVersionNode {
    pub id: String,
    pub created_at: Option<String>,
    pub created_by: Option<String>,
    pub metadata: Option<VersionMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppVersionConnection {
    pub edges: Vec<AppVersionEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppVersionEdge {
    pub node: AppVersionNode,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppVersionList {
    pub id: String,
    pub active_release: Option<ActiveRelease>,
    pub versions: Option<AppVersionConnection>,
    pub versions_count: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VersionDetail {
    pub id: String,
    pub metadata: Option<VersionMetadata>,
    pub app_modules: Option<Vec<AppModule>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VersionDiffResult {
    pub added: Vec<DiffModule>,
    pub updated: Vec<DiffModule>,
    pub removed: Vec<DiffModule>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DiffModule {
    pub uuid: String,
    pub registration_title: Option<String>,
    pub specification: Option<DiffSpecification>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DiffSpecification {
    pub identifier: String,
    pub experience: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppEdge {
    pub node: AppNode,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppNode {
    pub id: String,
    pub key: Option<String>,
    pub active_release: Option<ActiveRelease>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PageInfo {
    pub has_next_page: bool,
}

// ===== Internal Response Wrappers =====

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct OrganizationsResponse {
    organizations: OrgConnection,
}

#[derive(Deserialize, Serialize)]
struct OrgConnection {
    nodes: Vec<Organization>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct OrgDetailResponse {
    organizations: OrgDetailConnection,
}

#[derive(Deserialize, Serialize)]
struct OrgDetailConnection {
    nodes: Vec<OrgWithApps>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct OrgWithApps {
    id: String,
    name: String,
    apps: OrgAppConnection,
}

#[derive(Deserialize, Serialize)]
struct OrgAppConnection {
    page_info: PageInfo,
    nodes: Vec<MinimalAppInfo>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct CreateAppResponse {
    app_create: CreateAppData,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct AppUpdateResponse {
    app_update: UpdateUrlsResult,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateUrlsResult {
    user_errors: Vec<UserError>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct AppByKeyResponse {
    app: Option<OrganizationApp>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct BasicAppResponse {
    app: Option<BasicAppInfo>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct AppListResponse {
    apps_connection: AppListConnection,
}

#[derive(Deserialize, Serialize)]
struct AppListConnection {
    edges: Vec<AppEdge>,
    page_info: PageInfo,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct SpecificationsResponse {
    specifications: Vec<Specification>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct TemplateSpecificationsResponse {
    template_specifications: Vec<TemplateSpecification>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct CreateAppVersionResponse {
    app_version_create: DeployResult,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReleaseVersionResponse {
    app_release_create: ReleaseResult,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct CreateAssetUrlResponse {
    app_request_source_upload_url: SignedUploadUrl,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ActiveReleaseResponse {
    app: Option<OrganizationApp>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct AppVersionsResponse {
    app: AppVersionList,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct VersionByIdResponse {
    version: Option<VersionDetail>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct AppVersionsDiffResponse {
    app_by_key: AppWithDiff,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct AppWithDiff {
    versions_diff: Option<VersionDiffResult>,
}

/// Client for the Shopify App Management GraphQL API.
///
/// Wraps [`GraphqlClient`] with App Management–specific rate limiting, URL
/// resolution, and deprecation tracking. Every request goes through the
/// shared [`app_management_rate_limiter`] (150 ms minimum interval).
///
/// To test against a mock server, use [`with_graphql`](Self::with_graphql)
/// to inject a pre-configured client.
pub struct AppManagementClient {
    /// The authentication token used for every request.
    pub token: String,
    /// Optional environment overrides (used for FQDN resolution).
    pub env: Option<HashMap<String, String>>,
    /// Optional pre-configured GraphQL client (used for testing).
    graphql: Option<GraphqlClient>,
}

impl AppManagementClient {
    /// Create a new client with the given auth token and optional env map.
    pub fn new(token: String, env: Option<HashMap<String, String>>) -> Self {
        Self {
            token,
            env,
            graphql: None,
        }
    }

    /// Create a client backed by a pre-configured [`GraphqlClient`] (useful for
    /// testing against a mock server). The token and env from the injected
    /// client are used directly — `token` and `env` are set to empty defaults.
    pub fn with_graphql(graphql: GraphqlClient) -> Self {
        Self {
            token: String::new(),
            env: None,
            graphql: Some(graphql),
        }
    }

    /// Execute a GraphQL query against the App Management API.
    ///
    /// The request is rate-limited, automatically retried on transient errors,
    /// cached if `cache_options` is provided, and transparently re-authenticated
    /// if `unauthorized_handler` is supplied and a 401 response is received.
    ///
    /// When the client was created with [`with_graphql`](Self::with_graphql),
    /// the injected client is used directly (bypassing rate limiter, cache, and
    /// auth handler configuration from this method).
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
        // When a pre-configured GraphQL client is injected (test mode), use
        // it directly — the caller is responsible for rate limiting, headers,
        // etc.
        if let Some(ref gql) = self.graphql {
            return gql.query_with_variables(query, variables).await;
        }

        let url = format!(
            "https://{}/app_management/unstable/graphql.json",
            app_management_fqdn(self.env.as_ref()),
        );
        let mut client = GraphqlClient::new(url, Some(self.token.clone()))
            .with_rate_limiter(app_management_rate_limiter());

        if let Some(opts) = cache_options {
            client = client.with_cache_options(opts);
        }
        if let Some(handler) = unauthorized_handler {
            client = client.with_token_refresh_handler(handler);
        }

        client.query_with_variables(query, variables).await
    }

    // ===== Method Implementations =====

    /// Fetch all organizations accessible with the current token.
    pub async fn organizations(&self) -> Result<Vec<Organization>, GraphqlRequestError> {
        let resp: OrganizationsResponse = self
            .request(ORGANIZATIONS_QUERY, None::<()>, None, None)
            .await?;
        Ok(resp.organizations.nodes)
    }

    /// Fetch an organization by ID, including its apps.
    pub async fn org_from_id(&self, id: &str) -> Result<Option<OrgDetail>, GraphqlRequestError> {
        let vars = serde_json::json!({ "id": id });
        let resp: OrgDetailResponse = self
            .request(ORG_FROM_ID_QUERY, Some(vars), None, None)
            .await?;
        Ok(resp
            .organizations
            .nodes
            .into_iter()
            .next()
            .map(|o| OrgDetail {
                id: o.id,
                name: o.name,
                apps: o.apps.nodes,
                apps_page_info: o.apps.page_info.has_next_page,
            }))
    }

    /// Create a new app in the given organization.
    pub async fn create_app(
        &self,
        organization_id: &str,
        initial_version: serde_json::Value,
    ) -> Result<CreateAppData, GraphqlRequestError> {
        let vars = serde_json::json!({
            "organizationId": organization_id,
            "initialVersion": initial_version,
        });
        let resp: CreateAppResponse = self
            .request(CREATE_APP_MUTATION, Some(vars), None, None)
            .await?;
        Ok(resp.app_create)
    }

    /// Update application URLs for an app by API key.
    pub async fn update_urls(
        &self,
        api_key: &str,
        application_url: &str,
        redirect_url_whitelist: Vec<&str>,
    ) -> Result<Vec<UserError>, GraphqlRequestError> {
        let vars = serde_json::json!({
            "apiKey": api_key,
            "applicationUrl": application_url,
            "redirectUrlWhitelist": redirect_url_whitelist,
        });
        let resp: AppUpdateResponse = self
            .request(UPDATE_URLS_MUTATION, Some(vars), None, None)
            .await?;
        Ok(resp.app_update.user_errors)
    }

    /// Fetch an app by API key, including active release and credentials.
    pub async fn app_from_id(
        &self,
        api_key: &str,
    ) -> Result<Option<OrganizationApp>, GraphqlRequestError> {
        let vars = serde_json::json!({ "apiKey": api_key });
        let resp: AppByKeyResponse = self
            .request(APP_FROM_KEY_QUERY, Some(vars), None, None)
            .await?;
        Ok(resp.app)
    }

    /// Fetch basic app info (id, key, install count).
    pub async fn app_from_id_basic(
        &self,
        app_id: &str,
    ) -> Result<Option<BasicAppInfo>, GraphqlRequestError> {
        let vars = serde_json::json!({ "appId": app_id });
        let resp: BasicAppResponse = self
            .request(APP_FROM_ID_BASIC_QUERY, Some(vars), None, None)
            .await?;
        Ok(resp.app)
    }

    /// Find apps by name/keyword search.
    pub async fn app_from_name(&self, query: &str) -> Result<Vec<AppNode>, GraphqlRequestError> {
        let vars = serde_json::json!({ "query": query });
        let resp: AppListResponse = self
            .request(APPS_LIST_QUERY, Some(vars), None, None)
            .await?;
        Ok(resp
            .apps_connection
            .edges
            .into_iter()
            .map(|e| e.node)
            .collect())
    }

    /// Fetch extension registrations by extracting app modules from the active release.
    pub async fn app_extension_registrations(
        &self,
        api_key: &str,
    ) -> Result<Vec<AppModule>, GraphqlRequestError> {
        let app = self.app_from_id(api_key).await?;
        Ok(app
            .and_then(|a| a.active_release)
            .and_then(|r| r.version)
            .and_then(|v| v.app_modules)
            .unwrap_or_default())
    }

    /// Fetch available extension specification types.
    pub async fn specifications(
        &self,
        organization_id: &str,
    ) -> Result<Vec<Specification>, GraphqlRequestError> {
        let vars = serde_json::json!({ "organizationId": organization_id });
        let resp: SpecificationsResponse = self
            .request(SPECIFICATIONS_QUERY, Some(vars), None, None)
            .await?;
        Ok(resp.specifications)
    }

    /// Fetch template specifications for scaffolding new extensions.
    pub async fn template_specifications(
        &self,
    ) -> Result<Vec<TemplateSpecification>, GraphqlRequestError> {
        let resp: TemplateSpecificationsResponse = self
            .request(TEMPLATE_SPECIFICATIONS_QUERY, None::<()>, None, None)
            .await?;
        Ok(resp.template_specifications)
    }

    /// Deploy (create) a new app version.
    pub async fn deploy(
        &self,
        app_id: &str,
        version: serde_json::Value,
        metadata: Option<serde_json::Value>,
    ) -> Result<DeployResult, GraphqlRequestError> {
        let mut vars = serde_json::json!({
            "appId": app_id,
            "version": version,
        });
        if let Some(m) = metadata {
            vars["metadata"] = m;
        }
        let resp: CreateAppVersionResponse = self
            .request(CREATE_APP_VERSION_MUTATION, Some(vars), None, None)
            .await?;
        Ok(resp.app_version_create)
    }

    /// Release a specific app version.
    pub async fn release(
        &self,
        app_id: &str,
        version_id: &str,
    ) -> Result<ReleaseResult, GraphqlRequestError> {
        let vars = serde_json::json!({
            "appId": app_id,
            "versionId": version_id,
        });
        let resp: ReleaseVersionResponse = self
            .request(RELEASE_VERSION_MUTATION, Some(vars), None, None)
            .await?;
        Ok(resp.app_release_create)
    }

    /// Subscribe to app logs; returns the JWT token used for polling.
    pub async fn subscribe_to_app_logs(
        &self,
        shop_ids: &[i64],
        api_key: &str,
    ) -> Result<String, GraphqlRequestError> {
        use crate::api::generated::graphql::app_management::app_logs_subscribe::APP_LOGS_SUBSCRIBE_MUTATION;

        let vars = serde_json::json!({
            "shopIds": shop_ids,
            "apiKey": api_key,
        });
        let resp: serde_json::Value = self
            .request(APP_LOGS_SUBSCRIBE_MUTATION, Some(vars), None, None)
            .await?;
        let payload = resp
            .get("appLogsSubscribe")
            .ok_or_else(|| {
                GraphqlRequestError::ApiError(
                    "Failed to subscribe to app logs: No response received".into(),
                    500,
                )
            })?;
        if let Some(errors) = payload.get("errors").and_then(|e| e.as_array()) {
            let msgs: Vec<String> = errors
                .iter()
                .filter_map(|e| e.as_str().map(str::to_string))
                .collect();
            if !msgs.is_empty() {
                return Err(GraphqlRequestError::ApiError(msgs.join(", "), 400));
            }
        }
        payload
            .get("jwtToken")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .ok_or_else(|| {
                GraphqlRequestError::ApiError(
                    "Failed to subscribe to app logs: No JWT token received".into(),
                    500,
                )
            })
    }

    /// Poll the App Management app-logs HTTP endpoint.
    pub async fn fetch_app_logs(
        &self,
        organization_id: &str,
        jwt_token: &str,
        cursor: Option<&str>,
        filters: Option<HashMap<String, String>>,
    ) -> Result<cli_api::AppLogsFetchResult, String> {
        let url = app_management_app_logs_url(organization_id, cursor, filters);
        fetch_app_logs_http(&url, jwt_token).await
    }

    /// Generate a signed upload URL for an app bundle.
    pub async fn generate_signed_upload_url(
        &self,
        source_extension: &str,
        organization_id: &str,
    ) -> Result<SignedUploadUrl, GraphqlRequestError> {
        let vars = serde_json::json!({
            "sourceExtension": source_extension,
            "organizationId": organization_id,
        });
        let resp: CreateAssetUrlResponse = self
            .request(CREATE_ASSET_URL_MUTATION, Some(vars), None, None)
            .await?;
        Ok(resp.app_request_source_upload_url)
    }

    /// Fetch the currently active release for an app by ID.
    pub async fn active_app_version(
        &self,
        app_id: &str,
    ) -> Result<Option<OrganizationApp>, GraphqlRequestError> {
        let vars = serde_json::json!({ "appId": app_id });
        let resp: ActiveReleaseResponse = self
            .request(ACTIVE_RELEASE_QUERY, Some(vars), None, None)
            .await?;
        Ok(resp.app)
    }

    /// Fetch version history for an app.
    pub async fn app_versions(
        &self,
        app_id: &str,
    ) -> Result<Vec<AppVersionNode>, GraphqlRequestError> {
        let vars = serde_json::json!({ "appId": app_id });
        let resp: AppVersionsResponse = self
            .request(APP_VERSIONS_QUERY, Some(vars), None, None)
            .await?;
        Ok(resp
            .app
            .versions
            .map(|c| c.edges.into_iter().map(|e| e.node).collect())
            .unwrap_or_default())
    }

    /// Fetch a specific version by ID.
    pub async fn app_version_by_id(
        &self,
        version_id: &str,
    ) -> Result<Option<VersionDetail>, GraphqlRequestError> {
        let vars = serde_json::json!({ "versionId": version_id });
        let resp: VersionByIdResponse = self
            .request(APP_VERSION_BY_ID_QUERY, Some(vars), None, None)
            .await?;
        Ok(resp.version)
    }

    /// Compute the diff between the current active release and a specific version.
    pub async fn app_versions_diff(
        &self,
        api_key: &str,
        version_id: &str,
    ) -> Result<Option<VersionDiffResult>, GraphqlRequestError> {
        let vars = serde_json::json!({
            "apiKey": api_key,
            "versionId": version_id,
        });
        let resp: AppVersionsDiffResponse = self
            .request(APP_VERSIONS_DIFF_QUERY, Some(vars), None, None)
            .await?;
        Ok(resp.app_by_key.versions_diff)
    }
}

/// A single deprecation entry returned in a GraphQL response's `extensions`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Deprecation {
    pub expired_at: Option<String>,
    pub tacked_on: Option<String>,
    pub path: Option<String>,
}

/// A GraphQL response wrapper that preserves deprecation metadata alongside
/// the decoded data payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WithDeprecations<T> {
    pub data: T,
    pub deprecations: Vec<Deprecation>,
}

/// Parse deprecation entries from a `serde_json::Value` representing the
/// `extensions` object of a GraphQL response.
pub fn handle_deprecations(extensions: Option<&serde_json::Value>) -> Vec<Deprecation> {
    let Some(ext) = extensions else {
        return vec![];
    };
    let Some(deprecations) = ext.get("deprecations") else {
        return vec![];
    };
    let Some(arr) = deprecations.as_array() else {
        return vec![];
    };
    arr.iter()
        .filter_map(|d| serde_json::from_value(d.clone()).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===== Deserialization Tests =====

    #[test]
    fn organization_deserialize() {
        let json = serde_json::json!({ "id": "org-1", "name": "Test Org" });
        let o: Organization = serde_json::from_value(json).unwrap();
        assert_eq!(o.id, "org-1");
        assert_eq!(o.name, "Test Org");
    }

    #[test]
    fn minimal_app_info_deserialize() {
        let json = serde_json::json!({ "id": "app-1", "title": "My App", "key": "abc123" });
        let a: MinimalAppInfo = serde_json::from_value(json).unwrap();
        assert_eq!(a.id, "app-1");
        assert_eq!(a.key, Some("abc123".into()));
    }

    #[test]
    fn organization_app_deserialize() {
        let json = serde_json::json!({
            "id": "app-1",
            "key": "abc123",
            "organizationId": "org-1",
            "activeRoot": {
                "clientCredentials": { "secrets": [{ "key": "shpat_secret" }] },
                "grantedShopifyApprovalScopes": ["write_products"]
            },
            "activeRelease": {
                "id": "rel-1",
                "version": { "name": "1.0.0", "appModules": [] }
            }
        });
        let a: OrganizationApp = serde_json::from_value(json).unwrap();
        assert_eq!(a.key, "abc123");
        assert!(a.active_root.is_some());
        assert!(a.active_release.is_some());
    }

    #[test]
    fn app_module_deserialize() {
        let json = serde_json::json!({
            "uuid": "mod-1",
            "userIdentifier": "ui-1",
            "handle": "my-extension",
            "config": "{}",
            "target": "admin.product.index",
            "specification": {
                "identifier": "ui_extension",
                "externalIdentifier": "123",
                "name": "UI Extension",
                "experience": "LATEST",
                "managementExperience": "STABLE"
            }
        });
        let m: AppModule = serde_json::from_value(json).unwrap();
        assert_eq!(m.uuid, "mod-1");
        assert_eq!(m.specification.as_ref().unwrap().identifier, "ui_extension");
    }

    #[test]
    fn specification_deserialize() {
        let json = serde_json::json!({
            "name": "UI Extension",
            "identifier": "ui_extension",
            "externalIdentifier": "ext_123",
            "experience": "LATEST",
            "features": ["feature1"],
            "uidStrategy": {
                "__typename": "AppModuleUidStrategy",
                "appModuleLimit": 10,
                "isClientProvided": false
            },
            "validationSchema": { "jsonSchema": {} }
        });
        let s: Specification = serde_json::from_value(json).unwrap();
        assert_eq!(s.name, "UI Extension");
        assert!(s.uid_strategy.is_some());
    }

    #[test]
    fn deploy_result_deserialize() {
        let json = serde_json::json!({
            "version": {
                "id": "ver-1",
                "appModules": [],
                "metadata": { "versionTag": "v1", "message": "Initial" }
            },
            "userErrors": []
        });
        let d: DeployResult = serde_json::from_value(json).unwrap();
        assert!(d.version.is_some());
        assert!(d.user_errors.is_empty());
    }

    #[test]
    fn release_result_deserialize() {
        let json = serde_json::json!({
            "release": {
                "version": {
                    "id": "ver-1",
                    "metadata": { "message": "Released", "versionTag": "v1" }
                }
            },
            "userErrors": []
        });
        let r: ReleaseResult = serde_json::from_value(json).unwrap();
        assert!(r.release.is_some());
    }

    #[test]
    fn signed_upload_url_deserialize() {
        let json = serde_json::json!({
            "sourceUploadUrl": "https://example.com/upload",
            "userErrors": []
        });
        let s: SignedUploadUrl = serde_json::from_value(json).unwrap();
        assert_eq!(
            s.source_upload_url,
            Some("https://example.com/upload".into())
        );
    }

    #[test]
    fn create_app_data_deserialize() {
        let json = serde_json::json!({
            "app": {
                "id": "app-1",
                "key": "abc123",
                "activeRoot": {
                    "clientCredentials": { "secrets": [{ "key": "shpat_secret" }] }
                }
            },
            "userErrors": []
        });
        let c: CreateAppData = serde_json::from_value(json).unwrap();
        assert_eq!(c.app.as_ref().unwrap().key, "abc123");
    }

    #[test]
    fn basic_app_info_deserialize() {
        let json = serde_json::json!({ "id": "app-1", "key": "abc123", "installCount": 5 });
        let b: BasicAppInfo = serde_json::from_value(json).unwrap();
        assert_eq!(b.install_count, Some(5));
    }

    #[test]
    fn app_version_node_deserialize() {
        let json = serde_json::json!({
            "id": "ver-1",
            "createdAt": "2025-01-01T00:00:00Z",
            "createdBy": "user@shop.com",
            "metadata": { "message": "v1", "versionTag": "1.0.0" }
        });
        let v: AppVersionNode = serde_json::from_value(json).unwrap();
        assert_eq!(v.id, "ver-1");
        assert!(v.metadata.is_some());
    }

    #[test]
    fn version_detail_deserialize() {
        let json = serde_json::json!({
            "id": "ver-1",
            "metadata": { "message": "v1", "versionTag": "1.0.0" },
            "appModules": []
        });
        let v: VersionDetail = serde_json::from_value(json).unwrap();
        assert_eq!(v.id, "ver-1");
    }

    #[test]
    fn version_diff_result_deserialize() {
        let json = serde_json::json!({
            "added": [],
            "updated": [{
                "uuid": "mod-1",
                "registrationTitle": "My Extension",
                "specification": { "identifier": "ui_extension", "experience": "LATEST" }
            }],
            "removed": []
        });
        let d: VersionDiffResult = serde_json::from_value(json).unwrap();
        assert_eq!(d.updated.len(), 1);
        assert_eq!(
            d.updated[0].registration_title.as_deref(),
            Some("My Extension")
        );
    }

    #[test]
    fn template_specification_deserialize() {
        let json = serde_json::json!({
            "identifier": "ui_extension",
            "name": "UI Extension",
            "defaultName": "my-ext",
            "group": "Admin",
            "sortPriority": 1,
            "supportLinks": ["https://shopify.dev"],
            "types": [{
                "url": "https://example.com",
                "type": "checkout",
                "extensionPoints": ["checkout:render"],
                "supportedFlavors": [{ "name": "React", "value": "react", "path": "react" }]
            }]
        });
        let t: TemplateSpecification = serde_json::from_value(json).unwrap();
        assert_eq!(t.identifier, "ui_extension");
        assert!(t.types.is_some());
    }

    #[test]
    fn deploy_user_error_deserialize() {
        let json = serde_json::json!({
            "field": ["title"],
            "message": "Title is required",
            "category": "VALIDATION",
            "code": "REQUIRED",
            "on": "AppVersion"
        });
        let e: DeployUserError = serde_json::from_value(json).unwrap();
        assert_eq!(e.message.as_deref(), Some("Title is required"));
    }

    // ===== Query String Verification Tests =====

    #[test]
    fn organizations_query_has_organizations() {
        assert!(ORGANIZATIONS_QUERY.contains("organizations"));
    }

    #[test]
    fn create_app_mutation_has_app_create() {
        assert!(CREATE_APP_MUTATION.contains("appCreate"));
    }

    #[test]
    fn update_urls_mutation_has_app_update() {
        assert!(UPDATE_URLS_MUTATION.contains("appUpdate"));
    }

    #[test]
    fn app_from_key_query_has_app_by_key() {
        assert!(APP_FROM_KEY_QUERY.contains("appByKey"));
    }

    #[test]
    fn specifications_query_has_specifications() {
        assert!(SPECIFICATIONS_QUERY.contains("specifications"));
    }

    #[test]
    fn create_app_version_mutation_has_version_create() {
        assert!(CREATE_APP_VERSION_MUTATION.contains("appVersionCreate"));
    }

    #[test]
    fn release_version_mutation_has_app_release_create() {
        assert!(RELEASE_VERSION_MUTATION.contains("appReleaseCreate"));
    }

    #[test]
    fn create_asset_url_mutation_has_upload_url() {
        assert!(CREATE_ASSET_URL_MUTATION.contains("appRequestSourceUploadUrl"));
    }

    #[test]
    fn app_versions_query_has_versions() {
        assert!(APP_VERSIONS_QUERY.contains("versions"));
    }

    #[test]
    fn app_versions_diff_query_has_diff() {
        assert!(APP_VERSIONS_DIFF_QUERY.contains("versionsDiff"));
    }

    // ===== Client Tests =====

    #[test]
    fn client_new_sets_token() {
        let client = AppManagementClient::new("shpat_test".into(), None);
        assert_eq!(client.token, "shpat_test");
    }

    #[test]
    fn client_new_sets_env() {
        let mut env = HashMap::new();
        env.insert("SHOPIFY_SERVICE_ENV".to_string(), "production".to_string());
        let client = AppManagementClient::new("t".into(), Some(env.clone()));
        assert_eq!(client.env, Some(env));
    }

    #[test]
    fn client_new_env_none() {
        let client = AppManagementClient::new("t".into(), None);
        assert!(client.env.is_none());
    }

    // ===== URL Tests =====

    #[test]
    fn headers_contains_auth() {
        let headers = app_management_headers("shpat_test");
        assert!(headers.get("authorization").is_some());
    }

    #[test]
    fn app_logs_url_has_org_id() {
        let url = app_management_app_logs_url("org-123", None, None);
        assert!(url.contains("org-123"));
    }

    #[test]
    fn app_logs_url_contains_fqdn() {
        let fqdn = app_management_fqdn(None);
        let url = app_management_app_logs_url("org-1", None, None);
        assert!(url.contains(&fqdn));
    }

    #[test]
    fn app_logs_url_includes_cursor() {
        let url = app_management_app_logs_url("org-1", Some("cursor_abc"), None);
        assert!(url.contains("cursor_abc"));
    }

    #[test]
    fn app_logs_url_includes_filters() {
        let mut filters = HashMap::new();
        filters.insert("status".to_string(), "active".to_string());
        let url = app_management_app_logs_url("org-1", None, Some(filters));
        assert!(url.contains("status") || url.contains("active"));
    }

    // ===== Deprecation Tests =====

    #[test]
    fn deprecation_deserialize_all_fields() {
        let json = serde_json::json!({
            "expired_at": "2025-06-01",
            "tacked_on": "Use v2 instead",
            "path": "mutation.createApp",
        });
        let d: Deprecation = serde_json::from_value(json).unwrap();
        assert_eq!(d.expired_at.as_deref(), Some("2025-06-01"));
        assert_eq!(d.tacked_on.as_deref(), Some("Use v2 instead"));
        assert_eq!(d.path.as_deref(), Some("mutation.createApp"));
    }

    #[test]
    fn deprecation_deserialize_partial() {
        let json = serde_json::json!({"expired_at": "2025-06-01"});
        let d: Deprecation = serde_json::from_value(json).unwrap();
        assert_eq!(d.expired_at.as_deref(), Some("2025-06-01"));
        assert!(d.tacked_on.is_none());
        assert!(d.path.is_none());
    }

    #[test]
    fn handle_deprecations_returns_empty_for_none() {
        let result = handle_deprecations(None);
        assert!(result.is_empty());
    }

    #[test]
    fn handle_deprecations_returns_empty_for_no_deprecations_key() {
        let val = serde_json::json!({"cost": {"actualQueryCost": 1.0}});
        let result = handle_deprecations(Some(&val));
        assert!(result.is_empty());
    }

    #[test]
    fn handle_deprecations_parses_array() {
        let val = serde_json::json!({
            "deprecations": [
                {"expired_at": "2025-06-01", "path": "mutation.createApp"},
                {"expired_at": "2025-07-01", "path": "query.shop"},
            ]
        });
        let result = handle_deprecations(Some(&val));
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].path.as_deref(), Some("mutation.createApp"));
    }

    #[test]
    fn handle_deprecations_allows_entries_with_empty_fields() {
        let val = serde_json::json!({
            "deprecations": [
                {"expired_at": "2025-06-01"},
                {"invalid": true},
            ]
        });
        let result = handle_deprecations(Some(&val));
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn with_deprecations_struct() {
        let wd = WithDeprecations {
            data: 42u32,
            deprecations: vec![Deprecation {
                expired_at: None,
                tacked_on: None,
                path: Some("/test".into()),
            }],
        };
        assert_eq!(wd.data, 42);
        assert_eq!(wd.deprecations.len(), 1);
    }

    // ===== Rate Limiter Tests =====

    #[tokio::test]
    async fn rate_limiter_acquires_permit() {
        let limiter = app_management_rate_limiter();
        let permit = limiter.acquire().await;
        drop(permit);
    }

    #[tokio::test]
    async fn rate_limiter_static_returns_same() {
        let a = app_management_rate_limiter();
        let b = app_management_rate_limiter();
        let pa = a.acquire().await;
        let pb = b.acquire().await;
        drop(pa);
        drop(pb);
    }

    // ===== Wiremock Tests (3 key methods) =====

    #[tokio::test]
    async fn organizations_returns_list() {
        let mock_server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .and(wiremock::matchers::path("/"))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "data": {
                        "organizations": {
                            "nodes": [
                                { "id": "1", "name": "Org One" },
                                { "id": "2", "name": "Org Two" },
                            ]
                        }
                    },
                    "extensions": { "cost": { "actualQueryCost": null, "throttleStatus": null } }
                })),
            )
            .mount(&mock_server)
            .await;

        let client = mock_app_management_client(&mock_server);
        let orgs = client.organizations().await.unwrap();
        assert_eq!(orgs.len(), 2);
        assert_eq!(orgs[0].name, "Org One");
        assert_eq!(orgs[1].id, "2");
    }

    #[tokio::test]
    async fn organizations_returns_empty_list() {
        let mock_server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .and(wiremock::matchers::path("/"))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "data": { "organizations": { "nodes": [] } },
                    "extensions": { "cost": { "actualQueryCost": null, "throttleStatus": null } }
                })),
            )
            .mount(&mock_server)
            .await;

        let client = mock_app_management_client(&mock_server);
        let orgs = client.organizations().await.unwrap();
        assert!(orgs.is_empty());
    }

    #[tokio::test]
    async fn specifications_returns_list() {
        let mock_server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .and(wiremock::matchers::path("/"))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "data": {
                        "specifications": [{
                            "name": "UI Extension",
                            "identifier": "ui_extension",
                            "externalIdentifier": "ext_1",
                            "experience": "LATEST",
                            "features": ["feature1"],
                            "uidStrategy": {
                                "__typename": "AppModuleUidStrategy",
                                "appModuleLimit": 10,
                                "isClientProvided": false
                            },
                            "validationSchema": { "jsonSchema": {} }
                        }]
                    },
                    "extensions": { "cost": { "actualQueryCost": null, "throttleStatus": null } }
                })),
            )
            .mount(&mock_server)
            .await;

        let client = mock_app_management_client(&mock_server);
        let specs = client.specifications("org-1").await.unwrap();
        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0].identifier, "ui_extension");
    }

    #[tokio::test]
    async fn app_versions_returns_versions() {
        let mock_server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .and(wiremock::matchers::path("/"))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "data": {
                        "app": {
                            "id": "app-1",
                            "activeRelease": null,
                            "versions": {
                                "edges": [{
                                    "node": {
                                        "id": "ver-1",
                                        "createdAt": "2025-01-01T00:00:00Z",
                                        "createdBy": "user@shop.com",
                                        "metadata": { "message": "v1", "versionTag": "1.0.0" }
                                    }
                                }]
                            },
                            "versionsCount": 1
                        }
                    },
                    "extensions": { "cost": { "actualQueryCost": null, "throttleStatus": null } }
                })),
            )
            .mount(&mock_server)
            .await;

        let client = mock_app_management_client(&mock_server);
        let versions = client.app_versions("app-1").await.unwrap();
        assert_eq!(versions.len(), 1);
        assert_eq!(versions[0].id, "ver-1");
    }

    fn mock_app_management_client(server: &wiremock::MockServer) -> AppManagementClient {
        let gql = GraphqlClient::new(server.uri(), None);
        AppManagementClient::with_graphql(gql)
    }
}
