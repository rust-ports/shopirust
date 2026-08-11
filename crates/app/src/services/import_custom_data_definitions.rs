//! Import metafield / metaobject definitions into `shopify.app.toml` (DCDD).

use crate::error::AppError;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetafieldDefinitionInput {
    pub owner_type: String,
    pub namespace: String,
    pub key: String,
    pub name: String,
    pub type_name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetaobjectDefinitionInput {
    pub type_name: String,
    pub name: String,
    pub description: Option<String>,
    pub fields: Vec<MetaobjectFieldInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetaobjectFieldInput {
    pub key: String,
    pub name: String,
    pub type_name: String,
    pub required: bool,
}

#[derive(Debug, Clone)]
pub struct ImportCustomDataOptions {
    pub configuration_path: std::path::PathBuf,
    pub metafields: Vec<MetafieldDefinitionInput>,
    pub metaobjects: Vec<MetaobjectDefinitionInput>,
    /// When false, skip definitions already present in the TOML.
    pub include_existing: bool,
}

#[derive(Debug, Clone)]
pub struct ImportCustomDataResult {
    pub metafield_count: usize,
    pub metaobject_count: usize,
    pub toml_content: String,
}

/// Convert store metafield/metaobject definitions into TOML fragments and merge into app config.
pub fn import_custom_data_definitions(
    options: ImportCustomDataOptions,
) -> Result<ImportCustomDataResult, AppError> {
    let existing = if options.configuration_path.exists() {
        fs::read_to_string(&options.configuration_path)?
    } else {
        String::new()
    };

    let mut metafield_count = 0usize;
    let mut metaobject_count = 0usize;
    let mut additions = String::new();

    for mf in &options.metafields {
        let marker = format!(
            "[[metafields.{}]]\nnamespace = \"{}\"\nkey = \"{}\"",
            mf.owner_type, mf.namespace, mf.key
        );
        if !options.include_existing && existing.contains(&format!("key = \"{}\"", mf.key)) {
            // Rough de-dupe by key within owner section.
            if existing.contains(&format!("namespace = \"{}\"", mf.namespace))
                && existing.contains(&mf.owner_type)
            {
                continue;
            }
        }
        additions.push('\n');
        additions.push_str(&render_metafield_toml(mf));
        metafield_count += 1;
        let _ = marker;
    }

    for mo in &options.metaobjects {
        if !options.include_existing
            && existing.contains(&format!("type = \"{}\"", mo.type_name))
        {
            continue;
        }
        additions.push('\n');
        additions.push_str(&render_metaobject_toml(mo));
        metaobject_count += 1;
    }

    if metafield_count == 0 && metaobject_count == 0 {
        return Ok(ImportCustomDataResult {
            metafield_count: 0,
            metaobject_count: 0,
            toml_content: existing,
        });
    }

    let mut toml_content = existing;
    if !toml_content.ends_with('\n') && !toml_content.is_empty() {
        toml_content.push('\n');
    }
    toml_content.push_str(&additions);
    fs::write(&options.configuration_path, &toml_content)?;

    Ok(ImportCustomDataResult {
        metafield_count,
        metaobject_count,
        toml_content,
    })
}

pub fn render_metafield_toml(mf: &MetafieldDefinitionInput) -> String {
    let mut out = format!(
        "[[metafields.{owner}]]\nname = \"{name}\"\nnamespace = \"{namespace}\"\nkey = \"{key}\"\ntype = \"{type_name}\"\n",
        owner = mf.owner_type,
        name = escape_toml(&mf.name),
        namespace = escape_toml(&mf.namespace),
        key = escape_toml(&mf.key),
        type_name = escape_toml(&mf.type_name),
    );
    if let Some(desc) = &mf.description {
        out.push_str(&format!("description = \"{}\"\n", escape_toml(desc)));
    }
    out
}

pub fn render_metaobject_toml(mo: &MetaobjectDefinitionInput) -> String {
    let mut out = format!(
        "[[metaobjects]]\ntype = \"{type_name}\"\nname = \"{name}\"\n",
        type_name = escape_toml(&mo.type_name),
        name = escape_toml(&mo.name),
    );
    if let Some(desc) = &mo.description {
        out.push_str(&format!("description = \"{}\"\n", escape_toml(desc)));
    }
    for field in &mo.fields {
        out.push_str(&format!(
            "\n[[metaobjects.fields]]\nkey = \"{}\"\nname = \"{}\"\ntype = \"{}\"\nrequired = {}\n",
            escape_toml(&field.key),
            escape_toml(&field.name),
            escape_toml(&field.type_name),
            field.required,
        ));
    }
    out
}

/// Map GraphQL owner enum (e.g. `PRODUCT`) to TOML owner key (`product`).
pub fn graphql_owner_to_toml(owner: &str) -> String {
    owner.trim().to_lowercase()
}

fn escape_toml(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Parse a simplified JSON payload (from Admin API dump / fixture) into import inputs.
pub fn definitions_from_json(value: &serde_json::Value) -> Result<(Vec<MetafieldDefinitionInput>, Vec<MetaobjectDefinitionInput>), AppError> {
    let metafields = value
        .get("metafields")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|item| {
            Some(MetafieldDefinitionInput {
                owner_type: item.get("ownerType")?.as_str()?.to_lowercase(),
                namespace: item.get("namespace")?.as_str()?.to_string(),
                key: item.get("key")?.as_str()?.to_string(),
                name: item
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                type_name: item
                    .get("type")
                    .and_then(|v| v.get("name"))
                    .and_then(|v| v.as_str())
                    .or_else(|| item.get("typeName").and_then(|v| v.as_str()))
                    .unwrap_or("single_line_text_field")
                    .to_string(),
                description: item
                    .get("description")
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
            })
        })
        .collect();

    let metaobjects = value
        .get("metaobjects")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|item| {
            let fields = item
                .get("fieldDefinitions")
                .or_else(|| item.get("fields"))
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .filter_map(|f| {
                    Some(MetaobjectFieldInput {
                        key: f.get("key")?.as_str()?.to_string(),
                        name: f
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        type_name: f
                            .get("type")
                            .and_then(|v| v.get("name"))
                            .and_then(|v| v.as_str())
                            .or_else(|| f.get("typeName").and_then(|v| v.as_str()))
                            .unwrap_or("single_line_text_field")
                            .to_string(),
                        required: f.get("required").and_then(|v| v.as_bool()).unwrap_or(false),
                    })
                })
                .collect();
            Some(MetaobjectDefinitionInput {
                type_name: item.get("type")?.as_str()?.to_string(),
                name: item
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                description: item
                    .get("description")
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
                fields,
            })
        })
        .collect();

    Ok((metafields, metaobjects))
}

pub fn import_custom_data_from_json_file(
    configuration_path: &Path,
    definitions_json: &Path,
    include_existing: bool,
) -> Result<ImportCustomDataResult, AppError> {
    let raw = fs::read_to_string(definitions_json)?;
    let value: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|e| AppError::message(format!("Invalid definitions JSON: {e}")))?;
    let (metafields, metaobjects) = definitions_from_json(&value)?;
    import_custom_data_definitions(ImportCustomDataOptions {
        configuration_path: configuration_path.to_path_buf(),
        metafields,
        metaobjects,
        include_existing,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn renders_metafield_toml() {
        let toml = render_metafield_toml(&MetafieldDefinitionInput {
            owner_type: "product".into(),
            namespace: "custom".into(),
            key: "rating".into(),
            name: "Rating".into(),
            type_name: "number_integer".into(),
            description: Some("Stars".into()),
        });
        assert!(toml.contains("[[metafields.product]]"));
        assert!(toml.contains("key = \"rating\""));
        assert!(toml.contains("description = \"Stars\""));
    }

    #[test]
    fn imports_into_app_toml() {
        let dir = tempdir().unwrap();
        let config = dir.path().join("shopify.app.toml");
        fs::write(&config, "name = \"Demo\"\napplication_url = \"https://e.com\"\n").unwrap();
        let result = import_custom_data_definitions(ImportCustomDataOptions {
            configuration_path: config.clone(),
            metafields: vec![MetafieldDefinitionInput {
                owner_type: "product".into(),
                namespace: "custom".into(),
                key: "color".into(),
                name: "Color".into(),
                type_name: "single_line_text_field".into(),
                description: None,
            }],
            metaobjects: vec![MetaobjectDefinitionInput {
                type_name: "$app:author".into(),
                name: "Author".into(),
                description: None,
                fields: vec![MetaobjectFieldInput {
                    key: "name".into(),
                    name: "Name".into(),
                    type_name: "single_line_text_field".into(),
                    required: true,
                }],
            }],
            include_existing: true,
        })
        .unwrap();
        assert_eq!(result.metafield_count, 1);
        assert_eq!(result.metaobject_count, 1);
        let content = fs::read_to_string(config).unwrap();
        assert!(content.contains("[[metafields.product]]"));
        assert!(content.contains("[[metaobjects]]"));
        assert!(content.contains("required = true"));
    }

    #[test]
    fn parses_definitions_json() {
        let value = serde_json::json!({
            "metafields": [{
                "ownerType": "PRODUCT",
                "namespace": "custom",
                "key": "size",
                "name": "Size",
                "type": { "name": "single_line_text_field" }
            }],
            "metaobjects": [{
                "type": "$app:size",
                "name": "Size",
                "fieldDefinitions": [{
                    "key": "label",
                    "name": "Label",
                    "type": { "name": "single_line_text_field" },
                    "required": false
                }]
            }]
        });
        let (mf, mo) = definitions_from_json(&value).unwrap();
        assert_eq!(mf.len(), 1);
        assert_eq!(mf[0].owner_type, "product");
        assert_eq!(mo.len(), 1);
        assert_eq!(mo[0].fields.len(), 1);
    }
}
