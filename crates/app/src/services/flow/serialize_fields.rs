//! Local TOML fields → Partners serialized fields (upstream `serialize-fields.ts`).

use super::constants::{
    action_ui_type, is_supported_commerce_object, trigger_ui_type,
    ACTION_SUPPORTED_COMMERCE_OBJECTS, TRIGGER_SUPPORTED_COMMERCE_OBJECTS,
};
use super::types::{ConfigField, FlowExtensionType, SerializedField};
use super::validation::is_schema_type_reference;
use crate::error::AppError;

fn pascalize(s: &str) -> String {
    s.split('_')
        .filter(|p| !p.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect()
}

pub fn serialize_config_field(
    field: &ConfigField,
    ext_type: FlowExtensionType,
) -> Result<SerializedField, AppError> {
    let type_is_schema_ref = is_schema_type_reference(&field.r#type);
    let ui_type = if type_is_schema_ref {
        match ext_type {
            FlowExtensionType::FlowAction => action_ui_type("schema_type_reference"),
            FlowExtensionType::FlowTrigger => trigger_ui_type("schema_type_reference"),
        }
    } else {
        match ext_type {
            FlowExtensionType::FlowAction => action_ui_type(&field.r#type),
            FlowExtensionType::FlowTrigger => trigger_ui_type(&field.r#type),
        }
    };

    let Some(key) = field.key.as_deref() else {
        let json = serde_json::to_string(field).unwrap_or_else(|_| "{}".into());
        return Err(AppError::message(format!(
            "key property must be specified for non-commerce object fields in {json}"
        )));
    };

    let Some(ui_type) = ui_type else {
        let kind = if ext_type == FlowExtensionType::FlowAction {
            "Actions"
        } else {
            "Triggers"
        };
        return Err(AppError::message(format!(
            "Field type {} is not supported on Flow {kind}",
            field.r#type
        )));
    };

    let mut serialized = SerializedField {
        name: key.to_string(),
        description: field.description.clone(),
        ui_type: ui_type.to_string(),
        label: None,
        required: None,
        type_ref_name: None,
        marketing_activity_create_url: None,
        marketing_activity_delete_url: None,
    };

    if ext_type == FlowExtensionType::FlowAction {
        serialized.label = field.name.clone();
        serialized.required = field.required;
    }

    if type_is_schema_ref {
        serialized.type_ref_name = Some(field.r#type.replacen("schema.", "", 1));
    }

    Ok(serialized)
}

pub fn serialize_commerce_object_field(
    field: &ConfigField,
    ext_type: FlowExtensionType,
) -> Result<SerializedField, AppError> {
    match ext_type {
        FlowExtensionType::FlowTrigger
            if !TRIGGER_SUPPORTED_COMMERCE_OBJECTS.contains(&field.r#type.as_str()) =>
        {
            return Err(AppError::message(format!(
                "Commerce object {} is not supported for Flow Triggers",
                field.r#type
            )));
        }
        FlowExtensionType::FlowAction
            if !ACTION_SUPPORTED_COMMERCE_OBJECTS.contains(&field.r#type.as_str()) =>
        {
            return Err(AppError::message(format!(
                "Commerce object {} is not supported for Flow Actions",
                field.r#type
            )));
        }
        _ => {}
    }

    let commerce_object = field.r#type.replace("_reference", "");
    let mut ui_type = if ext_type == FlowExtensionType::FlowAction {
        "commerce-object-id".to_string()
    } else {
        commerce_object.clone()
    };

    let mut serialized = SerializedField {
        name: format!("{commerce_object}_id"),
        ui_type: ui_type.clone(),
        description: field.description.clone(),
        label: None,
        required: None,
        type_ref_name: None,
        marketing_activity_create_url: None,
        marketing_activity_delete_url: None,
    };

    if commerce_object == "marketing_activity" {
        ui_type = "marketing-activity-id".into();
        serialized.ui_type = ui_type;
        serialized.marketing_activity_create_url = field.marketing_activity_create_url.clone();
        serialized.marketing_activity_delete_url = field.marketing_activity_delete_url.clone();
    }

    if ext_type == FlowExtensionType::FlowAction {
        serialized.label = Some(format!("{} ID", pascalize(&commerce_object)));
        serialized.required = field.required;
    }

    Ok(serialized)
}

pub fn serialize_fields(
    ext_type: FlowExtensionType,
    fields: Option<&[ConfigField]>,
) -> Result<Vec<SerializedField>, AppError> {
    let Some(fields) = fields else {
        return Ok(vec![]);
    };
    fields
        .iter()
        .map(|field| {
            if is_supported_commerce_object(&field.r#type) {
                serialize_commerce_object_field(field, ext_type)
            } else {
                serialize_config_field(field, ext_type)
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_config_field_for_flow_action() {
        let field = ConfigField {
            r#type: "multi_line_text_field".into(),
            key: Some("my-field".into()),
            name: Some("My Field".into()),
            description: Some("This is my field".into()),
            required: Some(true),
            marketing_activity_create_url: None,
            marketing_activity_delete_url: None,
        };
        let got = serialize_config_field(&field, FlowExtensionType::FlowAction).unwrap();
        assert_eq!(got.name, "my-field");
        assert_eq!(got.ui_type, "text-multi-line");
        assert_eq!(got.label.as_deref(), Some("My Field"));
        assert_eq!(got.required, Some(true));
    }

    #[test]
    fn serializes_config_field_for_flow_trigger() {
        let field = ConfigField {
            r#type: "single_line_text_field".into(),
            key: Some("my-field".into()),
            name: Some("My Field".into()),
            description: Some("This is my field".into()),
            required: Some(true),
            marketing_activity_create_url: None,
            marketing_activity_delete_url: None,
        };
        let got = serialize_config_field(&field, FlowExtensionType::FlowTrigger).unwrap();
        assert_eq!(got.name, "my-field");
        assert_eq!(got.ui_type, "text-single-line");
        assert!(got.label.is_none());
        assert!(got.required.is_none());
    }

    #[test]
    fn rejects_missing_key() {
        let field = ConfigField {
            r#type: "string".into(),
            key: None,
            name: Some("My Field".into()),
            description: Some("This is my field".into()),
            required: Some(true),
            marketing_activity_create_url: None,
            marketing_activity_delete_url: None,
        };
        let err = serialize_config_field(&field, FlowExtensionType::FlowAction).unwrap_err();
        assert!(err.to_string().contains("key property must be specified"));
    }

    #[test]
    fn rejects_unsupported_trigger_type() {
        let field = ConfigField {
            r#type: "multi_line_text_field".into(),
            key: Some("my-field".into()),
            name: Some("My Field".into()),
            description: Some("This is my field".into()),
            required: Some(true),
            marketing_activity_create_url: None,
            marketing_activity_delete_url: None,
        };
        let err = serialize_config_field(&field, FlowExtensionType::FlowTrigger).unwrap_err();
        assert!(err
            .to_string()
            .contains("not supported on Flow Triggers"));
    }

    #[test]
    fn serializes_commerce_object_for_action() {
        let field = ConfigField {
            r#type: "product_reference".into(),
            key: Some("my-field".into()),
            name: Some("My Field".into()),
            description: Some("This is my field".into()),
            required: Some(true),
            marketing_activity_create_url: None,
            marketing_activity_delete_url: None,
        };
        let got = serialize_commerce_object_field(&field, FlowExtensionType::FlowAction).unwrap();
        assert_eq!(got.name, "product_id");
        assert_eq!(got.ui_type, "commerce-object-id");
        assert_eq!(got.label.as_deref(), Some("Product ID"));
    }

    #[test]
    fn serializes_company_contact_for_action() {
        let field = ConfigField {
            r#type: "company_contact_reference".into(),
            key: None,
            name: None,
            description: Some("This is my field".into()),
            required: Some(true),
            marketing_activity_create_url: None,
            marketing_activity_delete_url: None,
        };
        let got = serialize_commerce_object_field(&field, FlowExtensionType::FlowAction).unwrap();
        assert_eq!(got.name, "company_contact_id");
        assert_eq!(got.label.as_deref(), Some("CompanyContact ID"));
    }

    #[test]
    fn serializes_commerce_object_for_trigger() {
        let field = ConfigField {
            r#type: "product_reference".into(),
            key: None,
            name: None,
            description: Some("This is my field".into()),
            required: None,
            marketing_activity_create_url: None,
            marketing_activity_delete_url: None,
        };
        let got = serialize_commerce_object_field(&field, FlowExtensionType::FlowTrigger).unwrap();
        assert_eq!(got.name, "product_id");
        assert_eq!(got.ui_type, "product");
    }

    #[test]
    fn rejects_unsupported_commerce_objects() {
        let field = ConfigField {
            r#type: "invalid_reference".into(),
            key: None,
            name: None,
            description: None,
            required: None,
            marketing_activity_create_url: None,
            marketing_activity_delete_url: None,
        };
        assert!(serialize_commerce_object_field(&field, FlowExtensionType::FlowTrigger).is_err());
        assert!(serialize_commerce_object_field(&field, FlowExtensionType::FlowAction).is_err());
    }
}
