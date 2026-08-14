//! Partners `extensionUpdateDraft` — push a rebuilt extension as a draft.

use crate::error::AppError;
use crate::models::extensions::deploy::DeployConfigContext;
use crate::models::extensions::ExtensionInstance;
use base64::Engine;
use cli_api::{DeveloperPlatformClient, ExtensionUpdateDraftInput};
use serde_json::{json, Value};
use std::fs;
use std::path::Path;

/// Update a Partners draft registration with the current local build.
pub async fn update_extension_draft(
    extension: &ExtensionInstance,
    client: &dyn DeveloperPlatformClient,
    api_key: &str,
    registration_id: &str,
    app_configuration: Option<Value>,
    bundle_path: &Path,
) -> Result<(), AppError> {
    let ctx = DeployConfigContext {
        app_configuration,
        api_key: api_key.to_string(),
        module_id: extension.uid.clone(),
    };
    let mut config = extension
        .deploy_config(&ctx)
        .await?
        .unwrap_or_else(|| json!({}));

    if extension.has_esbuild_feature() {
        let file_path = esbuild_output_path(extension, bundle_path);
        if file_path.exists() {
            let bytes = fs::read(&file_path)?;
            if !bytes.is_empty() {
                let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
                if let Value::Object(ref mut map) = config {
                    map.insert("serialized_script".into(), Value::String(encoded));
                }
            }
        }
    }

    if extension.is_function_extension() {
        let wasm = extension.function_output_path();
        if wasm.exists() {
            let bytes = fs::read(&wasm)?;
            if !bytes.is_empty() {
                let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
                if let Value::Object(ref mut map) = config {
                    map.insert(
                        "uploaded_files".into(),
                        json!({ "dist/index.wasm": encoded }),
                    );
                }
            }
        }
    }

    let result = client
        .update_extension(&ExtensionUpdateDraftInput {
            api_key: api_key.to_string(),
            registration_id: registration_id.to_string(),
            config: config.to_string(),
            context: extension.context_value(),
            handle: Some(extension.handle.clone()),
        })
        .await
        .map_err(|e| AppError::message(e.to_string()))?;

    if !result.user_errors.is_empty() {
        let msg = result
            .user_errors
            .iter()
            .map(|e| e.message.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        return Err(AppError::message(format!(
            "Failed to update draft for '{}': {msg}",
            extension.handle
        )));
    }
    tracing::info!(
        target: "app_dev",
        "Draft updated for extension {}",
        extension.handle
    );
    Ok(())
}

fn esbuild_output_path(extension: &ExtensionInstance, bundle_path: &Path) -> std::path::PathBuf {
    if let Some(ref out) = extension.output_path {
        if out.exists() {
            return out.clone();
        }
    }
    let bundled = extension.get_output_path_for_directory(bundle_path);
    if bundled.exists() {
        return bundled;
    }
    extension.directory.join("dist").join("index.js")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::extensions::create_extension_specification;
    use crate::test_support::MockClient;
    use std::collections::HashMap;
    use std::io::Write;
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[tokio::test]
    async fn pushes_serialized_script_for_esbuild_extension() {
        let dir = tempdir().unwrap();
        let dist = dir.path().join("dist");
        fs::create_dir_all(&dist).unwrap();
        let js = dist.join("index.js");
        {
            let mut f = fs::File::create(&js).unwrap();
            f.write_all(b"console.log('ok')").unwrap();
        }
        let spec = create_extension_specification("ui_extension").unwrap();
        let mut ext = ExtensionInstance::new(
            "checkout-ui",
            dir.path().to_path_buf(),
            dir.path().join("shopify.extension.toml"),
            HashMap::new(),
            spec,
        );
        ext.output_path = Some(js);

        let client = MockClient::default();
        update_extension_draft(
            &ext,
            &client,
            "api-key",
            "reg-1",
            None,
            dir.path(),
        )
        .await
        .unwrap();

        let calls = client.updated_extensions.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].registration_id, "reg-1");
        assert!(calls[0].config.contains("serialized_script"));
    }

    #[tokio::test]
    async fn surfaces_user_errors() {
        let spec = create_extension_specification("theme").unwrap();
        let ext = ExtensionInstance::new(
            "theme-ext",
            PathBuf::from("/tmp/ext"),
            PathBuf::from("/tmp/ext/shopify.extension.toml"),
            HashMap::new(),
            spec,
        );
        let client = MockClient {
            update_errors: vec!["boom".into()],
            ..Default::default()
        };
        let err = update_extension_draft(&ext, &client, "k", "id", None, Path::new("/tmp"))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("boom"));
    }
}
