use serde::{Deserialize, Serialize};

pub const STORE_LIST_LIMIT: usize = 250;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StoreListEntry {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub store: String,
    pub created_at: String,
    pub organization_id: String,
    pub organization_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub store_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StoreListOrganization {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ListStoresResult {
    pub stores: Vec<StoreListEntry>,
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub organization: Option<StoreListOrganization>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notice: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreListOrg {
    pub id: String,
    pub business_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrganizationsAccessInfo {
    pub organizations: Vec<StoreListOrg>,
    pub current_user_resolved: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ListStoresOptions {
    pub organization_id: Option<String>,
}
