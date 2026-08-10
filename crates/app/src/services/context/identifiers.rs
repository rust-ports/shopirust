use crate::error::AppError;
use crate::local_storage::{set_cached_app_info, CachedAppInfo};
use crate::models::identifiers::Identifiers;
use crate::services::context::LinkedAppContext;
use crate::services::context::breakdown_extensions::{
    extensions_identifiers_deploy_breakdown, ExtensionBreakdown,
};
use cli_api::DeveloperPlatformClient;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Default)]
pub struct EnsureIdsOptions {
    pub allow_updates: bool,
}

#[derive(Debug, Clone)]
pub struct DeploymentIds {
    pub app: Option<String>,
    pub extensions: HashMap<String, String>,
    pub breakdown: ExtensionBreakdown,
}

/// Ensure every local extension has a remote registration id (create-on-deploy stubs).
pub async fn ensure_deployment_ids_presence(
    ctx: &LinkedAppContext,
    client: &dyn DeveloperPlatformClient,
    _options: EnsureIdsOptions,
) -> Result<DeploymentIds, AppError> {
    let breakdown = extensions_identifiers_deploy_breakdown(ctx, client).await?;
    let mut extensions = HashMap::new();

    // Existing matches from remote registrations / prior identifiers
    for (handle, uuid) in &breakdown.matched {
        extensions.insert(handle.clone(), uuid.clone());
    }
    // To-create extensions get provisional local UUIDs (platform assigns on deploy)
    for handle in &breakdown.to_create {
        extensions
            .entry(handle.clone())
            .or_insert_with(|| format!("pending:{handle}"));
    }
    // Already-identified locals
    for ext in &ctx.app.extensions {
        if let Some(ref uid) = ext.uid {
            extensions.entry(ext.handle.clone()).or_insert_with(|| uid.clone());
        } else {
            extensions
                .entry(ext.handle.clone())
                .or_insert_with(|| format!("local:{}", ext.handle));
        }
    }

    Ok(DeploymentIds {
        app: Some(ctx.remote_app.id.clone()),
        extensions,
        breakdown,
    })
}

pub fn persist_identifiers(directory: &Path, ids: &DeploymentIds) -> Result<(), AppError> {
    let mut identifiers = Identifiers::new().with_app(ids.app.clone().unwrap_or_default());
    for (k, v) in &ids.extensions {
        identifiers.set_extension(k, v);
    }

    let shopify = directory.join(".shopify");
    fs::create_dir_all(&shopify)?;
    let path = shopify.join("deploy-identifiers.json");
    fs::write(path, serde_json::to_string_pretty(&identifiers)?)?;

    set_cached_app_info(&CachedAppInfo {
        directory: directory.display().to_string(),
        config_file: None,
        app_id: ids.app.clone(),
        title: None,
        org_id: None,
        store_fqdn: None,
    })?;
    Ok(())
}

pub fn load_persisted_identifiers(directory: &Path) -> Option<Identifiers> {
    let path = directory.join(".shopify").join("deploy-identifiers.json");
    let raw = fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::loader::{load_app, LoadAppOptions};
    use tempfile::tempdir;

    #[test]
    fn persist_and_load_identifiers() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("shopify.app.toml"),
            "client_id = \"abc\"\nname = \"x\"\napplication_url = \"https://e.com\"\n",
        )
        .unwrap();
        let mut extensions = HashMap::new();
        extensions.insert("ext-a".into(), "uuid-a".into());
        persist_identifiers(
            dir.path(),
            &DeploymentIds {
                app: Some("app-1".into()),
                extensions,
                breakdown: ExtensionBreakdown::default(),
            },
        )
        .unwrap();
        let loaded = load_persisted_identifiers(dir.path()).unwrap();
        assert_eq!(loaded.get_extension("ext-a"), Some("uuid-a"));
        let _ = load_app(LoadAppOptions {
            directory: dir.path().to_path_buf(),
            config_name: None,
        });
    }
}
