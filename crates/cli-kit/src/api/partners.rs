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
    #[serde(default)]
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionCreateRegistration {
    pub id: String,
    pub uuid: String,
    #[serde(rename = "type")]
    pub type_name: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionCreateResult {
    pub extension_registration: Option<ExtensionCreateRegistration>,
    #[serde(default)]
    pub user_errors: Vec<UserError>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExtensionCreateResponse {
    extension_create: ExtensionCreateResult,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SignedUploadUrlResult {
    pub signed_upload_url: Option<String>,
    #[serde(default)]
    pub user_errors: Vec<UserError>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct SignedUploadUrlResponse {
    app_version_generate_signed_upload_url: SignedUploadUrlResult,
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

const EXTENSION_CREATE_MUTATION: &str = r#"
mutation ExtensionCreate(
  $apiKey: String!
  $type: ExtensionType!
  $title: String!
  $config: JSON!
  $context: String
  $handle: String
) {
  extensionCreate(
    input: {apiKey: $apiKey, type: $type, title: $title, config: $config, context: $context, handle: $handle}
  ) {
    extensionRegistration {
      id
      uuid
      type
      title
    }
    userErrors {
      field
      message
    }
  }
}
"#;

const EXTENSION_SPECIFICATIONS_QUERY: &str = r#"
query fetchSpecifications($apiKey: String!) {
  extensionSpecifications(apiKey: $apiKey) {
    name
    identifier
    experience
    options { managementExperience registrationLimit }
    validationSchema { jsonSchema }
  }
}
"#;

const APP_VERSIONS_QUERY: &str = r#"
query AppVersionsQuery($apiKey: String!) {
  app(apiKey: $apiKey) {
    id
    title
    appVersions {
      nodes { createdAt message status versionTag }
    }
  }
}
"#;

const TEMPLATE_SPECIFICATIONS_QUERY: &str = r#"
query RemoteTemplateSpecifications($version: String, $apiKey: String) {
  templateSpecifications(version: $version, apiKey: $apiKey) {
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

const ACTIVE_APP_VERSION_QUERY: &str = r#"
query activeAppVersion($apiKey: String!) {
  app(apiKey: $apiKey) {
    activeAppVersion {
      appModuleVersions {
        registrationId
        registrationUuid
        registrationTitle
        type
        config
        specification {
          identifier
          name
          experience
          options { managementExperience }
        }
      }
    }
  }
}
"#;

const APP_VERSION_BY_TAG_QUERY: &str = r#"
query AppVersionByTag($apiKey: String!, $versionTag: String!) {
  app(apiKey: $apiKey) {
    appVersion(versionTag: $versionTag) {
      id
      uuid
      versionTag
      location
      message
      appModuleVersions {
        registrationId
        registrationUuid
        registrationTitle
        type
        config
        specification {
          identifier
          name
          experience
          options { managementExperience }
        }
      }
    }
  }
}
"#;

const APP_VERSIONS_DIFF_QUERY: &str = r#"
query AppVersionsDiff($apiKey: String!, $versionId: ID!) {
  app(apiKey: $apiKey) {
    versionsDiff(appVersionId: $versionId) {
      added {
        uuid
        registrationTitle
        specification {
          identifier
          experience
          options { managementExperience }
        }
      }
      updated {
        uuid
        registrationTitle
        specification {
          identifier
          experience
          options { managementExperience }
        }
      }
      removed {
        uuid
        registrationTitle
        specification {
          identifier
          experience
          options { managementExperience }
        }
      }
    }
  }
}
"#;

const FIND_STORE_BY_DOMAIN_QUERY: &str = r#"
query FindOrganization($orgId: ID!, $shopDomain: String) {
  organizations(id: $orgId, first: 1) {
    nodes {
      id
      businessName
      stores(shopDomain: $shopDomain, first: 1, archived: false) {
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

const APP_RELEASE_MUTATION: &str = r#"
mutation AppRelease($apiKey: String!, $appVersionId: ID, $versionTag: String) {
  appRelease(input: {apiKey: $apiKey, appVersionId: $appVersionId, versionTag: $versionTag}) {
    appVersion { versionTag message location }
    userErrors { message field }
  }
}
"#;

const MIGRATE_APP_MODULE_MUTATION: &str = r#"
mutation MigrateAppModule($apiKey: String!, $registrationUuid: String, $type: String!) {
  migrateAppModule(input: {apiKey: $apiKey, registrationUuid: $registrationUuid, type: $type}) {
    migratedAppModule
    userErrors { field message }
  }
}
"#;

const MIGRATE_FLOW_EXTENSION_MUTATION: &str = r#"
mutation MigrateFlowExtension($apiKey: String!, $registrationUuid: String) {
  migrateFlowExtension(input: {apiKey: $apiKey, registrationUuid: $registrationUuid}) {
    migratedFlowExtension
    userErrors { field message }
  }
}
"#;

const MIGRATE_TO_UI_EXTENSION_MUTATION: &str = r#"
mutation MigrateToUiExtension($apiKey: String!, $registrationUuid: String) {
  migrateToUiExtension(input: {apiKey: $apiKey, registrationUuid: $registrationUuid}) {
    migratedToUiExtension
    userErrors { field message }
  }
}
"#;

const CONVERT_DEV_TO_TEST_STORE_MUTATION: &str = r#"
mutation convertDevToTestStore($input: ConvertDevToTestStoreInput!) {
  convertDevToTestStore(input: $input) {
    convertedToTestStore
    userErrors { message field }
  }
}
"#;

const FIND_APP_PREVIEW_MODE_QUERY: &str = r#"
query FindAppPreviewMode($apiKey: String!) {
  app(apiKey: $apiKey) { developmentStorePreviewEnabled }
}
"#;

const DEVELOPMENT_STORE_PREVIEW_UPDATE: &str = r#"
mutation DevelopmentStorePreviewUpdate($input: DevelopmentStorePreviewUpdateInput!) {
  developmentStorePreviewUpdate(input: $input) {
    app { id developmentStorePreviewEnabled }
    userErrors { message field }
  }
}
"#;

const GENERATE_SIGNED_UPLOAD_URL_MUTATION: &str = r#"
mutation GenerateSignedUploadUrl($apiKey: String!, $bundleFormat: Int!) {
  appVersionGenerateSignedUploadUrl(input: {apiKey: $apiKey, bundleFormat: $bundleFormat}) {
    signedUploadUrl
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PartnerAppModule {
    registration_id: Option<String>,
    registration_uuid: Option<String>,
    registration_title: Option<String>,
    #[serde(rename = "type")]
    module_type: Option<String>,
    config: Option<String>,
}

fn map_partner_modules(modules: Vec<PartnerAppModule>) -> Vec<cli_api::AppModuleVersion> {
    modules
        .into_iter()
        .map(|m| cli_api::AppModuleVersion {
            registration_id: m.registration_id.unwrap_or_default(),
            registration_uuid: m.registration_uuid,
            registration_title: m.registration_title.unwrap_or_default(),
            config: m.config.and_then(|raw| serde_json::from_str(&raw).ok()),
            target: None,
            module_type: m.module_type.unwrap_or_default(),
        })
        .collect()
}

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
        let url = format!("https://{}/api/cli/graphql", partners_fqdn(env.as_ref()),);
        let graphql =
            GraphqlClient::new(url, Some(token)).with_rate_limiter(partners_rate_limiter());
        Self { graphql }
    }

    /// Consume the client and return the underlying [`GraphqlClient`].
    pub fn into_inner(self) -> GraphqlClient {
        self.graphql
    }

    /// Fetch all organizations accessible with the current token.
    pub async fn organizations(&self) -> Result<Vec<Organization>, GraphqlRequestError> {
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
        requested_access_scopes: &[String],
    ) -> Result<CreateAppResult, GraphqlRequestError> {
        let vars = serde_json::json!({
            "org": org_id,
            "title": title,
            "appUrl": app_url,
            "redir": redirect_urls,
            "type": "undecided",
            "requestedAccessScopes": requested_access_scopes,
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
        self.deploy_app_input(serde_json::json!({
            "apiKey": api_key,
            "bundleUrl": bundle_url,
        }))
        .await
    }

    /// Deploy with full Partners `appDeploy` variables (`appModules`, `skipPublish`, …).
    pub async fn deploy_app_input(
        &self,
        vars: serde_json::Value,
    ) -> Result<DeployResult, GraphqlRequestError> {
        let resp: DeployResponse = self
            .graphql
            .query_with_variables(APP_DEPLOY_MUTATION, Some(vars))
            .await?;
        Ok(resp.app_deploy)
    }

    pub async fn create_extension(
        &self,
        input: &cli_api::types::ExtensionCreateInput,
    ) -> Result<ExtensionCreateResult, GraphqlRequestError> {
        let vars = serde_json::json!({
            "apiKey": input.api_key,
            "type": input.type_name,
            "title": input.title,
            "config": input.config,
            "context": input.context,
            "handle": input.handle,
        });
        let resp: ExtensionCreateResponse = self
            .graphql
            .query_with_variables(EXTENSION_CREATE_MUTATION, Some(vars))
            .await?;
        Ok(resp.extension_create)
    }

    pub async fn update_extension_draft(
        &self,
        input: &cli_api::types::ExtensionUpdateDraftInput,
    ) -> Result<cli_api::types::ExtensionUpdateDraftResult, GraphqlRequestError> {
        use crate::api::generated::graphql::partners::update_draft::{
            ExtensionUpdateDraftResponse, EXTENSION_UPDATE_DRAFT_MUTATION,
        };
        let config_value = serde_json::from_str::<serde_json::Value>(&input.config)
            .unwrap_or_else(|_| serde_json::Value::String(input.config.clone()));
        let vars = serde_json::json!({
            "apiKey": input.api_key,
            "registrationId": input.registration_id,
            "config": config_value,
            "context": input.context,
            "handle": input.handle,
        });
        let resp: ExtensionUpdateDraftResponse = self
            .graphql
            .query_with_variables(EXTENSION_UPDATE_DRAFT_MUTATION, Some(vars))
            .await?;
        let user_errors = resp
            .extension_update_draft
            .and_then(|d| d.user_errors)
            .unwrap_or_default()
            .into_iter()
            .map(|e| cli_api::types::UserError {
                field: e.field,
                message: e.message,
            })
            .collect();
        Ok(cli_api::types::ExtensionUpdateDraftResult { user_errors })
    }

    pub async fn generate_signed_upload_url(
        &self,
        api_key: &str,
        bundle_format: i64,
    ) -> Result<SignedUploadUrlResult, GraphqlRequestError> {
        let vars = serde_json::json!({
            "apiKey": api_key,
            "bundleFormat": bundle_format,
        });
        let resp: SignedUploadUrlResponse = self
            .graphql
            .query_with_variables(GENERATE_SIGNED_UPLOAD_URL_MUTATION, Some(vars))
            .await?;
        Ok(resp.app_version_generate_signed_upload_url)
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
    pub async fn current_account_info(&self) -> Result<AccountInfo, GraphqlRequestError> {
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

    /// Base Partners app-logs poll URL (no query string).
    pub fn app_logs_poll_base_url() -> String {
        let fqdn = partners_fqdn(None);
        format!("https://{fqdn}/app_logs/poll")
    }

    /// Subscribe to app logs; returns the JWT token used for polling.
    pub async fn subscribe_to_app_logs(
        &self,
        shop_ids: &[i64],
        api_key: &str,
    ) -> Result<String, GraphqlRequestError> {
        use crate::api::generated::graphql::app_management::app_logs_subscribe::{
            AppLogsSubscribeResponse, AppLogsSubscribeVariables, APP_LOGS_SUBSCRIBE_MUTATION,
        };
        use crate::api::generated::graphql::app_management::types::OneOrMany;

        let vars = AppLogsSubscribeVariables {
            shop_ids: OneOrMany::Many(shop_ids.to_vec()),
            api_key: api_key.to_string(),
        };
        let resp: AppLogsSubscribeResponse = self
            .graphql
            .query_with_variables(APP_LOGS_SUBSCRIBE_MUTATION, Some(vars))
            .await?;
        let payload = resp.app_logs_subscribe.ok_or_else(|| {
            GraphqlRequestError::ApiError(
                "Failed to subscribe to app logs: No response received".into(),
                500,
            )
        })?;
        if let Some(errors) = payload.errors.filter(|e| !e.is_empty()) {
            return Err(GraphqlRequestError::ApiError(errors.join(", "), 400));
        }
        payload.jwt_token.ok_or_else(|| {
            GraphqlRequestError::ApiError(
                "Failed to subscribe to app logs: No JWT token received".into(),
                500,
            )
        })
    }

    pub async fn extension_specifications(
        &self,
        api_key: &str,
    ) -> Result<Vec<cli_api::RemoteSpecification>, GraphqlRequestError> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct SpecNode {
            name: String,
            identifier: String,
            experience: String,
            options: Option<serde_json::Value>,
            validation_schema: Option<serde_json::Value>,
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct SpecsResponse {
            extension_specifications: Vec<SpecNode>,
        }
        let resp: SpecsResponse = self
            .graphql
            .query_with_variables(
                EXTENSION_SPECIFICATIONS_QUERY,
                Some(serde_json::json!({ "apiKey": api_key })),
            )
            .await?;
        Ok(resp
            .extension_specifications
            .into_iter()
            .map(|s| cli_api::RemoteSpecification {
                identifier: s.identifier,
                name: s.name,
                experience: s.experience,
                options: s.options,
                validation_schema: s.validation_schema,
            })
            .collect())
    }

    pub async fn app_versions_list(
        &self,
        api_key: &str,
    ) -> Result<serde_json::Value, GraphqlRequestError> {
        let resp: serde_json::Value = self
            .graphql
            .query_with_variables(
                APP_VERSIONS_QUERY,
                Some(serde_json::json!({ "apiKey": api_key })),
            )
            .await?;
        Ok(resp)
    }

    pub async fn template_specifications(
        &self,
        api_key: &str,
    ) -> Result<cli_api::ExtensionTemplatesResult, GraphqlRequestError> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct TemplateType {
            url: Option<String>,
            #[serde(rename = "type")]
            type_name: Option<String>,
            extension_points: Option<Vec<String>>,
            supported_flavors: Option<serde_json::Value>,
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct RemoteTemplate {
            identifier: String,
            name: String,
            group: Option<String>,
            sort_priority: Option<i64>,
            support_links: Option<Vec<String>>,
            types: Vec<TemplateType>,
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Resp {
            template_specifications: Vec<RemoteTemplate>,
        }

        let resp: Resp = self
            .graphql
            .query_with_variables(
                TEMPLATE_SPECIFICATIONS_QUERY,
                Some(serde_json::json!({ "apiKey": api_key })),
            )
            .await?;

        let mut group_order = Vec::new();
        let mut counter = 0i64;
        let templates = resp
            .template_specifications
            .into_iter()
            .map(|t| {
                let _ = t.sort_priority.unwrap_or_else(|| {
                    let n = counter;
                    counter += 1;
                    n
                });
                if let Some(group) = t.group.as_deref() {
                    if !group.is_empty() && !group_order.iter().any(|g| g == group) {
                        group_order.push(group.to_string());
                    }
                }
                let url = t
                    .types
                    .first()
                    .and_then(|ty| ty.url.clone())
                    .or_else(|| t.support_links.as_ref().and_then(|l| l.first().cloned()));
                let types = t
                    .types
                    .into_iter()
                    .map(|ty| {
                        serde_json::json!({
                            "url": ty.url,
                            "type": ty.type_name,
                            "extensionPoints": ty.extension_points,
                            "supportedFlavors": ty.supported_flavors,
                        })
                    })
                    .collect();
                cli_api::ExtensionTemplate {
                    identifier: t.identifier,
                    name: t.name,
                    group: t.group,
                    url,
                    types,
                }
            })
            .collect();
        Ok(cli_api::ExtensionTemplatesResult {
            templates,
            group_order,
        })
    }

    pub async fn active_app_version(
        &self,
        api_key: &str,
    ) -> Result<Option<cli_api::AppVersion>, GraphqlRequestError> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Active {
            app_module_versions: Option<Vec<PartnerAppModule>>,
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct App {
            active_app_version: Option<Active>,
        }
        #[derive(Deserialize)]
        struct Resp {
            app: Option<App>,
        }

        let resp: Resp = self
            .graphql
            .query_with_variables(
                ACTIVE_APP_VERSION_QUERY,
                Some(serde_json::json!({ "apiKey": api_key })),
            )
            .await?;
        Ok(resp
            .app
            .and_then(|a| a.active_app_version)
            .map(|v| cli_api::AppVersion {
                app_module_versions: map_partner_modules(v.app_module_versions.unwrap_or_default()),
            }))
    }

    pub async fn app_version_by_tag(
        &self,
        api_key: &str,
        version_tag: &str,
    ) -> Result<cli_api::AppVersionWithContext, GraphqlRequestError> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Version {
            id: i64,
            uuid: String,
            version_tag: Option<String>,
            app_module_versions: Option<Vec<PartnerAppModule>>,
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct App {
            app_version: Option<Version>,
        }
        #[derive(Deserialize)]
        struct Resp {
            app: Option<App>,
        }

        let resp: Resp = self
            .graphql
            .query_with_variables(
                APP_VERSION_BY_TAG_QUERY,
                Some(serde_json::json!({ "apiKey": api_key, "versionTag": version_tag })),
            )
            .await?;
        let version = resp.app.and_then(|a| a.app_version).ok_or_else(|| {
            GraphqlRequestError::ApiError(format!("Version {version_tag} not found"), 404)
        })?;
        Ok(cli_api::AppVersionWithContext {
            id: version.id,
            uuid: version.uuid,
            version_tag: version.version_tag,
            app_module_versions: map_partner_modules(
                version.app_module_versions.unwrap_or_default(),
            ),
        })
    }

    pub async fn app_versions_diff(
        &self,
        api_key: &str,
        version_id: i64,
    ) -> Result<serde_json::Value, GraphqlRequestError> {
        self.graphql
            .query_with_variables(
                APP_VERSIONS_DIFF_QUERY,
                Some(serde_json::json!({ "apiKey": api_key, "versionId": version_id })),
            )
            .await
    }

    pub async fn find_store_by_domain(
        &self,
        org_id: &str,
        shop_domain: &str,
    ) -> Result<Option<OrganizationStore>, GraphqlRequestError> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Stores {
            nodes: Vec<OrganizationStore>,
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Org {
            stores: Option<Stores>,
        }
        #[derive(Deserialize)]
        struct Connection {
            nodes: Vec<Org>,
        }
        #[derive(Deserialize)]
        struct Resp {
            organizations: Connection,
        }

        let resp: Resp = self
            .graphql
            .query_with_variables(
                FIND_STORE_BY_DOMAIN_QUERY,
                Some(serde_json::json!({ "orgId": org_id, "shopDomain": shop_domain })),
            )
            .await?;
        Ok(resp
            .organizations
            .nodes
            .into_iter()
            .next()
            .and_then(|o| o.stores)
            .and_then(|s| s.nodes.into_iter().next()))
    }

    pub async fn release_app_version(
        &self,
        api_key: &str,
        version_tag: Option<&str>,
        app_version_id: Option<i64>,
    ) -> Result<serde_json::Value, GraphqlRequestError> {
        self.graphql
            .query_with_variables(
                APP_RELEASE_MUTATION,
                Some(serde_json::json!({
                    "apiKey": api_key,
                    "versionTag": version_tag,
                    "appVersionId": app_version_id,
                })),
            )
            .await
    }

    pub async fn migrate_app_module(
        &self,
        api_key: &str,
        registration_uuid: &str,
        type_name: &str,
    ) -> Result<bool, GraphqlRequestError> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Inner {
            migrated_app_module: Option<bool>,
            user_errors: Option<Vec<UserError>>,
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Resp {
            migrate_app_module: Inner,
        }
        let resp: Resp = self
            .graphql
            .query_with_variables(
                MIGRATE_APP_MODULE_MUTATION,
                Some(serde_json::json!({
                    "apiKey": api_key,
                    "registrationUuid": registration_uuid,
                    "type": type_name,
                })),
            )
            .await?;
        if resp
            .migrate_app_module
            .user_errors
            .as_ref()
            .is_some_and(|e| !e.is_empty())
        {
            return Ok(false);
        }
        Ok(resp.migrate_app_module.migrated_app_module.unwrap_or(false))
    }

    pub async fn migrate_flow_extension(
        &self,
        api_key: &str,
        registration_uuid: &str,
    ) -> Result<bool, GraphqlRequestError> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Inner {
            migrated_flow_extension: Option<bool>,
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Resp {
            migrate_flow_extension: Inner,
        }
        let resp: Resp = self
            .graphql
            .query_with_variables(
                MIGRATE_FLOW_EXTENSION_MUTATION,
                Some(serde_json::json!({
                    "apiKey": api_key,
                    "registrationUuid": registration_uuid,
                })),
            )
            .await?;
        Ok(resp
            .migrate_flow_extension
            .migrated_flow_extension
            .unwrap_or(false))
    }

    pub async fn migrate_to_ui_extension(
        &self,
        api_key: &str,
        registration_uuid: &str,
    ) -> Result<bool, GraphqlRequestError> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Inner {
            migrated_to_ui_extension: Option<bool>,
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Resp {
            migrate_to_ui_extension: Inner,
        }
        let resp: Resp = self
            .graphql
            .query_with_variables(
                MIGRATE_TO_UI_EXTENSION_MUTATION,
                Some(serde_json::json!({
                    "apiKey": api_key,
                    "registrationUuid": registration_uuid,
                })),
            )
            .await?;
        Ok(resp
            .migrate_to_ui_extension
            .migrated_to_ui_extension
            .unwrap_or(false))
    }

    pub async fn convert_dev_to_test_store(
        &self,
        organization_id: i64,
        shop_id: &str,
    ) -> Result<bool, GraphqlRequestError> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Inner {
            converted_to_test_store: Option<bool>,
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Resp {
            convert_dev_to_test_store: Inner,
        }
        let resp: Resp = self
            .graphql
            .query_with_variables(
                CONVERT_DEV_TO_TEST_STORE_MUTATION,
                Some(serde_json::json!({
                    "input": { "organizationID": organization_id, "shopId": shop_id }
                })),
            )
            .await?;
        Ok(resp
            .convert_dev_to_test_store
            .converted_to_test_store
            .unwrap_or(false))
    }

    pub async fn app_preview_mode(
        &self,
        api_key: &str,
    ) -> Result<Option<bool>, GraphqlRequestError> {
        #[derive(Deserialize)]
        struct AppNode {
            #[serde(rename = "developmentStorePreviewEnabled")]
            enabled: Option<bool>,
        }
        #[derive(Deserialize)]
        struct Resp {
            app: Option<AppNode>,
        }
        let resp: Resp = self
            .graphql
            .query_with_variables(
                FIND_APP_PREVIEW_MODE_QUERY,
                Some(serde_json::json!({ "apiKey": api_key })),
            )
            .await?;
        Ok(resp.app.and_then(|a| a.enabled))
    }

    pub async fn update_developer_preview(
        &self,
        api_key: &str,
        enabled: bool,
    ) -> Result<bool, GraphqlRequestError> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Inner {
            user_errors: Option<Vec<UserError>>,
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Resp {
            development_store_preview_update: Inner,
        }
        let resp: Resp = self
            .graphql
            .query_with_variables(
                DEVELOPMENT_STORE_PREVIEW_UPDATE,
                Some(serde_json::json!({
                    "input": { "apiKey": api_key, "enabled": enabled }
                })),
            )
            .await?;
        Ok(resp
            .development_store_preview_update
            .user_errors
            .as_ref()
            .map(|e| e.is_empty())
            .unwrap_or(true))
    }

    /// Poll the Partners app-logs HTTP endpoint.
    pub async fn fetch_app_logs(
        &self,
        jwt_token: &str,
        cursor: Option<&str>,
        filters: Option<std::collections::HashMap<String, String>>,
    ) -> Result<cli_api::AppLogsFetchResult, String> {
        let url = Self::generate_fetch_app_log_url(cursor, filters);
        crate::api::app_management::fetch_app_logs_http(&url, jwt_token).await
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
            .create_app(1, "New App", "https://example.com", vec![], &[])
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
            .create_app(1, "", "https://example.com", vec![], &[])
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

    #[tokio::test]
    async fn migrate_app_module_succeeds() {
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": {
                    "migrateAppModule": {
                        "migratedAppModule": true,
                        "userErrors": []
                    }
                }
            })))
            .mount(&mock_server)
            .await;
        let client = mock_client(&mock_server);
        assert!(client
            .migrate_app_module("k", "uuid", "payments_extension")
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn specifications_map_identifier() {
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": {
                    "extensionSpecifications": [
                        { "name": "Theme", "identifier": "theme", "experience": "extension", "options": null }
                    ]
                }
            })))
            .mount(&mock_server)
            .await;
        let client = mock_client(&mock_server);
        let specs = client.extension_specifications("k").await.unwrap();
        assert_eq!(specs[0].identifier, "theme");
    }

    #[tokio::test]
    async fn convert_dev_to_test_store_succeeds() {
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": {
                    "convertDevToTestStore": { "convertedToTestStore": true, "userErrors": [] }
                }
            })))
            .mount(&mock_server)
            .await;
        let client = mock_client(&mock_server);
        assert!(client.convert_dev_to_test_store(1, "shop-1").await.unwrap());
    }

    #[tokio::test]
    async fn template_specifications_maps_types_and_group_order() {
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": {
                    "templateSpecifications": [
                        {
                            "identifier": "checkout_ui",
                            "name": "Checkout UI",
                            "group": "Checkout",
                            "sortPriority": 0,
                            "supportLinks": [],
                            "types": [{ "url": "https://github.com/Shopify/checkout-ui", "type": "checkout_ui_extension", "extensionPoints": [], "supportedFlavors": [] }]
                        },
                        {
                            "identifier": "theme",
                            "name": "Theme",
                            "group": "Online store",
                            "types": [{ "url": "https://github.com/Shopify/theme-ext", "type": "theme_app_extension" }]
                        }
                    ]
                },
                "extensions": { "cost": { "actualQueryCost": null, "throttleStatus": null } }
            })))
            .mount(&mock_server)
            .await;
        let client = mock_client(&mock_server);
        let result = client.template_specifications("key").await.unwrap();
        assert_eq!(result.templates.len(), 2);
        assert_eq!(
            result.templates[0].url.as_deref(),
            Some("https://github.com/Shopify/checkout-ui")
        );
        assert_eq!(result.group_order, vec!["Checkout", "Online store"]);
    }

    #[tokio::test]
    async fn active_app_version_parses_module_config() {
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": {
                    "app": {
                        "activeAppVersion": {
                            "appModuleVersions": [{
                                "registrationId": "1",
                                "registrationUuid": "uuid-1",
                                "registrationTitle": "Checkout",
                                "type": "checkout_ui_extension",
                                "config": "{\"name\":\"x\"}"
                            }]
                        }
                    }
                },
                "extensions": { "cost": { "actualQueryCost": null, "throttleStatus": null } }
            })))
            .mount(&mock_server)
            .await;
        let client = mock_client(&mock_server);
        let version = client.active_app_version("key").await.unwrap().unwrap();
        assert_eq!(version.app_module_versions[0].registration_id, "1");
        assert_eq!(
            version.app_module_versions[0]
                .config
                .as_ref()
                .and_then(|v| v.get("name"))
                .and_then(|v| v.as_str()),
            Some("x")
        );
    }

    #[tokio::test]
    async fn app_version_by_tag_returns_context() {
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": {
                    "app": {
                        "appVersion": {
                            "id": 42,
                            "uuid": "ver-uuid",
                            "versionTag": "1.0.0",
                            "location": "https://partners.shopify.com/versions/42",
                            "message": "ship it",
                            "appModuleVersions": []
                        }
                    }
                },
                "extensions": { "cost": { "actualQueryCost": null, "throttleStatus": null } }
            })))
            .mount(&mock_server)
            .await;
        let client = mock_client(&mock_server);
        let version = client.app_version_by_tag("key", "1.0.0").await.unwrap();
        assert_eq!(version.id, 42);
        assert_eq!(version.uuid, "ver-uuid");
        assert_eq!(version.version_tag.as_deref(), Some("1.0.0"));
    }

    #[tokio::test]
    async fn app_versions_diff_returns_added() {
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": {
                    "app": {
                        "versionsDiff": {
                            "added": [{ "uuid": "a", "registrationTitle": "New", "specification": { "identifier": "theme", "experience": "extension", "options": { "managementExperience": "cli" } } }],
                            "updated": [],
                            "removed": []
                        }
                    }
                },
                "extensions": { "cost": { "actualQueryCost": null, "throttleStatus": null } }
            })))
            .mount(&mock_server)
            .await;
        let client = mock_client(&mock_server);
        let diff = client.app_versions_diff("key", 42).await.unwrap();
        assert_eq!(diff["app"]["versionsDiff"]["added"][0]["uuid"], "a");
    }

    #[tokio::test]
    async fn find_store_by_domain_returns_shop() {
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": {
                    "organizations": {
                        "nodes": [{
                            "id": "1",
                            "businessName": "Org",
                            "stores": {
                                "nodes": [{
                                    "shopId": "shop-1",
                                    "link": "https://demo.myshopify.com",
                                    "shopDomain": "demo.myshopify.com",
                                    "shopName": "Demo",
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
        let store = client
            .find_store_by_domain("1", "demo.myshopify.com")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(store.shop_domain, "demo.myshopify.com");
    }
}
