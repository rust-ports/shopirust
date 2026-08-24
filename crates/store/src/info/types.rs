use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct StoreInfoStoreOwner {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct OwningOrgInternal {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct StoreInfoResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    pub subdomain: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub organization_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub organization_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub store_owner: Option<StoreInfoStoreOwner>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub store_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feature_preview: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub admin_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub save_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DestinationsContext {
    pub owning_org: Option<OwningOrgInternal>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OrganizationShopFields {
    pub shopify_shop_id: Option<String>,
    pub name: Option<String>,
    pub primary_domain: Option<String>,
    pub store_type: Option<String>,
    pub developer_preview_handle: Option<String>,
    pub plan_name: Option<String>,
    pub owner_name: Option<String>,
    pub owner_email: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AdminShopInfo {
    pub id: Option<String>,
    pub name: Option<String>,
    pub myshopify_domain: Option<String>,
    pub email: Option<String>,
    pub shop_owner_name: Option<String>,
    pub plan_public_display_name: Option<String>,
    pub partner_development: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PreviewStoreUrls {
    pub access_url: String,
    pub save_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DestinationNode {
    pub public_id: String,
    pub primary_domain: Option<String>,
    pub web_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OwningOrgRaw {
    pub id: Option<String>,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OrganizationShopNode {
    pub shopify_shop_id: Option<String>,
    pub name: Option<String>,
    pub primary_domain: Option<String>,
    pub store_type: Option<String>,
    pub developer_preview_handle: Option<String>,
    pub plan_name: Option<String>,
    pub owner_name: Option<String>,
    pub owner_email: Option<String>,
}
