//! Partners serialized fields → local TOML fields (upstream `serialize-partners-fields.ts`).

use super::constants::{field_type_from_ui_type, PARTNERS_COMMERCE_OBJECTS};
use super::types::{ConfigField, FlowPartnersExtensionType, SerializedField};
use crate::error::AppError;

fn serialize_config_field(
    field: &SerializedField,
    ext_type: FlowPartnersExtensionType,
) -> Result<ConfigField, AppError> {
    let Some(field_type) = field_type_from_ui_type(&field.ui_type) else {
        return Err(AppError::message(format!(
            "Field type {} is not supported",
            field.ui_type
        )));
    };

    let mut serialized = ConfigField {
        key: Some(field.name.clone()),
        description: field.description.clone(),
        r#type: field_type.to_string(),
        name: None,
        required: None,
        marketing_activity_create_url: None,
        marketing_activity_delete_url: None,
    };

    if ext_type == FlowPartnersExtensionType::FlowActionDefinition {
        serialized.name = field.label.clone();
        serialized.required = field.required;
    }

    Ok(serialized)
}

fn serialize_commerce_object_field(
    field: &SerializedField,
    ext_type: FlowPartnersExtensionType,
) -> ConfigField {
    let is_action = ext_type == FlowPartnersExtensionType::FlowActionDefinition;
    let field_type = if is_action {
        format!("{}_reference", field.name.replace("_id", ""))
    } else {
        format!("{}_reference", field.ui_type)
    };

    let mut serialized = ConfigField {
        r#type: field_type,
        key: None,
        name: None,
        description: None,
        required: None,
        marketing_activity_create_url: None,
        marketing_activity_delete_url: None,
    };

    if is_action {
        serialized.required = field.required;
    }

    if field.ui_type == "marketing-activity-id" {
        serialized.marketing_activity_create_url = field.marketing_activity_create_url.clone();
        serialized.marketing_activity_delete_url = field.marketing_activity_delete_url.clone();
    }

    serialized
}

pub fn config_from_serialized_fields(
    ext_type: FlowPartnersExtensionType,
    fields: Option<&[SerializedField]>,
) -> Result<Vec<ConfigField>, AppError> {
    let Some(fields) = fields else {
        return Ok(vec![]);
    };

    fields
        .iter()
        .map(|field| {
            if field.ui_type == "commerce-object-id"
                || field.ui_type == "marketing-activity-id"
                || PARTNERS_COMMERCE_OBJECTS.contains(&field.ui_type.as_str())
            {
                Ok(serialize_commerce_object_field(field, ext_type))
            } else {
                serialize_config_field(field, ext_type)
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::flow::serialize_fields::serialize_fields;
    use crate::services::flow::types::FlowExtensionType;

    #[test]
    fn roundtrip_flow_action_fields() {
        let fields = vec![
            SerializedField {
                name: "customer_id".into(),
                label: Some("Customer ID".into()),
                required: Some(true),
                ui_type: "commerce-object-id".into(),
                description: None,
                type_ref_name: None,
                marketing_activity_create_url: None,
                marketing_activity_delete_url: None,
            },
            SerializedField {
                name: "product_id".into(),
                label: Some("Product ID".into()),
                required: Some(true),
                ui_type: "commerce-object-id".into(),
                description: None,
                type_ref_name: None,
                marketing_activity_create_url: None,
                marketing_activity_delete_url: None,
            },
            SerializedField {
                name: "marketing_activity_id".into(),
                label: Some("MarketingActivity ID".into()),
                required: Some(false),
                ui_type: "marketing-activity-id".into(),
                description: None,
                type_ref_name: None,
                marketing_activity_create_url: None,
                marketing_activity_delete_url: None,
            },
            SerializedField {
                name: "email field".into(),
                label: Some("email label".into()),
                description: Some("email help".into()),
                required: Some(false),
                ui_type: "email".into(),
                type_ref_name: None,
                marketing_activity_create_url: None,
                marketing_activity_delete_url: None,
            },
            SerializedField {
                name: "number name".into(),
                label: Some("number label".into()),
                description: Some("number help".into()),
                required: Some(true),
                ui_type: "number".into(),
                type_ref_name: None,
                marketing_activity_create_url: None,
                marketing_activity_delete_url: None,
            },
        ];

        let config = config_from_serialized_fields(
            FlowPartnersExtensionType::FlowActionDefinition,
            Some(&fields),
        )
        .unwrap();
        let reserialized = serialize_fields(FlowExtensionType::FlowAction, Some(&config)).unwrap();
        assert_eq!(reserialized, fields);
    }

    #[test]
    fn roundtrip_flow_trigger_fields() {
        let fields = vec![
            SerializedField {
                name: "customer_id".into(),
                ui_type: "customer".into(),
                label: None,
                description: None,
                required: None,
                type_ref_name: None,
                marketing_activity_create_url: None,
                marketing_activity_delete_url: None,
            },
            SerializedField {
                description: Some("number description".into()),
                name: "number property".into(),
                ui_type: "number".into(),
                label: None,
                required: None,
                type_ref_name: None,
                marketing_activity_create_url: None,
                marketing_activity_delete_url: None,
            },
            SerializedField {
                description: Some("email description".into()),
                name: "email name".into(),
                ui_type: "email".into(),
                label: None,
                required: None,
                type_ref_name: None,
                marketing_activity_create_url: None,
                marketing_activity_delete_url: None,
            },
        ];

        let config = config_from_serialized_fields(
            FlowPartnersExtensionType::FlowTriggerDefinition,
            Some(&fields),
        )
        .unwrap();
        let reserialized = serialize_fields(FlowExtensionType::FlowTrigger, Some(&config)).unwrap();
        assert_eq!(reserialized, fields);
    }
}
