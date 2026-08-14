pub mod upload;

use crate::error::AppError;
use crate::models::extensions::ExtensionInstance;
use crate::validations::{validate_message, validate_version};
use crate::prompts::deploy_release::{
    deploy_or_release_confirmation_prompt, DeployConfirmOptions,
};
use crate::prompts::Prompter;
use crate::services::bundle::{
    compress_bundle, default_bundle_path, write_manifest_to_bundle, AppManifest,
};
use crate::services::context::identifiers::{
    ensure_deployment_ids_presence, persist_identifiers, EnsureIdsOptions,
};
use crate::services::context::LinkedAppContext;
use crate::services::deploy::upload::{
    format_partners_deploy_error, upload_extensions_bundle, UploadExtensionsBundleOptions,
};
use crate::services::import_extensions::{
    filter_out_imported_extensions, import_extensions, ExtensionRegistration,
    ImportExtensionsOptions,
};
use cli_api::{DeveloperPlatformClient, MinimalAppIdentifiers};
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
    prompter: Option<&dyn Prompter>,
) -> Result<DeployResult, AppError> {
    validate_version(options.version.as_deref())?;
    validate_message(options.message.as_deref())?;
    import_extensions_if_needed(ctx, client, prompter).await?;

    let ids = ensure_deployment_ids_presence(
        ctx,
        client,
        EnsureIdsOptions {
            allow_updates: options.allow_updates || options.force,
        },
        prompter,
    )
    .await?;

    let confirmed = deploy_or_release_confirmation_prompt(
        prompter,
        &DeployConfirmOptions {
            app_title: Some(ctx.remote_app.title.clone()),
            release: !options.no_release,
            force: options.force,
            allow_updates: options.allow_updates,
            allow_deletes: options.allow_deletes,
            is_tty: options.is_tty,
        },
        &ids.breakdown,
    )?;
    if !confirmed {
        return Err(AppError::message("Deploy cancelled."));
    }

    let bundle_dir = ctx.app.directory.join(".shopify").join("deploy-bundle");
    prepare_bundle_directory(&bundle_dir, &ctx.app.extensions, options.no_build)?;

    let include_config = ctx
        .app
        .configuration
        .build
        .as_ref()
        .and_then(|b| b.include_config_on_deploy)
        .unwrap_or(true);
    let mut modules = Vec::new();
    let app_config_json =
        serde_json::to_value(&ctx.app.configuration).unwrap_or(serde_json::json!({}));
    for e in &ctx.app.extensions {
        if !include_on_deploy(e, include_config) {
            continue;
        }
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

    write_manifest_to_bundle(
        &AppManifest {
            name: ctx.app.name.clone(),
            handle: None,
            modules: modules.clone(),
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
            app_modules: modules,
        },
    )
    .await?;

    persist_identifiers(&ctx.app.directory, &ids)?;

    let user_errors = if client.supports_atomic_deployments() {
        upload.user_errors
    } else {
        upload
            .user_errors
            .iter()
            .map(|m| format_partners_deploy_error(m))
            .collect()
    };
    let success = user_errors.is_empty();
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
            format!("Deploy failed: {}", user_errors.join(", "))
        },
        user_errors,
    })
}

pub(crate) fn include_on_deploy(
    ext: &ExtensionInstance,
    include_config: bool,
) -> bool {
    include_config || !ext.specification.is_app_config()
}

async fn import_extensions_if_needed(
    ctx: &LinkedAppContext,
    client: &dyn DeveloperPlatformClient,
    prompter: Option<&dyn Prompter>,
) -> Result<(), AppError> {
    if !client.supports_dashboard_managed_extensions() && client.supports_atomic_deployments() {
        // App Management: still block if dashboard-managed leftovers exist.
    }
    let identifiers = MinimalAppIdentifiers {
        api_key: ctx.remote_app.api_key.clone(),
        organization_id: ctx
            .remote_app
            .organization_id
            .clone()
            .unwrap_or_else(|| ctx.organization.id.clone()),
        id: ctx.remote_app.id.clone(),
    };
    let regs = client
        .app_extension_registrations(&identifiers)
        .await
        .map_err(|e| AppError::message(e.to_string()))?;
    let dashboard = regs
        .pointer("/dashboardManagedExtensionRegistrations")
        .or_else(|| regs.pointer("/app/dashboardManagedExtensionRegistrations"))
        .or_else(|| regs.pointer("/data/app/dashboardManagedExtensionRegistrations"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    if dashboard.is_empty() {
        return Ok(());
    }
    let pending: Vec<ExtensionRegistration> = dashboard.iter().filter_map(parse_dashboard_registration).collect();
    let env_uuids = std::collections::HashMap::new();
    let pending = filter_out_imported_extensions(&ctx.app, &pending, &env_uuids);
    let local: std::collections::HashSet<String> = ctx
        .app
        .extensions
        .iter()
        .filter_map(|e| e.uid.clone())
        .collect();
    let pending: Vec<_> = pending
        .into_iter()
        .filter(|e| !local.contains(&e.uuid))
        .collect();
    if pending.is_empty() {
        return Ok(());
    }
    if client.supports_atomic_deployments() {
        return Err(AppError::message(
            "Dashboard-managed extensions must be imported before deploy. Run `shopify app import-extensions`.",
        ));
    }
    let confirmed = match prompter {
        Some(p) => p.confirm(&format!(
            "Import {} dashboard-managed extension(s) before deploy?",
            pending.len()
        ))?,
        None => false,
    };
    if !confirmed {
        return Err(AppError::message(
            "Deploy cancelled: dashboard extensions not imported.",
        ));
    }
    import_extensions(
        &ctx.app,
        ImportExtensionsOptions {
            extensions: pending,
            extension_types: vec![],
            all: true,
            overwrite_existing: false,
            app_embedded: ctx.app.configuration.embedded.unwrap_or(false),
        },
    )?;
    Ok(())
}

fn parse_dashboard_registration(value: &serde_json::Value) -> Option<ExtensionRegistration> {
    Some(ExtensionRegistration {
        uuid: value.get("uuid")?.as_str()?.to_string(),
        title: value
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("extension")
            .to_string(),
        type_name: value
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("theme")
            .to_string(),
        draft_version: None,
        active_version: None,
    })
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
    use super::parse_dashboard_registration;
    use crate::prompts::deploy_release::{
        should_skip_confirmation_prompt, DeployConfirmOptions,
    };
    use crate::services::context::breakdown_extensions::ExtensionBreakdown;

    fn opts(allow_updates: bool, allow_deletes: bool, force: bool, is_tty: bool) -> DeployConfirmOptions {
        DeployConfirmOptions {
            app_title: Some("Demo".into()),
            release: true,
            force,
            allow_updates,
            allow_deletes,
            is_tty,
        }
    }

    fn with_updates() -> ExtensionBreakdown {
        ExtensionBreakdown {
            to_create: vec!["new-ext".into()],
            ..Default::default()
        }
    }

    #[test]
    fn confirm_deploy_requires_flag_non_tty() {
        let err = should_skip_confirmation_prompt(&opts(false, false, false, false), &with_updates())
            .unwrap_err();
        assert!(err.to_string().contains("--allow-updates"));
    }

    #[test]
    fn confirm_deploy_allow_updates_skips() {
        assert!(should_skip_confirmation_prompt(&opts(true, false, false, false), &with_updates()).unwrap());
    }

    #[test]
    fn parses_dashboard_registration() {
        let parsed = parse_dashboard_registration(&serde_json::json!({
            "uuid": "u1",
            "title": "My Theme",
            "type": "theme_app_extension"
        }))
        .unwrap();
        assert_eq!(parsed.uuid, "u1");
        assert_eq!(parsed.type_name, "theme_app_extension");
    }

    #[test]
    fn include_on_deploy_filters_config_extensions() {
        use crate::models::extensions::create_extension_specification;
        use crate::models::extensions::extension_instance::ExtensionInstance;
        use std::collections::HashMap;
        use std::path::PathBuf;

        let branding = ExtensionInstance::new(
            "branding",
            PathBuf::from("."),
            PathBuf::from("shopify.app.toml"),
            HashMap::new(),
            create_extension_specification("branding").unwrap(),
        );
        let ui = ExtensionInstance::new(
            "my-ui",
            PathBuf::from("."),
            PathBuf::from("shopify.extension.toml"),
            HashMap::new(),
            create_extension_specification("ui_extension").unwrap(),
        );
        assert!(!super::include_on_deploy(&branding, false));
        assert!(super::include_on_deploy(&branding, true));
        assert!(super::include_on_deploy(&ui, false));
        assert!(super::include_on_deploy(&ui, true));
    }

    #[test]
    fn confirm_deploy_force_skips() {
        assert!(should_skip_confirmation_prompt(&opts(false, false, true, false), &with_updates()).unwrap());
    }

    #[test]
    fn rejects_invalid_version_name() {
        use crate::test_support::{sample_org_app, MockClient};
        use crate::services::context::LinkedAppContext;
        use crate::models::loader::LoadedApp;
        use std::path::PathBuf;

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let err = rt.block_on(async {
            let client = MockClient::with_app(sample_org_app("k"));
            let ctx = LinkedAppContext {
                app: LoadedApp {
                    directory: PathBuf::from("."),
                    configuration_path: PathBuf::from("shopify.app.toml"),
                    configuration: Default::default(),
                    hidden_config: Default::default(),
                    extensions: vec![],
                    webs: vec![],
                    identifiers: Default::default(),
                    name: "Demo".into(),
                    errors: vec![],
                    dev_application_urls: None,
                },
                remote_app: sample_org_app("k"),
                organization: cli_api::Organization {
                    id: "org-1".into(),
                    business_name: "Acme".into(),
                    source: cli_api::OrganizationSource::BusinessPlatform,
                },
            };
            super::deploy(
                &ctx,
                &client,
                super::DeployOptions {
                    message: None,
                    version: Some("bad version!".into()),
                    no_build: true,
                    no_release: true,
                    allow_updates: true,
                    allow_deletes: true,
                    force: true,
                    is_tty: false,
                    source_control_url: None,
                },
                None,
            )
            .await
        })
        .unwrap_err();
        assert!(err.to_string().contains("Invalid version"));
    }
}
