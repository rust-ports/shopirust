//! Shared helpers for `app function *` commands.

use crate::error::AppError;
use crate::models::extensions::extension_instance::ExtensionInstance;
use crate::models::loader::LoadedApp;
use crate::prompts::{PromptItem, Prompter};
use crate::services::function::schema::{generate_schema_service, SchemaDefinitionFetcher};
use std::path::{Path, PathBuf};

const DEFAULT_FUNCTION_EXPORT: &str = "_start";

/// Directory where `app dev` writes function run logs (`.shopify/logs` under the app root).
pub fn function_logs_dir(app_directory: &Path) -> PathBuf {
    app_directory.join(".shopify").join("logs")
}

/// Pick a function extension from the app, preferring one whose directory matches `path`.
///
/// Multiple functions: interactive [`Prompter::select`] when provided; otherwise error.
pub fn choose_function(
    app: &LoadedApp,
    path: &Path,
    prompter: Option<&dyn Prompter>,
) -> Result<ExtensionInstance, AppError> {
    let all: Vec<&ExtensionInstance> = app
        .all_extensions()
        .iter()
        .filter(|ext| ext.is_function_extension())
        .collect();

    let canonical_path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    if let Some(found) = all.iter().find(|fun| {
        fun.directory
            .canonicalize()
            .unwrap_or_else(|_| fun.directory.clone())
            == canonical_path
    }) {
        return Ok((*found).clone());
    }

    if all.len() == 1 {
        return Ok(all[0].clone());
    }

    if all.is_empty() {
        return Err(AppError::message(
            "No function extensions found in this app.",
        ));
    }

    if let Some(prompter) = prompter {
        let items: Vec<PromptItem> = all
            .iter()
            .map(|fun| {
                PromptItem::new(fun.handle.clone(), fun.handle.clone())
                    .with_hint(fun.directory.display().to_string())
            })
            .collect();
        let handle = prompter.select("Select a function to run", &items)?;
        return all
            .into_iter()
            .find(|fun| fun.handle == handle)
            .cloned()
            .ok_or_else(|| AppError::message(format!("Unknown function '{handle}'.")));
    }

    Err(AppError::message(
        "Multiple functions found. Run this command from a function directory or use `--path`.",
    ))
}

/// Resolve a function export: `--export` flag, single targeting entry, or a select prompt.
pub fn choose_function_export(
    ext: &ExtensionInstance,
    flag: Option<&str>,
    prompter: Option<&dyn Prompter>,
) -> Result<String, AppError> {
    if let Some(export) = flag.filter(|s| !s.is_empty()) {
        return Ok(export.to_string());
    }
    let exports: Vec<String> = ext
        .targeting()
        .into_iter()
        .filter_map(|t| t.export)
        .filter(|s| !s.is_empty())
        .collect();
    if exports.len() <= 1 {
        return Ok(exports
            .into_iter()
            .next()
            .unwrap_or_else(|| DEFAULT_FUNCTION_EXPORT.into()));
    }
    if let Some(prompter) = prompter {
        let items: Vec<PromptItem> = exports
            .iter()
            .map(|e| PromptItem::new(e.clone(), e.clone()))
            .collect();
        return prompter.select("Select a function export", &items);
    }
    Err(AppError::message(
        "Multiple function exports found. Pass `--export`.",
    ))
}

/// Return existing `schema.graphql`, or generate it via `fetcher` when provided.
pub async fn get_or_generate_schema_path(
    extension: &ExtensionInstance,
    api_key: &str,
    org_id: &str,
    fetcher: Option<&dyn SchemaDefinitionFetcher>,
) -> Result<Option<PathBuf>, AppError> {
    let path = extension.directory.join("schema.graphql");
    if path.is_file() {
        return Ok(Some(path));
    }

    let Some(fetcher) = fetcher else {
        return Ok(None);
    };

    generate_schema_service(extension, api_key, org_id, false, fetcher).await?;
    if path.is_file() {
        Ok(Some(path))
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::extensions::create_extension_specification;
    use crate::models::extensions::extension_instance::ExtensionInstance;
    use crate::models::loader::{load_app, LoadAppOptions};
    use std::collections::HashMap;
    use std::fs;
    use tempfile::tempdir;

    fn write_app_with_functions(dir: &Path, handles: &[&str]) {
        fs::write(
            dir.join("shopify.app.toml"),
            "client_id = \"abc\"\nname = \"Demo\"\napplication_url = \"https://e.com\"\n",
        )
        .unwrap();
        for handle in handles {
            let ext = dir.join("extensions").join(handle);
            fs::create_dir_all(&ext).unwrap();
            fs::write(
                ext.join("shopify.extension.toml"),
                format!(
                    "type = \"function\"\nhandle = \"{handle}\"\nname = \"{handle}\"\napi_version = \"2024-01\"\n"
                ),
            )
            .unwrap();
            fs::create_dir_all(ext.join("src")).unwrap();
            fs::write(ext.join("src/index.js"), "export default {};\n").unwrap();
        }
    }

    #[test]
    fn choose_single_function() {
        let dir = tempdir().unwrap();
        write_app_with_functions(dir.path(), &["only-fn"]);
        let app = load_app(LoadAppOptions {
            directory: dir.path().to_path_buf(),
            config_name: None,
            ignore_unknown_extensions: false,
        })
        .unwrap();
        let chosen = choose_function(&app, dir.path(), None).unwrap();
        assert_eq!(chosen.handle, "only-fn");
    }

    #[test]
    fn choose_by_path() {
        let dir = tempdir().unwrap();
        write_app_with_functions(dir.path(), &["a", "b"]);
        let app = load_app(LoadAppOptions {
            directory: dir.path().to_path_buf(),
            config_name: None,
            ignore_unknown_extensions: false,
        })
        .unwrap();
        let fn_dir = dir.path().join("extensions/b");
        let chosen = choose_function(&app, &fn_dir, None).unwrap();
        assert_eq!(chosen.handle, "b");
    }

    #[test]
    fn choose_errors_with_multiple_without_path() {
        let dir = tempdir().unwrap();
        write_app_with_functions(dir.path(), &["a", "b"]);
        let app = load_app(LoadAppOptions {
            directory: dir.path().to_path_buf(),
            config_name: None,
            ignore_unknown_extensions: false,
        })
        .unwrap();
        assert!(choose_function(&app, dir.path(), None).is_err());
    }

    #[test]
    fn choose_prompts_when_multiple() {
        use crate::prompts::InjectedPrompter;
        let dir = tempdir().unwrap();
        write_app_with_functions(dir.path(), &["a", "b"]);
        let app = load_app(LoadAppOptions {
            directory: dir.path().to_path_buf(),
            config_name: None,
            ignore_unknown_extensions: false,
        })
        .unwrap();
        let p = InjectedPrompter::new();
        p.push_select("b");
        let chosen = choose_function(&app, dir.path(), Some(&p)).unwrap();
        assert_eq!(chosen.handle, "b");
    }

    #[test]
    fn choose_export_prompts_when_multiple() {
        use crate::models::extensions::create_extension_specification;
        use crate::prompts::InjectedPrompter;
        use serde_json::json;
        use std::collections::HashMap;
        let spec = create_extension_specification("function").unwrap();
        let mut cfg = HashMap::new();
        cfg.insert(
            "targeting".into(),
            json!([
                { "target": "cart.transform.run", "export": "run" },
                { "target": "cart.transform.run", "export": "run-b" }
            ]),
        );
        let ext = ExtensionInstance::new(
            "fn",
            PathBuf::from("."),
            PathBuf::from("shopify.extension.toml"),
            cfg,
            spec,
        );
        let p = InjectedPrompter::new();
        p.push_select("run-b");
        assert_eq!(
            choose_function_export(&ext, None, Some(&p)).unwrap(),
            "run-b"
        );
        assert_eq!(
            choose_function_export(&ext, Some("run"), None).unwrap(),
            "run"
        );
        assert!(choose_function_export(&ext, None, None).is_err());
    }

    #[tokio::test]
    async fn get_or_generate_returns_existing() {
        let dir = tempdir().unwrap();
        let schema = dir.path().join("schema.graphql");
        fs::write(&schema, "type Query { id: ID }").unwrap();
        let spec = create_extension_specification("function").unwrap();
        let ext = ExtensionInstance::new(
            "fn",
            dir.path().to_path_buf(),
            dir.path().join("shopify.extension.toml"),
            HashMap::new(),
            spec,
        );
        let path = get_or_generate_schema_path(&ext, "key", "org", None)
            .await
            .unwrap();
        assert_eq!(path.unwrap(), schema);
    }

    #[test]
    fn logs_dir_under_shopify() {
        let dir = PathBuf::from("/tmp/my-app");
        assert_eq!(
            function_logs_dir(&dir),
            PathBuf::from("/tmp/my-app/.shopify/logs")
        );
    }
}
