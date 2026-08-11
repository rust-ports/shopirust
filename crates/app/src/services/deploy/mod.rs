pub mod upload;

use crate::error::AppError;
use crate::models::extensions::ExtensionInstance;
use crate::services::bundle::{
    compress_bundle, default_bundle_path, write_manifest_to_bundle, AppManifest,
};
use crate::services::context::identifiers::{
    ensure_deployment_ids_presence, persist_identifiers, EnsureIdsOptions,
};
use crate::services::context::LinkedAppContext;
use crate::services::deploy::upload::{upload_extensions_bundle, UploadExtensionsBundleOptions};
use cli_api::DeveloperPlatformClient;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct DeployOptions {
    pub message: Option<String>,
    pub version: Option<String>,
    pub no_build: bool,
    pub no_release: bool,
    pub allow_updates: bool,
    pub allow_deletes: bool,
    pub force: bool,
    pub is_tty: bool,
    pub source_control_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DeployResult {
    pub success: bool,
    pub version_id: Option<String>,
    pub message: String,
    pub user_errors: Vec<String>,
}

pub async fn deploy(
    ctx: &LinkedAppContext,
    client: &dyn DeveloperPlatformClient,
    options: DeployOptions,
) -> Result<DeployResult, AppError> {
    confirm_deploy(&options)?;

    let ids = ensure_deployment_ids_presence(
        ctx,
        client,
        EnsureIdsOptions {
            allow_updates: options.allow_updates || options.force,
        },
    )
    .await?;

    let bundle_dir = ctx.app.directory.join(".shopify").join("deploy-bundle");
    prepare_bundle_directory(&bundle_dir, &ctx.app.extensions, options.no_build)?;

    write_manifest_to_bundle(
        &AppManifest {
            name: ctx.app.name.clone(),
            handle: None,
            modules: {
                let mut modules = Vec::new();
                let app_config_json = serde_json::to_value(&ctx.app.configuration)
                    .unwrap_or(serde_json::json!({}));
                for e in &ctx.app.extensions {
                    e.validate()?;
                    let deploy_ctx = crate::models::extensions::DeployConfigContext {
                        app_configuration: Some(app_config_json.clone()),
                        api_key: ctx.remote_app.api_key.clone(),
                        module_id: ids.extensions.get(&e.handle).cloned(),
                    };
                    let config = e.deploy_config(&deploy_ctx).await?.unwrap_or_default();
                    modules.push(serde_json::json!({
                        "uuid": ids.extensions.get(&e.handle).cloned().unwrap_or_default(),
                        "handle": e.handle,
                        "type": e.type_name(),
                        "uid": e.uid.clone().unwrap_or_default(),
                        "config": config,
                    }));
                }
                modules
            },
        },
        &bundle_dir,
    )?;

    let format = client.bundle_format();
    let bundle_path = default_bundle_path(&ctx.app.directory, format);
    compress_bundle(&bundle_dir, &bundle_path)?;

    let upload = upload_extensions_bundle(
        ctx,
        client,
        UploadExtensionsBundleOptions {
            bundle_path: bundle_path.clone(),
            message: options.message.clone(),
            version: options.version.clone(),
            no_release: options.no_release,
            source_control_url: options.source_control_url.clone(),
        },
    )
    .await?;

    persist_identifiers(&ctx.app.directory, &ids)?;

    let success = upload.user_errors.is_empty();
    Ok(DeployResult {
        success,
        version_id: upload.version_id,
        message: if success {
            if options.no_release {
                "App version created (not released).".into()
            } else {
                "App deployed and released.".into()
            }
        } else {
            format!("Deploy failed: {}", upload.user_errors.join(", "))
        },
        user_errors: upload.user_errors,
    })
}

fn confirm_deploy(options: &DeployOptions) -> Result<(), AppError> {
    if options.force || options.allow_updates {
        return Ok(());
    }
    if !options.is_tty {
        return Err(AppError::message(
            "Non-interactive deploy requires --allow-updates (or --force via allow-updates+allow-deletes)",
        ));
    }
    Err(AppError::message(
        "Deploy aborted. Pass --allow-updates to confirm.",
    ))
}

fn prepare_bundle_directory(
    bundle_dir: &Path,
    extensions: &[ExtensionInstance],
    no_build: bool,
) -> Result<(), AppError> {
    if bundle_dir.exists() {
        fs::remove_dir_all(bundle_dir)?;
    }
    fs::create_dir_all(bundle_dir)?;

    for ext in extensions {
        let dest = bundle_dir.join(&ext.handle);
        fs::create_dir_all(&dest)?;
        if no_build {
            copy_dir_contents(&ext.directory, &dest)?;
        } else if let Some(ref out) = ext.output_path {
            if out.exists() {
                copy_path(out, &dest.join(out.file_name().unwrap_or_default()))?;
            } else {
                copy_dir_contents(&ext.directory, &dest)?;
            }
        } else {
            copy_dir_contents(&ext.directory, &dest)?;
        }
    }
    Ok(())
}

fn copy_dir_contents(src: &Path, dest: &Path) -> Result<(), AppError> {
    fs::create_dir_all(dest)?;
    if !src.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let target = dest.join(entry.file_name());
        if path.is_dir() {
            copy_dir_contents(&path, &target)?;
        } else {
            fs::copy(&path, &target)?;
        }
    }
    Ok(())
}

fn copy_path(src: &Path, dest: &Path) -> Result<(), AppError> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    if src.is_dir() {
        copy_dir_contents(src, dest)?;
    } else {
        fs::copy(src, dest)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confirm_deploy_requires_flag_non_tty() {
        let err = confirm_deploy(&DeployOptions {
            message: None,
            version: None,
            no_build: true,
            no_release: false,
            allow_updates: false,
            allow_deletes: false,
            force: false,
            is_tty: false,
            source_control_url: None,
        })
        .unwrap_err();
        assert!(err.to_string().contains("--allow-updates"));
    }
}
