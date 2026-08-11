//! Import dashboard Flow registrations → local TOML config (upstream `extension-config-builder.ts`).

use super::serialize_partners_fields::config_from_serialized_fields;
use super::types::{FlowPartnersExtensionType, SerializedField};
use crate::error::AppError;
use crate::models::extensions::schemas::MAX_EXTENSION_HANDLE_LENGTH;
use crate::services::generate::slugify;
use crate::services::import_extensions::ExtensionRegistration;
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Debug, Deserialize)]
struct FlowConfig {
    title: String,
    description: Option<String>,
    url: Option<String>,
    #[serde(default)]
    fields: Vec<PartnersSerializedField>,
    custom_configuration_page_url: Option<String>,
    custom_configuration_page_preview_url: Option<String>,
    validation_url: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PartnersSerializedField {
    name: String,
    label: Option<String>,
    description: Option<String>,
    required: Option<bool>,
    ui_type: String,
    marketing_activity_create_url: Option<String>,
    marketing_activity_delete_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FlowWebhookConfig {
    url: String,
}

fn truncated_handle(title: &str) -> String {
    let truncated: String = title.chars().take(MAX_EXTENSION_HANDLE_LENGTH).collect();
    slugify(&truncated)
}

fn version_config(ext: &ExtensionRegistration) -> Result<&str, AppError> {
    ext.active_version
        .as_ref()
        .or(ext.draft_version.as_ref())
        .and_then(|v| v.config.as_deref())
        .ok_or_else(|| AppError::message("No config found for extension"))
}

/// Convert a Partners Flow registration into the local unified TOML representation.
pub fn build_extension_config(extension: &ExtensionRegistration) -> Result<Value, AppError> {
    let version_config = version_config(extension)?;

    if extension.type_name == "flow_trigger_discovery_webhook" {
        let config: FlowWebhookConfig = serde_json::from_str(version_config)?;
        return Ok(json!({
            "extensions": [{
                "type": "flow_trigger_lifecycle_callback",
                "name": extension.title,
                "handle": truncated_handle(&extension.title),
                "url": config.url,
            }]
        }));
    }

    let config: FlowConfig = serde_json::from_str(version_config)?;
    let partners_type = FlowPartnersExtensionType::parse(&extension.type_name).ok_or_else(|| {
        AppError::message(format!(
            "Unsupported flow extension type: {}",
            extension.type_name
        ))
    })?;

    let fields: Vec<SerializedField> = config
        .fields
        .into_iter()
        .map(|f| SerializedField {
            name: f.name,
            label: f.label,
            description: f.description,
            required: f.required,
            ui_type: f.ui_type,
            type_ref_name: None,
            marketing_activity_create_url: f.marketing_activity_create_url,
            marketing_activity_delete_url: f.marketing_activity_delete_url,
        })
        .collect();

    let local_fields = config_from_serialized_fields(partners_type, Some(&fields))?;
    let default_url = if partners_type == FlowPartnersExtensionType::FlowActionDefinition {
        Some("https://url.com/api/execute")
    } else {
        None
    };

    let local_type = extension.type_name.replace("_definition", "");
    let mut extension_obj = json!({
        "type": local_type,
        "name": config.title,
        "handle": truncated_handle(&extension.title),
        "description": config.description,
        "runtime_url": config.url.or_else(|| default_url.map(|s| s.to_string())),
        "config_page_url": config.custom_configuration_page_url,
        "config_page_preview_url": config.custom_configuration_page_preview_url,
        "validation_url": config.validation_url,
    });

    // Drop null runtime_url for triggers when unset (keep key absence cleaner for TOML).
    if partners_type == FlowPartnersExtensionType::FlowTriggerDefinition {
        if let Some(obj) = extension_obj.as_object_mut() {
            if obj.get("runtime_url").map(|v| v.is_null()).unwrap_or(false) {
                obj.remove("runtime_url");
            }
            if obj
                .get("config_page_url")
                .map(|v| v.is_null())
                .unwrap_or(false)
            {
                obj.remove("config_page_url");
            }
            if obj
                .get("config_page_preview_url")
                .map(|v| v.is_null())
                .unwrap_or(false)
            {
                obj.remove("config_page_preview_url");
            }
            if obj
                .get("validation_url")
                .map(|v| v.is_null())
                .unwrap_or(false)
            {
                obj.remove("validation_url");
            }
        }
    }

    let mut out = json!({ "extensions": [extension_obj] });
    if !local_fields.is_empty() {
        out.as_object_mut()
            .unwrap()
            .insert("settings".into(), json!({ "fields": local_fields }));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::import_extensions::ExtensionVersion;

    #[test]
    fn builds_flow_action_config() {
        let extension = ExtensionRegistration {
            uuid: "ad9947a9-bc0b-4855-82da-008aefbc1c71".into(),
            title: "flow action @ Char!".into(),
            type_name: "flow_action_definition".into(),
            draft_version: Some(ExtensionVersion {
                config: Some(
                    r#"{"title":"action title","description":"action description","url":"https://google.es","fields":[{"name":"customer_id","label":"Customer ID","description":"","required":true,"id":"bc16767a-02ab-4775-93e0-04bfe91a94e2","uiType":"commerce-object-id"},{"name":"product_id","label":"Product ID","description":"","required":true,"id":"8dc7911e-f15d-46ee-8aae-82eac7630378","uiType":"commerce-object-id"},{"name":"email field","label":"email label","description":"email help","required":false,"id":"b174c2aa-6cee-4e13-82f8-b60033e84835","uiType":"email"},{"name":"number name","label":"number label","description":"number help","required":true,"id":"363619e5-7b34-4fff-8bd6-c3af054be321","uiType":"number"}],"custom_configuration_page_url":"https://destinationsurl.test.dev","custom_configuration_page_preview_url":"https://previewurl.test.dev","validation_url":"https://validation.test.dev"}"#
                        .into(),
                ),
                context: None,
            }),
            active_version: None,
        };

        let got = build_extension_config(&extension).unwrap();
        assert_eq!(
            got.pointer("/extensions/0/type").and_then(|v| v.as_str()),
            Some("flow_action")
        );
        assert_eq!(
            got.pointer("/extensions/0/handle").and_then(|v| v.as_str()),
            Some("flow-action-char")
        );
        assert_eq!(
            got.pointer("/extensions/0/runtime_url")
                .and_then(|v| v.as_str()),
            Some("https://google.es")
        );
        let fields = got.pointer("/settings/fields").unwrap().as_array().unwrap();
        assert_eq!(fields.len(), 4);
        assert_eq!(fields[0]["type"], "customer_reference");
        assert_eq!(fields[2]["type"], "email");
        assert_eq!(fields[3]["type"], "number_decimal");
    }

    #[test]
    fn truncates_long_handles() {
        let extension = ExtensionRegistration {
            uuid: "u".into(),
            title: "flow action @ Char! flow action @ Char! flow action @ Char! flow action @ Char!"
                .into(),
            type_name: "flow_action_definition".into(),
            draft_version: Some(ExtensionVersion {
                config: Some(
                    r#"{"title":"action title","description":"action description","url":"https://google.es","fields":[]}"#
                        .into(),
                ),
                context: None,
            }),
            active_version: None,
        };
        let got = build_extension_config(&extension).unwrap();
        assert_eq!(
            got.pointer("/extensions/0/handle").and_then(|v| v.as_str()),
            Some("flow-action-char-flow-action-char-flow-actio")
        );
    }

    #[test]
    fn builds_flow_trigger_config() {
        let extension = ExtensionRegistration {
            uuid: "e1cb40b1-2af2-4292-91a9-0824e0157bb2".into(),
            title: "trigger ext!\"*^ÑÇ¨:\"!".into(),
            type_name: "flow_trigger_definition".into(),
            active_version: Some(ExtensionVersion {
                config: Some(
                    r#"{"title":"trigger title","description":"trigger description","feature_version":2,"fields":[{"description":"","name":"customer_id","id":"2ed1d556-be40-488b-b4a1-3456a79d2963","uiType":"customer"},{"description":"number description","name":"number property","id":"1b76c360-f0c3-4a05-a845-d910e3546a43","uiType":"number"},{"description":"email description","name":"email name","id":"75c1a5f3-f9d1-46f8-8383-b22798fe8f89","uiType":"email"}]}"#
                        .into(),
                ),
                context: None,
            }),
            draft_version: None,
        };
        let got = build_extension_config(&extension).unwrap();
        assert_eq!(
            got.pointer("/extensions/0/type").and_then(|v| v.as_str()),
            Some("flow_trigger")
        );
        assert_eq!(
            got.pointer("/extensions/0/handle").and_then(|v| v.as_str()),
            Some("trigger-ext")
        );
        let fields = got.pointer("/settings/fields").unwrap().as_array().unwrap();
        assert_eq!(fields[0]["type"], "customer_reference");
        assert_eq!(fields[1]["type"], "number_decimal");
    }
}
