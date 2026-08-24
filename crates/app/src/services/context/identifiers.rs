use crate::error::AppError;
use crate::local_storage::{set_cached_app_info, CachedAppInfo};
use crate::models::identifiers::Identifiers;
use crate::prompts::Prompter;
use crate::services::context::breakdown_extensions::{
    extensions_identifiers_deploy_breakdown, ExtensionBreakdown,
};
use crate::services::context::id_manual_matching::manual_match_ids;
use crate::services::context::id_matching::{automatic_matchmaking, LocalSource, RemoteSource};
use crate::services::context::LinkedAppContext;
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
    prompter: Option<&dyn Prompter>,
) -> Result<DeploymentIds, AppError> {
    let mut breakdown = extensions_identifiers_deploy_breakdown(ctx, client).await?;
    let mut extensions = HashMap::new();

    for (handle, uuid) in &breakdown.matched {
        extensions.insert(handle.clone(), uuid.clone());
    }

    // Manual matching for leftovers (same-type local/remote that automatic matching couldn't pair).
    let local_pending: Vec<LocalSource> = ctx
        .app
        .extensions
        .iter()
        .filter(|e| breakdown.to_create.iter().any(|h| h == &e.handle))
        .map(|e| LocalSource {
            local_identifier: e.handle.clone(),
            handle: e.handle.clone(),
            graph_ql_type: e.type_name().to_string(),
            external_type: e.type_name().to_string(),
            type_name: e.type_name().to_string(),
            uid: e.uid.clone(),
        })
        .collect();
    let remote_pending: Vec<RemoteSource> = breakdown
        .only_remote
        .iter()
        .map(|title| RemoteSource {
            uuid: title.clone(),
            id: String::new(),
            title: title.clone(),
            type_name: String::new(),
        })
        .collect();

    if !local_pending.is_empty() && !remote_pending.is_empty() {
        // Prefer automatic matchmaking leftovers, then prompt.
        let auto = automatic_matchmaking(&local_pending, &remote_pending, &extensions, false);
        extensions.extend(auto.identifiers);
        let manual = manual_match_ids(
            &auto.to_manual_match.local,
            &auto.to_manual_match.remote,
            prompter,
        )?;
        extensions.extend(manual.identifiers);
        breakdown.to_create = manual.to_create.iter().map(|l| l.handle.clone()).collect();
        breakdown.only_remote = manual.only_remote.iter().map(|r| r.title.clone()).collect();
        for (handle, uuid) in &extensions {
            breakdown.matched.insert(handle.clone(), uuid.clone());
        }
    }

    let atomic = client.supports_atomic_deployments();
    for handle in &breakdown.to_create {
        if atomic {
            // App Management assigns UUIDs on deploy; do not invent pending: handles.
            if let Some(ext) = ctx.app.extensions.iter().find(|e| &e.handle == handle) {
                if let Some(ref uid) = ext.uid {
                    extensions
                        .entry(handle.clone())
                        .or_insert_with(|| uid.clone());
                }
            }
        } else if let Some(ext) = ctx.app.extensions.iter().find(|e| &e.handle == handle) {
            let created = client
                .create_extension(&cli_api::ExtensionCreateInput {
                    api_key: ctx.remote_app.api_key.clone(),
                    type_name: client.to_extension_graphql_type(ext.type_name()),
                    title: ext.handle.clone(),
                    config: "{}".into(),
                    context: None,
                    handle: ext.handle.clone(),
                })
                .await
                .map_err(|e| AppError::message(e.to_string()))?;
            extensions.entry(handle.clone()).or_insert(created.uuid);
        }
    }
    for ext in &ctx.app.extensions {
        if let Some(ref uid) = ext.uid {
            extensions
                .entry(ext.handle.clone())
                .or_insert_with(|| uid.clone());
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
        ..Default::default()
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
            ignore_unknown_extensions: false,
        });
    }

    fn write_theme_ext(dir: &std::path::Path, handle: &str) {
        let ext_dir = dir.join("extensions").join(handle);
        fs::create_dir_all(&ext_dir).unwrap();
        fs::write(
            ext_dir.join("shopify.extension.toml"),
            format!("type = \"theme\"\nhandle = \"{handle}\"\nname = \"{handle}\"\n"),
        )
        .unwrap();
    }

    #[tokio::test]
    async fn atomic_deploy_does_not_invent_pending_ids() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("shopify.app.toml"),
            "client_id = \"key-1\"\nname = \"Demo\"\napplication_url = \"https://example.com\"\n",
        )
        .unwrap();
        write_theme_ext(dir.path(), "brand-new");
        let client =
            crate::test_support::MockClient::with_app(crate::test_support::sample_org_app("key-1"));
        let ctx = crate::services::context::linked_app_context(
            crate::services::context::LinkedAppContextOptions {
                directory: dir.path().to_path_buf(),
                config_name: None,
                client_id: None,
                force_relink: false,
            },
            &client,
            None,
        )
        .await
        .unwrap();
        let ids = ensure_deployment_ids_presence(
            &ctx,
            &client,
            EnsureIdsOptions {
                allow_updates: true,
            },
            None,
        )
        .await
        .unwrap();
        assert!(
            !ids.extensions.values().any(|v| v.starts_with("pending:")),
            "AM atomic deploy must not invent pending: UUIDs, got {:?}",
            ids.extensions
        );
        assert!(ids.breakdown.to_create.contains(&"brand-new".to_string()));
    }

    #[tokio::test]
    async fn partners_creates_extension_ids() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("shopify.app.toml"),
            "client_id = \"key-1\"\nname = \"Demo\"\napplication_url = \"https://example.com\"\n",
        )
        .unwrap();
        write_theme_ext(dir.path(), "brand-new");
        let mut client =
            crate::test_support::MockClient::with_app(crate::test_support::sample_org_app("key-1"));
        client.atomic = false;
        let ctx = crate::services::context::linked_app_context(
            crate::services::context::LinkedAppContextOptions {
                directory: dir.path().to_path_buf(),
                config_name: None,
                client_id: None,
                force_relink: false,
            },
            &client,
            None,
        )
        .await
        .unwrap();
        let ids = ensure_deployment_ids_presence(
            &ctx,
            &client,
            EnsureIdsOptions {
                allow_updates: true,
            },
            None,
        )
        .await
        .unwrap();
        assert_eq!(
            ids.extensions.get("brand-new").map(String::as_str),
            Some("created:brand-new")
        );
        assert_eq!(client.created_extensions.lock().unwrap().len(), 1);
    }

    #[test]
    fn match_by_handle_and_uuid() {
        use crate::services::context::id_matching::{
            automatic_matchmaking, LocalSource, RemoteSource,
        };
        let local = vec![LocalSource {
            local_identifier: "offsite".into(),
            handle: "offsite".into(),
            graph_ql_type: "payments_extension".into(),
            external_type: "payments_extension".into(),
            type_name: "payments_extension".into(),
            uid: Some("uid-1".into()),
        }];
        let remote = vec![RemoteSource {
            uuid: "uid-1".into(),
            id: "uid-1".into(),
            title: "Offsite".into(),
            type_name: "payments_extension".into(),
        }];
        let matched = automatic_matchmaking(&local, &remote, &HashMap::new(), true);
        assert_eq!(
            matched.identifiers.get("offsite").map(String::as_str),
            Some("uid-1")
        );
        assert!(matched.to_create.is_empty());
    }
}
