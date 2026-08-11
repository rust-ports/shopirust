//! Flow extension field and URL types (upstream `services/flow/types.ts`).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConfigField {
    #[serde(default)]
    pub r#type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(
        default,
        rename = "marketingActivityCreateUrl",
        skip_serializing_if = "Option::is_none"
    )]
    pub marketing_activity_create_url: Option<String>,
    #[serde(
        default,
        rename = "marketingActivityDeleteUrl",
        skip_serializing_if = "Option::is_none"
    )]
    pub marketing_activity_delete_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SerializedField {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required: Option<bool>,
    pub ui_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub type_ref_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub marketing_activity_create_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub marketing_activity_delete_url: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowExtensionType {
    FlowAction,
    FlowTrigger,
}

impl FlowExtensionType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FlowAction => "flow_action",
            Self::FlowTrigger => "flow_trigger",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "flow_action" => Some(Self::FlowAction),
            "flow_trigger" => Some(Self::FlowTrigger),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowPartnersExtensionType {
    FlowActionDefinition,
    FlowTriggerDefinition,
}

impl FlowPartnersExtensionType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FlowActionDefinition => "flow_action_definition",
            Self::FlowTriggerDefinition => "flow_trigger_definition",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "flow_action_definition" => Some(Self::FlowActionDefinition),
            "flow_trigger_definition" => Some(Self::FlowTriggerDefinition),
            _ => None,
        }
    }
}

pub const FLOW_ACTION_URL_FIELDS: &[&str] = &[
    "runtime_url",
    "validation_url",
    "config_page_url",
    "config_page_preview_url",
];
