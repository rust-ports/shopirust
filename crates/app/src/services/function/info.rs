//! Format function metadata for `app function info`.

use crate::models::extensions::extension_instance::ExtensionInstance;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionInfoFormat {
    Json,
    Text,
}

#[derive(Debug, Clone)]
pub struct FunctionInfoOptions {
    pub format: FunctionInfoFormat,
    pub function_runner_path: PathBuf,
    pub schema_path: Option<PathBuf>,
}

pub type TargetingMap = BTreeMap<String, TargetingEntry>;

#[derive(Debug, Clone, Default)]
pub struct TargetingEntry {
    pub input_query_path: Option<String>,
    pub export: Option<String>,
}

pub fn build_targeting_data(ext: &ExtensionInstance) -> TargetingMap {
    let mut targeting = TargetingMap::new();
    for target in ext.targeting() {
        let mut entry = TargetingEntry::default();
        if let Some(iq) = target.input_query {
            entry.input_query_path = Some(format!("{}/{}", ext.directory.display(), iq));
        }
        if let Some(export) = target.export {
            entry.export = Some(export);
        }
        targeting.insert(target.target, entry);
    }
    targeting
}

pub fn function_info(ext: &ExtensionInstance, options: FunctionInfoOptions) -> String {
    let targeting = build_targeting_data(ext);
    let wasm_path = ext.function_output_path();

    match options.format {
        FunctionInfoFormat::Json => {
            let mut targeting_json = serde_json::Map::new();
            for (target, entry) in &targeting {
                let mut obj = serde_json::Map::new();
                if let Some(ref iq) = entry.input_query_path {
                    obj.insert("inputQueryPath".into(), Value::String(iq.clone()));
                }
                if let Some(ref export) = entry.export {
                    obj.insert("export".into(), Value::String(export.clone()));
                }
                targeting_json.insert(target.clone(), Value::Object(obj));
            }
            serde_json::to_string_pretty(&json!({
                "handle": ext.handle,
                "name": ext.name(),
                "apiVersion": ext.api_version(),
                "targeting": targeting_json,
                "schemaPath": options.schema_path.as_ref().map(|p| p.display().to_string()),
                "wasmPath": wasm_path.display().to_string(),
                "functionRunnerPath": options.function_runner_path.display().to_string(),
            }))
            .unwrap_or_else(|_| "{}".into())
        }
        FunctionInfoFormat::Text => {
            let mut lines = vec![
                "CONFIGURATION".into(),
                format!("  Handle        {}", ext.handle),
                format!("  Name          {}", ext.name()),
                format!("  API Version   {}", ext.api_version().unwrap_or("N/A")),
            ];
            if !targeting.is_empty() {
                lines.push(String::new());
                lines.push("TARGETING".into());
                for (target, entry) in &targeting {
                    lines.push(format!("  {target}"));
                    if let Some(ref iq) = entry.input_query_path {
                        lines.push(format!("    Input Query Path  {iq}"));
                    }
                    if let Some(ref export) = entry.export {
                        lines.push(format!("    Export            {export}"));
                    }
                }
            }
            lines.push(String::new());
            lines.push("BUILD".into());
            lines.push(format!(
                "  Schema Path   {}",
                options
                    .schema_path
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "N/A".into())
            ));
            lines.push(format!("  Wasm Path     {}", wasm_path.display()));
            lines.push(String::new());
            lines.push("FUNCTION RUNNER".into());
            lines.push(format!(
                "  Path          {}",
                options.function_runner_path.display()
            ));
            lines.join("\n")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::extensions::create_extension_specification;
    use serde_json::json;
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn make_fn(dir: &str, build_path: Option<&str>) -> ExtensionInstance {
        let mut config = HashMap::new();
        config.insert("name".into(), json!("My Function"));
        config.insert("type".into(), json!("function"));
        config.insert("handle".into(), json!("my-function"));
        config.insert("api_version".into(), json!("2024-01"));
        if let Some(path) = build_path {
            config.insert("build".into(), json!({ "path": path }));
        }
        config.insert(
            "targeting".into(),
            json!([{
                "target": "cart.lines.discounts.generate.run",
                "input_query": "src/run.graphql",
                "export": "run"
            }]),
        );
        let spec = create_extension_specification("function").unwrap();
        ExtensionInstance::new(
            "my-function",
            PathBuf::from(dir),
            PathBuf::from(format!("{dir}/shopify.extension.toml")),
            config,
            spec,
        )
    }

    #[test]
    fn json_format_includes_fields() {
        let ext = make_fn("/path/to/function", None);
        let out = function_info(
            &ext,
            FunctionInfoOptions {
                format: FunctionInfoFormat::Json,
                function_runner_path: PathBuf::from("/path/to/runner"),
                schema_path: Some(PathBuf::from("/path/to/schema.graphql")),
            },
        );
        let parsed: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(parsed["handle"], "my-function");
        assert_eq!(parsed["name"], "My Function");
        assert_eq!(parsed["apiVersion"], "2024-01");
        assert!(parsed["targeting"].is_object());
        assert_eq!(parsed["wasmPath"], "/path/to/function/dist/index.wasm");
    }

    #[test]
    fn uses_build_path_when_present() {
        let ext = make_fn("/path/to/function", Some("custom/output.wasm"));
        let out = function_info(
            &ext,
            FunctionInfoOptions {
                format: FunctionInfoFormat::Json,
                function_runner_path: PathBuf::from("/runner"),
                schema_path: None,
            },
        );
        let parsed: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(parsed["wasmPath"], "/path/to/function/custom/output.wasm");
    }

    #[test]
    fn text_format_has_sections() {
        let ext = make_fn("/path/to/function", None);
        let out = function_info(
            &ext,
            FunctionInfoOptions {
                format: FunctionInfoFormat::Text,
                function_runner_path: PathBuf::from("/runner"),
                schema_path: Some(PathBuf::from("/schema.graphql")),
            },
        );
        assert!(out.contains("CONFIGURATION"));
        assert!(out.contains("TARGETING"));
        assert!(out.contains("BUILD"));
        assert!(out.contains("FUNCTION RUNNER"));
    }
}
