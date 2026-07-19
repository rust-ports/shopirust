use crate::api::graphql::{GraphqlClient, GraphqlRequestError};
use crate::api::rate_limiter::ApiRateLimiter;
use crate::api::utilities::add_cursor_and_filters_to_app_logs_url;
use crate::constants::partners_fqdn;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

fn partners_rate_limiter() -> ApiRateLimiter {
    static LIMITER: OnceLock<ApiRateLimiter> = OnceLock::new();
    LIMITER.get_or_init(ApiRateLimiter::shopify_default).clone()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Organization {
    pub id: String,
    pub business_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OrganizationApp {
    pub id: String,
    pub title: String,
    pub api_key: String,
    pub organization_id: Option<String>,
    pub api_secret_keys: Vec<ApiSecretKey>,
    pub app_type: Option<String>,
    pub granted_scopes: Vec<String>,
    pub application_url: Option<String>,
    pub redirect_url_whitelist: Vec<String>,
    pub requested_access_scopes: Option<Vec<String>>,
    pub webhook_api_version: Option<String>,
    pub embedded: bool,
    pub disabled_flags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ApiSecretKey {
    pub secret: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OrganizationStore {
    pub shop_id: String,
    pub link: String,
    pub shop_domain: String,
    pub shop_name: String,
    pub transfer_disabled: bool,
    pub convertable_to_partner_test: bool,
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
pub struct AppVersion {
    pub uuid: String,
    pub id: i64,
    pub message: Option<String>,
    pub version_tag: Option<String>,
    pub location: String,
    pub app_module_versions: Vec<AppModuleVersion>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppModuleVersion {
    pub uuid: String,
    pub registration_uuid: String,
    pub validation_errors: Vec<ValidationError>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ValidationError {
    pub field: Vec<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UserError {
    pub field: Vec<String>,
    pub message: String,
    pub category: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AccountInfo {
    #[serde(rename = "__typename")]
    pub type_name: String,
    pub email: Option<String>,
    pub org_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DeployResult {
    pub app_version: Option<AppVersion>,
    pub user_errors: Vec<UserError>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CreateAppResult {
    pub app: Option<OrganizationApp>,
    pub user_errors: Vec<UserError>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct OrgsResponse {
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
    business_name: String,
    apps: OrgAppConnection,
    stores: Option<StoreConnection>,
}

#[derive(Deserialize, Serialize)]
struct OrgAppConnection {
    page_info: PageInfo,
    nodes: Vec<OrgAppNode>,
}

#[derive(Deserialize, Serialize)]
struct PageInfo {
    has_next_page: bool,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct OrgAppNode {
    id: String,
    title: String,
    api_key: String,
}

#[derive(Deserialize, Serialize)]
struct StoreConnection {
    nodes: Vec<OrganizationStore>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct AppResponse {
    app: Option<OrganizationApp>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct CreateAppResponse {
    app_create: CreateAppResult,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct DeployResponse {
    app_deploy: DeployResult,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateUrlsResponse {
    app_update: UpdateUrlsBody,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateUrlsBody {
    user_errors: Vec<UserError>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExtensionRegistrationsResponse {
    app: RemoteExtensionRegistrations,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct RemoteExtensionRegistrations {
    extension_registrations: Vec<ExtensionRegistration>,
    configuration_registrations: Vec<ExtensionRegistration>,
    dashboard_managed_extension_registrations: Vec<ExtensionRegistration>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct DevStoresResponse {
    organizations: DevStoreOrgConnection,
}

#[derive(Deserialize, Serialize)]
struct DevStoreOrgConnection {
    nodes: Vec<DevStoreOrg>,
}

#[derive(Deserialize, Serialize)]
struct DevStoreOrg {
    id: String,
    stores: Option<StoreConnection>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct CurrentAccountInfoResponse {
    current_account_info: AccountInfo,
}

const ALL_ORGS_QUERY: &str = r#"
query AllOrgs {
  organizations(first: 200) {
    nodes {
      id
      businessName
    }
  }
}
"#;

const FIND_ORG_QUERY: &str = r#"
query FindOrganization($id: ID!, $title: String) {
  organizations(id: $id, first: 1) {
    nodes {
      id
      businessName
      apps(first: 25, title: $title) {
        pageInfo {
          hasNextPage
        }
        nodes {
          id
          title
          apiKey
        }
      }
    }
  }
}
"#;

const FIND_ORG_BASIC_QUERY: &str = r#"
query FindOrganization($id: ID!) {
  organizations(id: $id, first: 1) {
    nodes {
      id
      businessName
    }
  }
}
"#;

const FIND_APP_QUERY: &str = r#"
query FindApp($apiKey: String!) {
  app(apiKey: $apiKey) {
    id
    title
    apiKey
    organizationId
    apiSecretKeys {
      secret
    }
    appType
    grantedScopes
    applicationUrl
    redirectUrlWhitelist
    requestedAccessScopes
    webhookApiVersion
    embedded
    disabledFlags
  }
}
"#;

const CREATE_APP_MUTATION: &str = r#"
mutation AppCreate($org: Int!, $title: String!, $appUrl: Url!, $redir: [Url]!, $type: AppType, $requestedAccessScopes: [String!]) {
  appCreate(input: {organizationID: $org, title: $title, applicationUrl: $appUrl, redirectUrlWhitelist: $redir, appType: $type, requestedAccessScopes: $requestedAccessScopes}) {
    app {
      id
      title
      apiKey
      organizationId
      apiSecretKeys { secret }
      appType
      grantedScopes
      applicationUrl
      redirectUrlWhitelist
      requestedAccessScopes
      webhookApiVersion
      embedded
      disabledFlags
    }
    userErrors {
      field
      message
    }
  }
}
"#;

const APP_DEPLOY_MUTATION: &str = r#"
mutation AppDeploy($apiKey: String!, $bundleUrl: String, $appModules: [AppModuleSettings!], $skipPublish: Boolean, $message: String, $versionTag: String, $commitReference: String) {
  appDeploy(input: {apiKey: $apiKey, bundleUrl: $bundleUrl, appModules: $appModules, skipPublish: $skipPublish, message: $message, versionTag: $versionTag, commitReference: $commitReference}) {
    appVersion {
      uuid
      id
      message
      versionTag
      location
      appModuleVersions {
        uuid
        registrationUuid
        validationErrors {
          message
          field
        }
      }
    }
    userErrors {
      message
      field
      category
    }
  }
}
"#;

const UPDATE_URLS_MUTATION: &str = r#"
mutation appUpdate($apiKey: String!, $applicationUrl: Url!, $redirectUrlWhitelist: [Url]!) {
  appUpdate(input: {apiKey: $apiKey, applicationUrl: $applicationUrl, redirectUrlWhitelist: $redirectUrlWhitelist}) {
    userErrors {
      message
      field
    }
  }
}
"#;

const ALL_EXTENSION_REGISTRATIONS_QUERY: &str = r#"
query allAppExtensionRegistrations($apiKey: String!) {
  app(apiKey: $apiKey) {
    extensionRegistrations {
      id
      uuid
      title
      type
      draftVersion { config context }
      activeVersion { config context }
    }
    configurationRegistrations {
      id
      uuid
      title
      type
      draftVersion { config context }
      activeVersion { config context }
    }
    dashboardManagedExtensionRegistrations {
      id
      uuid
      title
      type
      activeVersion { config context }
      draftVersion { config context }
    }
  }
}
"#;

const DEV_STORES_QUERY: &str = r#"
query DevStoresByOrg($id: ID!) {
  organizations(id: $id, first: 1) {
    nodes {
      id
      stores(first: 500, archived: false, type: [DEVELOPMENT, MANAGED, PLUS_SANDBOX]) {
        nodes {
          shopId
          link
          shopDomain
          shopName
          transferDisabled
          convertableToPartnerTest
        }
      }
    }
  }
}
"#;

const CURRENT_ACCOUNT_INFO_QUERY: &str = r#"
query currentAccountInfo {
  currentAccountInfo {
    __typename
    ... on ServiceAccount {
      orgName
    }
    ... on UserAccount {
      email
    }
  }
}
"#;

/// Client for the Shopify Partners GraphQL API.
///
/// Wraps [`GraphqlClient`] with Partners-specific rate limiting (150 ms
/// minimum interval, 10 max concurrent), FQDN resolution, and cache/auth
/// passthrough. Provides high-level query methods for organizations, apps,
/// extension registrations, dev stores, and deployments.
pub struct PartnersClient {
    graphql: GraphqlClient,
}

impl PartnersClient {
    /// Wrap an existing [`GraphqlClient`] as a Partners client.
    pub fn new(graphql: GraphqlClient) -> Self {
        Self { graphql }
    }

    /// Build a Partners client from a raw token and optional env map.
    ///
    /// The client is configured with the Partners rate limiter and resolves
    /// the FQDN at construction time.
    pub fn new_with_token(
        token: String,
        env: Option<std::collections::HashMap<String, String>>,
    ) -> Self {
        let url = format!(
            "https://{}/api/cli/graphql",
            partners_fqdn(env.as_ref()),
        );
        let graphql = GraphqlClient::new(url, Some(token))
            .with_rate_limiter(partners_rate_limiter());
        Self { graphql }
    }

    /// Consume the client and return the underlying [`GraphqlClient`].
    pub fn into_inner(self) -> GraphqlClient {
        self.graphql
    }

    /// Fetch all organizations accessible with the current token.
    pub async fn organizations(
        &self,
    ) -> Result<Vec<Organization>, GraphqlRequestError> {
        let resp: OrgsResponse = self.graphql.query(ALL_ORGS_QUERY).await?;
        Ok(resp.organizations.nodes)
    }

    /// Fetch an organization by ID, including its apps (optionally filtered
    /// by title).
    pub async fn org_from_id(
        &self,
        id: &str,
        app_title: Option<&str>,
    ) -> Result<Option<OrgWithAppsInfo>, GraphqlRequestError> {
        let vars = serde_json::json!({ "id": id, "title": app_title });
        let resp: OrgDetailResponse = self
            .graphql
            .query_with_variables(FIND_ORG_QUERY, Some(vars))
            .await?;
        Ok(resp
            .organizations
            .nodes
            .into_iter()
            .next()
            .map(|o| OrgWithAppsInfo {
                id: o.id,
                business_name: o.business_name,
                apps: o
                    .apps
                    .nodes
                    .into_iter()
                    .map(|a| MinimalApp {
                        id: a.id,
                        title: a.title,
                        api_key: a.api_key,
                    })
                    .collect(),
                apps_page_info: o.apps.page_info.has_next_page,
            }))
    }

    /// Fetch an organization by ID, returning only its ID and name.
    pub async fn org_from_id_basic(
        &self,
        id: &str,
    ) -> Result<Option<Organization>, GraphqlRequestError> {
        let vars = serde_json::json!({ "id": id });
        let resp: OrgsResponse = self
            .graphql
            .query_with_variables(FIND_ORG_BASIC_QUERY, Some(vars))
            .await?;
        Ok(resp.organizations.nodes.into_iter().next())
    }

    /// Fetch an app by its API key.
    pub async fn app_from_id(
        &self,
        api_key: &str,
    ) -> Result<Option<OrganizationApp>, GraphqlRequestError> {
        let vars = serde_json::json!({ "apiKey": api_key });
        let resp: AppResponse = self
            .graphql
            .query_with_variables(FIND_APP_QUERY, Some(vars))
            .await?;
        Ok(resp.app)
    }

    /// Create a new app in the given organization.
    pub async fn create_app(
        &self,
        org_id: i64,
        title: &str,
        app_url: &str,
        redirect_urls: Vec<&str>,
    ) -> Result<CreateAppResult, GraphqlRequestError> {
        let vars = serde_json::json!({
            "org": org_id,
            "title": title,
            "appUrl": app_url,
            "redir": redirect_urls,
            "type": "undecided",
        });
        let resp: CreateAppResponse = self
            .graphql
            .query_with_variables(CREATE_APP_MUTATION, Some(vars))
            .await?;
        Ok(resp.app_create)
    }

    /// Deploy an app version from a bundle URL.
    pub async fn deploy_app(
        &self,
        api_key: &str,
        bundle_url: &str,
    ) -> Result<DeployResult, GraphqlRequestError> {
        let vars = serde_json::json!({
            "apiKey": api_key,
            "bundleUrl": bundle_url,
        });
        let resp: DeployResponse = self
            .graphql
            .query_with_variables(APP_DEPLOY_MUTATION, Some(vars))
            .await?;
        Ok(resp.app_deploy)
    }

    /// Update the application URL and redirect URL whitelist for an app.
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
        let resp: UpdateUrlsResponse = self
            .graphql
            .query_with_variables(UPDATE_URLS_MUTATION, Some(vars))
            .await?;
        Ok(resp.app_update.user_errors)
    }

    /// Fetch all extension registrations (standard, configuration, and
    /// dashboard-managed) for an app.
    pub async fn extension_registrations(
        &self,
        api_key: &str,
    ) -> Result<Vec<ExtensionRegistration>, GraphqlRequestError> {
        let vars = serde_json::json!({ "apiKey": api_key });
        let resp: ExtensionRegistrationsResponse = self
            .graphql
            .query_with_variables(ALL_EXTENSION_REGISTRATIONS_QUERY, Some(vars))
            .await?;
        let mut all = resp.app.extension_registrations;
        all.extend(resp.app.configuration_registrations);
        all.extend(resp.app.dashboard_managed_extension_registrations);
        Ok(all)
    }

    /// Fetch dev stores for a given organization.
    pub async fn dev_stores_by_org(
        &self,
        org_id: &str,
    ) -> Result<Vec<OrganizationStore>, GraphqlRequestError> {
        let vars = serde_json::json!({ "id": org_id });
        let resp: DevStoresResponse = self
            .graphql
            .query_with_variables(DEV_STORES_QUERY, Some(vars))
            .await?;
        Ok(resp
            .organizations
            .nodes
            .into_iter()
            .next()
            .and_then(|o| o.stores)
            .map(|s| s.nodes)
            .unwrap_or_default())
    }

    /// Fetch the current account info (user email or service account name).
    pub async fn current_account_info(
        &self,
    ) -> Result<AccountInfo, GraphqlRequestError> {
        let resp: CurrentAccountInfoResponse =
            self.graphql.query(CURRENT_ACCOUNT_INFO_QUERY).await?;
        Ok(resp.current_account_info)
    }

    /// Build a Partners app logs polling URL with optional cursor and filters.
    pub fn generate_fetch_app_log_url(
        cursor: Option<&str>,
        filters: Option<std::collections::HashMap<String, String>>,
    ) -> String {
        let fqdn = partners_fqdn(None);
        let url = format!("https://{fqdn}/app_logs/poll");
        add_cursor_and_filters_to_app_logs_url(&url, cursor, filters)
    }
}

/// Summary of an organization with its apps and pagination info.
#[derive(Debug, Clone)]
pub struct OrgWithAppsInfo {
    pub id: String,
    pub business_name: String,
    pub apps: Vec<MinimalApp>,
    pub apps_page_info: bool,
}

/// Minimal app info returned in org listings.
#[derive(Debug, Clone)]
pub struct MinimalApp {
    pub id: String,
    pub title: String,
    pub api_key: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::graphql::GraphqlClient;
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn mock_client(server: &MockServer) -> PartnersClient {
        let gql = GraphqlClient::new(server.uri(), None);
        PartnersClient::new(gql)
    }

    #[test]
    fn new_with_token_sets_fqdn() {
        let client = PartnersClient::new_with_token("shpat_test".into(), None);
        assert!(client.graphql.url.contains("partners.shopify.com"));
    }

    #[test]
    fn generate_fetch_app_log_url_basic() {
        let url = PartnersClient::generate_fetch_app_log_url(None, None);
        assert!(url.starts_with("https://partners.shopify.com/app_logs/poll"));
    }

    #[test]
    fn generate_fetch_app_log_url_with_cursor() {
        let url = PartnersClient::generate_fetch_app_log_url(Some("abc"), None);
        assert!(url.contains("abc"));
    }

    #[tokio::test]
    async fn organizations_returns_list() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": {
                    "organizations": {
                        "nodes": [
                            { "id": "1", "businessName": "Org One" },
                            { "id": "2", "businessName": "Org Two" },
                        ]
                    }
                },
                "extensions": { "cost": { "actualQueryCost": null, "throttleStatus": null } }
            })))
            .mount(&mock_server)
            .await;

        let client = mock_client(&mock_server);
        let orgs = client.organizations().await.unwrap();
        assert_eq!(orgs.len(), 2);
        assert_eq!(orgs[0].business_name, "Org One");
        assert_eq!(orgs[1].id, "2");
    }

    #[tokio::test]
    async fn organizations_empty_list() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": { "organizations": { "nodes": [] } },
                "extensions": { "cost": { "actualQueryCost": null, "throttleStatus": null } }
            })))
            .mount(&mock_server)
            .await;

        let client = mock_client(&mock_server);
        let orgs = client.organizations().await.unwrap();
        assert!(orgs.is_empty());
    }

    #[tokio::test]
    async fn app_from_id_returns_app() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": {
                    "app": {
                        "id": "app-1",
                        "title": "My App",
                        "apiKey": "abc123",
                        "organizationId": "org-1",
                        "apiSecretKeys": [{ "secret": "shpat_secret" }],
                        "appType": "undecided",
                        "grantedScopes": ["write_products"],
                        "applicationUrl": "https://example.com",
                        "redirectUrlWhitelist": ["https://example.com/auth"],
                        "webhookApiVersion": "2024-01",
                        "embedded": false,
                        "disabledFlags": [],
                    }
                },
                "extensions": { "cost": { "actualQueryCost": null, "throttleStatus": null } }
            })))
            .mount(&mock_server)
            .await;

        let client = mock_client(&mock_server);
        let app = client.app_from_id("abc123").await.unwrap().unwrap();
        assert_eq!(app.title, "My App");
        assert_eq!(app.api_key, "abc123");
        assert_eq!(app.api_secret_keys[0].secret, "shpat_secret");
    }

    #[tokio::test]
    async fn app_from_id_returns_none_when_not_found() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": { "app": null },
                "extensions": { "cost": { "actualQueryCost": null, "throttleStatus": null } }
            })))
            .mount(&mock_server)
            .await;

        let client = mock_client(&mock_server);
        let app = client.app_from_id("nonexistent").await.unwrap();
        assert!(app.is_none());
    }

    #[tokio::test]
    async fn create_app_returns_created_app() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": {
                    "appCreate": {
                        "app": {
                            "id": "new-app-1",
                            "title": "New App",
                            "apiKey": "new-key",
                            "organizationId": "1",
                            "apiSecretKeys": [{ "secret": "shpat_new" }],
                            "appType": "undecided",
                            "grantedScopes": [],
                            "applicationUrl": "https://example.com",
                            "redirectUrlWhitelist": [],
                            "webhookApiVersion": "2024-01",
                            "embedded": false,
                            "disabledFlags": [],
                        },
                        "userErrors": []
                    }
                },
                "extensions": { "cost": { "actualQueryCost": null, "throttleStatus": null } }
            })))
            .mount(&mock_server)
            .await;

        let client = mock_client(&mock_server);
        let result = client
            .create_app(1, "New App", "https://example.com", vec![])
            .await
            .unwrap();
        assert_eq!(result.app.as_ref().unwrap().title, "New App");
        assert!(result.user_errors.is_empty());
    }

    #[tokio::test]
    async fn create_app_returns_user_errors() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": {
                    "appCreate": {
                        "app": null,
                        "userErrors": [{ "field": ["title"], "message": "Title is required" }]
                    }
                },
                "extensions": { "cost": { "actualQueryCost": null, "throttleStatus": null } }
            })))
            .mount(&mock_server)
            .await;

        let client = mock_client(&mock_server);
        let result = client
            .create_app(1, "", "https://example.com", vec![])
            .await
            .unwrap();
        assert_eq!(result.user_errors.len(), 1);
        assert_eq!(result.user_errors[0].message, "Title is required");
    }

    #[tokio::test]
    async fn dev_stores_by_org_returns_stores() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": {
                    "organizations": {
                        "nodes": [{
                            "id": "1",
                            "stores": {
                                "nodes": [{
                                    "shopId": "shop-1",
                                    "link": "https://shop1.myshopify.com",
                                    "shopDomain": "shop1.myshopify.com",
                                    "shopName": "Shop One",
                                    "transferDisabled": false,
                                    "convertableToPartnerTest": true
                                }]
                            }
                        }]
                    }
                },
                "extensions": { "cost": { "actualQueryCost": null, "throttleStatus": null } }
            })))
            .mount(&mock_server)
            .await;

        let client = mock_client(&mock_server);
        let stores = client.dev_stores_by_org("1").await.unwrap();
        assert_eq!(stores.len(), 1);
        assert_eq!(stores[0].shop_name, "Shop One");
    }

    #[tokio::test]
    async fn extension_registrations_returns_all_types() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": {
                    "app": {
                        "extensionRegistrations": [{
                            "id": "ext-1",
                            "uuid": "uuid-1",
                            "title": "Checkout UI",
                            "type": "CHECKOUT_UI",
                            "draftVersion": { "config": "{}", "context": null },
                            "activeVersion": { "config": "{}", "context": null }
                        }],
                        "configurationRegistrations": [],
                        "dashboardManagedExtensionRegistrations": []
                    }
                },
                "extensions": { "cost": { "actualQueryCost": null, "throttleStatus": null } }
            })))
            .mount(&mock_server)
            .await;

        let client = mock_client(&mock_server);
        let regs = client.extension_registrations("abc123").await.unwrap();
        assert_eq!(regs.len(), 1);
        assert_eq!(regs[0].title, "Checkout UI");
    }

    #[tokio::test]
    async fn update_urls_succeeds() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": {
                    "appUpdate": {
                        "userErrors": []
                    }
                },
                "extensions": { "cost": { "actualQueryCost": null, "throttleStatus": null } }
            })))
            .mount(&mock_server)
            .await;

        let client = mock_client(&mock_server);
        let errors = client
            .update_urls("abc123", "https://example.com", vec![])
            .await
            .unwrap();
        assert!(errors.is_empty());
    }

    #[tokio::test]
    async fn current_account_info_returns_account() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": {
                    "currentAccountInfo": {
                        "__typename": "UserAccount",
                        "email": "test@shopify.com"
                    }
                },
                "extensions": { "cost": { "actualQueryCost": null, "throttleStatus": null } }
            })))
            .mount(&mock_server)
            .await;

        let client = mock_client(&mock_server);
        let info = client.current_account_info().await.unwrap();
        assert_eq!(info.type_name, "UserAccount");
        assert_eq!(info.email, Some("test@shopify.com".into()));
    }

    #[tokio::test]
    async fn org_from_id_basic_returns_org() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": {
                    "organizations": {
                        "nodes": [{ "id": "1", "businessName": "Test Org" }]
                    }
                },
                "extensions": { "cost": { "actualQueryCost": null, "throttleStatus": null } }
            })))
            .mount(&mock_server)
            .await;

        let client = mock_client(&mock_server);
        let org = client.org_from_id_basic("1").await.unwrap().unwrap();
        assert_eq!(org.business_name, "Test Org");
    }

    #[tokio::test]
    async fn deploy_app_returns_result() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": {
                    "appDeploy": {
                        "appVersion": {
                            "uuid": "ver-uuid",
                            "id": 1,
                            "message": null,
                            "versionTag": null,
                            "location": "https://storage.example.com/bundle.zip",
                            "appModuleVersions": []
                        },
                        "userErrors": []
                    }
                },
                "extensions": { "cost": { "actualQueryCost": null, "throttleStatus": null } }
            })))
            .mount(&mock_server)
            .await;

        let client = mock_client(&mock_server);
        let result = client
            .deploy_app("abc123", "https://bundle.url")
            .await
            .unwrap();
        assert!(result.app_version.is_some());
        assert_eq!(result.app_version.unwrap().uuid, "ver-uuid");
    }
}
