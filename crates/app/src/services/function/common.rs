//! Shared helpers for `app function *` commands.

use crate::error::AppError;
use crate::models::extensions::extension_instance::ExtensionInstance;
use crate::models::loader::LoadedApp;
use crate::services::function::schema::{generate_schema_service, SchemaDefinitionFetcher};
use is_terminal::IsTerminal;
use std::io::stdin;
use std::path::{Path, PathBuf};

/// Directory where `app dev` writes function run logs (`.shopify/logs` under the app root).
pub fn function_logs_dir(app_directory: &Path) -> PathBuf {
    app_directory.join(".shopify").join("logs")
}

/// Pick a function extension from the app, preferring one whose directory matches `path`.
pub fn choose_function(app: &LoadedApp, path: &Path) -> Result<ExtensionInstance, AppError> {
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

    if stdin().is_terminal() {
        // Non-interactive selection without a full prompt UI: require --path.
        // Callers that want a prompt can list handles themselves.
        eprintln!("Multiple functions found:");
        for fun in &all {
            eprintln!("  - {} ({})", fun.handle, fun.directory.display());
        }
    }

    Err(AppError::message(
        "Run this command from a function directory or use `--path` to specify a function directory.",
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
        let chosen = choose_function(&app, dir.path()).unwrap();
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
        let chosen = choose_function(&app, &fn_dir).unwrap();
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
        assert!(choose_function(&app, dir.path()).is_err());
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
