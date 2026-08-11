//! UI extension payload models (mirrors upstream `payload/models.ts`).

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Asset {
    pub name: String,
    pub url: String,
    #[serde(rename = "lastUpdated")]
    pub last_updated: u64,
}

impl Asset {
    pub fn new(name: impl Into<String>, url: impl Into<String>, last_updated: u64) -> Self {
        Self {
            name: name.into(),
            url: url.into(),
            last_updated,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UrlHolder {
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OptionalUrlHolder {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DevelopmentError {
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct DevelopmentPayload {
    #[serde(default)]
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hidden: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<DevelopmentError>,
    #[serde(rename = "localizationStatus", skip_serializing_if = "Option::is_none")]
    pub localization_status: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DevelopmentState {
    pub resource: OptionalUrlHolder,
    pub root: UrlHolder,
    pub hidden: bool,
    pub status: String,
    #[serde(rename = "localizationStatus")]
    pub localization_status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<DevelopmentError>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DevNewExtensionPoint {
    pub target: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surface: Option<String>,
    pub root: UrlHolder,
    pub resource: UrlHolder,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assets: Option<std::collections::HashMap<String, Asset>>,
    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppPayload {
    #[serde(rename = "apiKey")]
    pub api_key: String,
    pub url: String,
    #[serde(rename = "mobileUrl")]
    pub mobile_url: String,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UIExtensionPayload {
    pub assets: MainAssets,
    #[serde(rename = "supportedFeatures", skip_serializing_if = "Option::is_none")]
    pub supported_features: Option<SupportedFeatures>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<Value>,
    pub development: DevelopmentState,
    #[serde(rename = "extensionPoints")]
    pub extension_points: Value,
    pub localization: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metafields: Option<Value>,
    #[serde(rename = "type")]
    pub type_name: String,
    #[serde(rename = "externalType")]
    pub external_type: String,
    #[serde(rename = "apiVersion", skip_serializing_if = "Option::is_none")]
    pub api_version: Option<String>,
    pub uuid: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    pub surface: String,
    pub title: String,
    pub handle: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(rename = "approvalScopes")]
    pub approval_scopes: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settings: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MainAssets {
    pub main: Asset,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SupportedFeatures {
    #[serde(rename = "runsOffline")]
    pub runs_offline: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtensionsEndpointPayload {
    pub app: AppPayload,
    #[serde(rename = "appId", skip_serializing_if = "Option::is_none")]
    pub app_id: Option<String>,
    pub store: String,
    pub extensions: Vec<UIExtensionPayload>,
    pub version: String,
    pub root: UrlHolder,
    #[serde(rename = "devConsole")]
    pub dev_console: UrlHolder,
    pub socket: UrlHolder,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConnectedPayload {
    pub app: AppPayload,
    #[serde(rename = "appId", skip_serializing_if = "Option::is_none")]
    pub app_id: Option<String>,
    pub store: String,
    pub extensions: Vec<UIExtensionPayload>,
}
