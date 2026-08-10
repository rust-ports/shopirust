use serde::{Deserialize, Serialize};

/// Which developer-platform backend a client targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ClientName {
    AppManagement,
    Partners,
}

impl ClientName {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AppManagement => "app-management",
            Self::Partners => "partners",
        }
    }
}

/// Where organization records come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OrganizationSource {
    Partners,
    BusinessPlatform,
}

/// Bundle compression used when uploading app versions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BundleFormat {
    Zip,
    Br,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Organization {
    pub id: String,
    pub business_name: String,
    pub source: OrganizationSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MinimalAppIdentifiers {
    pub api_key: String,
    pub organization_id: String,
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MinimalOrganizationApp {
    #[serde(flatten)]
    pub identifiers: MinimalAppIdentifiers,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrganizationApp {
    pub id: String,
    pub title: String,
    pub api_key: String,
    pub organization_id: Option<String>,
    pub api_secret_keys: Vec<ApiSecretKey>,
    pub granted_scopes: Vec<String>,
    pub application_url: Option<String>,
    pub redirect_url_whitelist: Vec<String>,
    pub flags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiSecretKey {
    pub secret: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrganizationStore {
    pub shop_id: String,
    pub shop_domain: String,
    pub shop_name: String,
    pub transfer_disabled: bool,
    pub convertable_to_partner_test: bool,
    pub provisionable: bool,
    pub link: Option<String>,
    pub store_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Paginateable<T> {
    #[serde(flatten)]
    pub data: T,
    pub has_more_pages: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateAppOptions {
    pub name: String,
    pub is_launchable: Option<bool>,
    pub scopes_array: Option<Vec<String>>,
    pub directory: Option<String>,
    pub is_embedded: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppVersionIdentifiers {
    pub app_version_id: i64,
    pub version_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppModuleVersion {
    pub registration_id: String,
    pub registration_uuid: Option<String>,
    pub registration_title: String,
    pub config: Option<serde_json::Value>,
    pub target: Option<String>,
    #[serde(rename = "type")]
    pub module_type: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppVersion {
    pub app_module_versions: Vec<AppModuleVersion>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppVersionWithContext {
    pub id: i64,
    pub uuid: String,
    pub version_tag: Option<String>,
    pub app_module_versions: Vec<AppModuleVersion>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteSpecification {
    pub identifier: String,
    pub name: String,
    pub experience: String,
    pub options: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionTemplate {
    pub identifier: String,
    pub name: String,
    pub group: Option<String>,
    pub url: Option<String>,
    pub types: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionTemplatesResult {
    pub templates: Vec<ExtensionTemplate>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetUrlSchema {
    pub asset_url: Option<String>,
    pub user_errors: Vec<UserError>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserError {
    pub field: Option<Vec<String>>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountInfo {
    pub email: Option<String>,
    pub id: Option<String>,
}

/// Filter disabled feature flags the same way upstream does.
pub fn filter_disabled_flags(disabled_flags: &[String]) -> Vec<String> {
    disabled_flags
        .iter()
        .filter(|f| !f.is_empty())
        .cloned()
        .collect()
}
