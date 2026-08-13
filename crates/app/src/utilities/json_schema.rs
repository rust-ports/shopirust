//! Lightweight JSON Schema checks used when merging remote contracts with local config.
//!
//! Upstream `utilities/json-schema.ts` uses AJV. This port covers the unit-tested surface:
//! empty-schema fallback, required-property errors, and combining local + contract errors.

use serde_json::Value;
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaError {
    pub path: Vec<String>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseState {
    Ok,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseConfigurationResult {
    pub state: ParseState,
    pub data: Option<Value>,
    pub errors: Option<Vec<SchemaError>>,
}

pub type LocalParser = fn(&Value) -> ParseConfigurationResult;

const BASE_PROPERTIES: &[&str] = &["type", "handle", "uid", "path", "extensions"];

/// Validate `subject` against a JSON Schema object.
///
/// Uses the `jsonschema` crate for draft-07 (`$ref`, `allOf`, `enum`, …) and
/// falls back to the lightweight required/type checker if compilation fails.
pub fn json_schema_validate(subject: &Value, schema: &Value) -> ParseConfigurationResult {
    if let Ok(validator) = jsonschema::validator_for(schema) {
        let errors: Vec<SchemaError> = validator
            .iter_errors(subject)
            .map(|e| {
                let message = map_jsonschema_message(&e.to_string());
                let mut path = instance_path_to_vec(&e.instance_path.to_string());
                if path.is_empty() {
                    if let Some(name) = required_property_name(&message) {
                        path.push(name);
                    }
                }
                SchemaError { path, message }
            })
            .collect();
        return if errors.is_empty() {
            ParseConfigurationResult {
                state: ParseState::Ok,
                data: Some(subject.clone()),
                errors: None,
            }
        } else {
            ParseConfigurationResult {
                state: ParseState::Error,
                data: None,
                errors: Some(errors),
            }
        };
    }
    json_schema_validate_simple(subject, schema)
}

fn instance_path_to_vec(path: &str) -> Vec<String> {
    path.split('/')
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

fn required_property_name(message: &str) -> Option<String> {
    message
        .strip_prefix("must have required property '")
        .and_then(|s| s.strip_suffix("'"))
        .map(str::to_string)
}

fn map_jsonschema_message(message: &str) -> String {
    // jsonschema: `"name" is a required property` → AJV-style message used in tests.
    let trimmed = message.trim();
    if let Some(name) = trimmed
        .strip_prefix('"')
        .and_then(|s| s.split_once("\" is a required property"))
        .map(|(n, _)| n)
    {
        return format!("must have required property '{name}'");
    }
    if let Some(rest) = trimmed.strip_prefix('"') {
        if let Some((_, after)) = rest.split_once("\" is not of type ") {
            let ty = after.trim().trim_matches('"');
            return format!("must be {ty}");
        }
    }
    message.to_string()
}

fn json_schema_validate_simple(subject: &Value, schema: &Value) -> ParseConfigurationResult {
    let mut errors = Vec::new();
    if let Some(required) = schema.get("required").and_then(|v| v.as_array()) {
        for key in required {
            let Some(name) = key.as_str() else { continue };
            if subject.get(name).is_none() {
                errors.push(SchemaError {
                    path: vec![name.to_string()],
                    message: format!("must have required property '{name}'"),
                });
            }
        }
    }
    if let Some(properties) = schema.get("properties").and_then(|v| v.as_object()) {
        if let Some(obj) = subject.as_object() {
            for (key, value) in obj {
                if let Some(prop_schema) = properties.get(key) {
                    if let Some(expected) = prop_schema.get("type").and_then(|t| t.as_str()) {
                        if !value_matches_type(value, expected) {
                            errors.push(SchemaError {
                                path: vec![key.clone()],
                                message: format!("must be {expected}"),
                            });
                        }
                    }
                }
            }
        }
    }
    if errors.is_empty() {
        ParseConfigurationResult {
            state: ParseState::Ok,
            data: Some(subject.clone()),
            errors: None,
        }
    } else {
        ParseConfigurationResult {
            state: ParseState::Error,
            data: None,
            errors: Some(errors),
        }
    }
}

#[cfg(test)]
mod draft07_tests {
    use super::*;

    #[test]
    fn validates_enum_and_ref() {
        let schema = serde_json::json!({
            "$defs": {
                "mode": { "type": "string", "enum": ["online", "offline"] }
            },
            "type": "object",
            "properties": {
                "mode": { "$ref": "#/$defs/mode" }
            },
            "required": ["mode"]
        });
        let ok = json_schema_validate(&serde_json::json!({"mode": "online"}), &schema);
        assert_eq!(ok.state, ParseState::Ok);
        let bad = json_schema_validate(&serde_json::json!({"mode": "nope"}), &schema);
        assert_eq!(bad.state, ParseState::Error);
    }

    #[test]
    fn validates_all_of_any_of_nested_required_and_pattern() {
        let schema = serde_json::json!({
            "allOf": [
                {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string", "pattern": "^[A-Z]" }
                    },
                    "required": ["name"]
                },
                {
                    "anyOf": [
                        { "properties": { "kind": { "const": "a" } }, "required": ["kind"] },
                        { "properties": { "kind": { "const": "b" } }, "required": ["kind"] }
                    ]
                }
            ],
            "properties": {
                "nested": {
                    "type": "object",
                    "required": ["id"],
                    "properties": { "id": { "type": "string" } }
                }
            }
        });
        let ok = json_schema_validate(
            &serde_json::json!({"name": "Alpha", "kind": "a", "nested": {"id": "1"}}),
            &schema,
        );
        assert_eq!(ok.state, ParseState::Ok);
        let missing = json_schema_validate(
            &serde_json::json!({"name": "alpha", "kind": "z"}),
            &schema,
        );
        assert_eq!(missing.state, ParseState::Error);
    }
}

fn value_matches_type(value: &Value, expected: &str) -> bool {
    match expected {
        "string" => value.is_string(),
        "number" => value.is_number(),
        "integer" => value.as_i64().is_some() || value.as_u64().is_some(),
        "boolean" => value.is_boolean(),
        "object" => value.is_object(),
        "array" => value.is_array(),
        "null" => value.is_null(),
        _ => true,
    }
}

fn is_empty_schema(schema_json: &str) -> bool {
    match serde_json::from_str::<Value>(schema_json) {
        Ok(Value::Object(map)) => map.is_empty(),
        Ok(Value::Null) => true,
        Err(_) => true,
        _ => false,
    }
}

/// Merge a local parser with an optional remote JSON Schema contract.
pub fn unified_configuration_parser(
    local_parse: LocalParser,
    json_schema: Option<&str>,
) -> impl Fn(&Value) -> ParseConfigurationResult + '_ {
    move |config: &Value| {
        if json_schema.is_none() || json_schema.is_some_and(is_empty_schema) {
            return local_parse(config);
        }
        let mut contract: Value =
            serde_json::from_str(json_schema.unwrap()).unwrap_or_else(|_| serde_json::json!({}));
        if let Some(props) = contract.get_mut("properties") {
            if let Some(obj) = props.as_object_mut() {
                for key in BASE_PROPERTIES {
                    obj.entry((*key).to_string()).or_insert_with(|| {
                        if *key == "extensions" {
                            serde_json::json!({})
                        } else {
                            serde_json::json!({"type": "string"})
                        }
                    });
                }
            }
        }

        let zod_parse = local_parse(config);
        let subject = zod_parse.data.as_ref().unwrap_or(config);
        let json_parse = json_schema_validate(subject, &contract);

        let mut errors = zod_parse.errors.clone().unwrap_or_default();
        if json_parse.state == ParseState::Error {
            errors.extend(json_parse.errors.unwrap_or_default());
        }
        let mut seen = HashSet::new();
        errors.retain(|e| {
            let key = format!("{:?}|{}", e.path, e.message);
            seen.insert(key)
        });

        if zod_parse.state != ParseState::Ok || !errors.is_empty() {
            return ParseConfigurationResult {
                state: ParseState::Error,
                data: None,
                errors: Some(errors),
            };
        }
        ParseConfigurationResult {
            state: ParseState::Ok,
            data: json_parse.data,
            errors: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_parse(config: &Value) -> ParseConfigurationResult {
        if config.get("type").and_then(|v| v.as_str()) == Some("invalid") {
            return ParseConfigurationResult {
                state: ParseState::Error,
                data: None,
                errors: Some(vec![SchemaError {
                    path: vec!["type".into()],
                    message: "Invalid type".into(),
                }]),
            };
        }
        ParseConfigurationResult {
            state: ParseState::Ok,
            data: Some(config.clone()),
            errors: None,
        }
    }

    #[test]
    fn falls_back_when_no_json_schema() {
        let parser = unified_configuration_parser(mock_parse, None);
        let result = parser(&serde_json::json!({"type": "product_subscription"}));
        assert_eq!(result.state, ParseState::Ok);
        assert_eq!(
            result.data,
            Some(serde_json::json!({"type": "product_subscription"}))
        );
        assert!(result.errors.is_none());
    }

    #[test]
    fn falls_back_when_json_schema_empty() {
        let parser = unified_configuration_parser(mock_parse, Some("{}"));
        let result = parser(&serde_json::json!({"type": "product_subscription"}));
        assert_eq!(result.state, ParseState::Ok);
        assert_eq!(
            result.data,
            Some(serde_json::json!({"type": "product_subscription"}))
        );
    }

    #[test]
    fn validates_when_both_succeed() {
        let parser = unified_configuration_parser(
            mock_parse,
            Some(r#"{"type":"object","properties":{"type":{"type":"string"}}}"#),
        );
        let result = parser(&serde_json::json!({"type": "product_subscription"}));
        assert_eq!(result.state, ParseState::Ok);
        assert_eq!(
            result.data,
            Some(serde_json::json!({"type": "product_subscription"}))
        );
    }

    #[test]
    fn returns_errors_when_local_validation_fails() {
        let parser = unified_configuration_parser(
            mock_parse,
            Some(r#"{"type":"object","properties":{"type":{"type":"string"}}}"#),
        );
        let result = parser(&serde_json::json!({"type": "invalid"}));
        assert_eq!(result.state, ParseState::Error);
        assert!(result.data.is_none());
        assert_eq!(result.errors.as_ref().unwrap().len(), 1);
        assert_eq!(result.errors.as_ref().unwrap()[0].path, vec!["type"]);
        assert_eq!(result.errors.as_ref().unwrap()[0].message, "Invalid type");
    }

    #[test]
    fn returns_errors_when_json_schema_fails() {
        let parser = unified_configuration_parser(
            mock_parse,
            Some(r#"{"type":"object","properties":{"type":{"type":"string"}},"required":["price"]}"#),
        );
        let result = parser(&serde_json::json!({"type": "product_subscription"}));
        assert_eq!(result.state, ParseState::Error);
        assert!(result.data.is_none());
        let errors = result.errors.unwrap();
        assert!(!errors.is_empty());
        assert!(errors.iter().any(|e| e.path.contains(&"price".to_string())));
    }

    #[test]
    fn combines_errors_from_both_validations() {
        let parser = unified_configuration_parser(
            mock_parse,
            Some(r#"{"type":"object","properties":{"type":{"type":"string"}},"required":["price"]}"#),
        );
        let result = parser(&serde_json::json!({"type": "invalid"}));
        assert_eq!(result.state, ParseState::Error);
        let errors = result.errors.unwrap();
        assert!(errors.len() > 1);
        assert!(errors.iter().any(|e| e.path.contains(&"type".to_string())));
        assert!(errors.iter().any(|e| e.path.contains(&"price".to_string())));
    }

    #[test]
    fn adds_base_properties_to_the_json_schema() {
        let parser = unified_configuration_parser(
            mock_parse,
            Some(r#"{"type":"object","properties":{"custom":{"type":"string"}}}"#),
        );
        let result = parser(&serde_json::json!({
            "type": "product_subscription",
            "handle": "test-handle",
            "uid": "test-uid",
            "path": "test-path",
            "extensions": {},
            "custom": "value",
        }));
        assert_eq!(result.state, ParseState::Ok);
        let data = result.data.unwrap();
        assert_eq!(data["handle"], "test-handle");
        assert_eq!(data["custom"], "value");
    }
}
