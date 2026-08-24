//! Flow validation helpers (upstream `services/flow/validation.ts`).

use super::constants::is_supported_commerce_object;
use super::types::{ConfigField, FlowExtensionType};
use crate::error::AppError;

pub fn is_schema_type_reference(ty: &str) -> bool {
    ty.starts_with("schema.")
}

fn field_validation_error(
    property: &str,
    config_field: &ConfigField,
    handle: &str,
    index: usize,
) -> String {
    let json = serde_json::to_string(config_field).unwrap_or_else(|_| "{}".into());
    format!(
        "'{property}' property must be a string for 'field[{index}]' {json} of flow extension '{handle}'"
    )
}

/// Validate a single settings field shape for flow_action / flow_trigger.
pub fn validate_field_shape(
    config_field: &ConfigField,
    ext_type: FlowExtensionType,
    extension_handle: &str,
    index: usize,
) -> Result<ConfigField, AppError> {
    let is_commerce = is_supported_commerce_object(&config_field.r#type);

    if !is_commerce {
        if config_field.r#type.is_empty() {
            return Err(AppError::message("type property must be a string"));
        }
        if ext_type == FlowExtensionType::FlowAction {
            if config_field.key.is_none() {
                return Err(AppError::message(field_validation_error(
                    "key",
                    config_field,
                    extension_handle,
                    index,
                )));
            }
            if config_field.name.is_none() {
                return Err(AppError::message(field_validation_error(
                    "name",
                    config_field,
                    extension_handle,
                    index,
                )));
            }
            return Ok(config_field.clone());
        }

        if let Some(key) = &config_field.key {
            if !key.chars().all(|c| c.is_ascii_alphabetic() || c == ' ') {
                return Err(AppError::message(
                    "String must contain only alphabetic characters and spaces",
                ));
            }
        }
        return Ok(config_field.clone());
    }

    if config_field.key.is_some() {
        return Err(AppError::message("Unrecognized key(s) in object: 'key'"));
    }
    if config_field.name.is_some() {
        return Err(AppError::message("Unrecognized key(s) in object: 'name'"));
    }
    Ok(config_field.clone())
}

fn contains_url_control_character(value: &str) -> bool {
    value.chars().any(|c| matches!(c, '\r' | '\n' | '\t'))
}

/// Absolute HTTPS or single-slash relative URL (no protocol-relative, no control chars).
pub fn validate_flow_action_url(value: &str) -> Result<(), AppError> {
    const MSG: &str = "Invalid URL: URL must be an absolute HTTPS URL or a relative URL starting with a single slash (e.g. \"/api/endpoint\").";
    if contains_url_control_character(value) {
        return Err(AppError::message(
            "Invalid URL: URL must not contain control characters such as newlines or tabs.",
        ));
    }
    if value.starts_with("//") {
        return Err(AppError::message(
            "Invalid URL: Relative URLs must start with a single slash.",
        ));
    }
    if value.starts_with('/') {
        return Ok(());
    }
    match url::Url::parse(value) {
        Ok(u) if u.scheme().eq_ignore_ascii_case("https") => Ok(()),
        _ => Err(AppError::message(MSG)),
    }
}

pub fn validate_custom_configuration_page_config(
    config_page_url: Option<&str>,
    config_page_preview_url: Option<&str>,
    validation_url: Option<&str>,
) -> Result<(), AppError> {
    if config_page_url.is_none() && config_page_preview_url.is_none() {
        return Ok(());
    }
    if config_page_url.is_none() {
        return Err(AppError::message(
            "To set a custom configuration page a `config_page_url` must be specified.",
        ));
    }
    if config_page_preview_url.is_none() {
        return Err(AppError::message(
            "To set a custom configuration page a `config_page_preview_url` must be specified.",
        ));
    }
    if validation_url.is_none() {
        return Err(AppError::message(
            "To set a custom configuration page a `validation_url` must be specified.",
        ));
    }
    Ok(())
}

pub fn validate_trigger_schema_presence(
    fields: &[ConfigField],
    schema: Option<&str>,
) -> Result<(), AppError> {
    if fields.iter().any(|f| is_schema_type_reference(&f.r#type)) && schema.is_none() {
        return Err(AppError::message(
            "To reference schema types a `schema` must be specified.",
        ));
    }
    Ok(())
}

pub fn validate_return_type_config(
    return_type_ref: Option<&str>,
    schema: Option<&str>,
) -> Result<(), AppError> {
    if return_type_ref.is_none() && schema.is_none() {
        return Ok(());
    }
    if return_type_ref.is_none() {
        return Err(AppError::message(
            "When uploading a schema a `return_type_ref` must be specified.",
        ));
    }
    if schema.is_none() {
        return Err(AppError::message(
            "To set a return type a `schema` must be specified.",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_flow_action_urls() {
        assert!(validate_flow_action_url("https://example.com/api/execute").is_ok());
        assert!(validate_flow_action_url("/api/execute").is_ok());
        assert!(validate_flow_action_url("http://example.com/api/execute").is_err());
        assert!(validate_flow_action_url("//example.com/api/execute").is_err());
        assert!(validate_flow_action_url("/api/execute\nmalicious").is_err());
        assert!(validate_flow_action_url("https://example.com/api\texecute").is_err());
    }

    #[test]
    fn validates_field_shapes() {
        let ok = ConfigField {
            r#type: "multi_line_text_field".into(),
            key: Some("my-field".into()),
            name: Some("My Field".into()),
            description: Some("This is my field".into()),
            required: Some(true),
            marketing_activity_create_url: None,
            marketing_activity_delete_url: None,
        };
        assert!(validate_field_shape(&ok, FlowExtensionType::FlowAction, "handle", 0).is_ok());

        let trigger = ConfigField {
            r#type: "multi_line_text_field".into(),
            key: Some("my field".into()),
            name: Some("My Field".into()),
            description: Some("This is my field".into()),
            required: Some(true),
            marketing_activity_create_url: None,
            marketing_activity_delete_url: None,
        };
        assert!(
            validate_field_shape(&trigger, FlowExtensionType::FlowTrigger, "handle", 0).is_ok()
        );

        let commerce = ConfigField {
            r#type: "product_reference".into(),
            key: None,
            name: None,
            description: Some("This is my field".into()),
            required: Some(true),
            marketing_activity_create_url: None,
            marketing_activity_delete_url: None,
        };
        assert!(
            validate_field_shape(&commerce, FlowExtensionType::FlowAction, "handle", 0).is_ok()
        );

        let missing_key = ConfigField {
            r#type: "string".into(),
            key: None,
            name: Some("My Field".into()),
            description: None,
            required: None,
            marketing_activity_create_url: None,
            marketing_activity_delete_url: None,
        };
        assert!(
            validate_field_shape(&missing_key, FlowExtensionType::FlowAction, "handle", 0).is_err()
        );

        let bad_trigger_key = ConfigField {
            r#type: "string".into(),
            key: Some("my-field".into()),
            name: None,
            description: None,
            required: None,
            marketing_activity_create_url: None,
            marketing_activity_delete_url: None,
        };
        assert!(validate_field_shape(
            &bad_trigger_key,
            FlowExtensionType::FlowTrigger,
            "handle",
            0
        )
        .is_err());

        let commerce_with_key = ConfigField {
            r#type: "customer_reference".into(),
            key: Some("foo".into()),
            name: None,
            description: Some("x".into()),
            required: None,
            marketing_activity_create_url: None,
            marketing_activity_delete_url: None,
        };
        assert!(validate_field_shape(
            &commerce_with_key,
            FlowExtensionType::FlowAction,
            "handle",
            0
        )
        .is_err());
    }

    #[test]
    fn validates_custom_configuration_page() {
        assert!(validate_custom_configuration_page_config(None, None, None).is_ok());
        assert!(validate_custom_configuration_page_config(None, Some("p"), Some("v")).is_err());
        assert!(validate_custom_configuration_page_config(Some("c"), None, Some("v")).is_err());
        assert!(validate_custom_configuration_page_config(Some("c"), Some("p"), None).is_err());
        assert!(validate_custom_configuration_page_config(Some("c"), Some("p"), Some("v")).is_ok());
    }

    #[test]
    fn validates_schema_presence_and_return_type() {
        let fields = vec![ConfigField {
            r#type: "schema.Foo".into(),
            key: Some("k".into()),
            name: Some("n".into()),
            description: None,
            required: None,
            marketing_activity_create_url: None,
            marketing_activity_delete_url: None,
        }];
        assert!(validate_trigger_schema_presence(&fields, None).is_err());
        assert!(validate_trigger_schema_presence(&fields, Some("./schema.graphql")).is_ok());

        assert!(validate_return_type_config(None, None).is_ok());
        assert!(validate_return_type_config(None, Some("s")).is_err());
        assert!(validate_return_type_config(Some("r"), None).is_err());
        assert!(validate_return_type_config(Some("r"), Some("s")).is_ok());
    }
}
