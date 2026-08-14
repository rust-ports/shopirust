//! Draftable extension process (Partners path) — push drafts on rebuild.

use super::types::{DevProcess, DevProcessKind};
use crate::error::AppError;
use crate::models::extensions::ExtensionInstance;
use crate::services::dev::app_events::{AppEventWatcher, EventType, ExtensionBuildResult};
use crate::services::dev::update_extension::update_extension_draft;
use cli_api::DeveloperPlatformClient;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

pub struct DraftableExtensionOptions {
    pub api_key: String,
    pub proxy_url: String,
    pub extensions: Vec<ExtensionInstance>,
    pub remote_extension_ids: HashMap<String, String>,
    pub app_configuration: Option<Value>,
    pub client: Option<Arc<dyn DeveloperPlatformClient>>,
}

/// Partners apps without Dev Sessions use draftable extension push.
pub fn setup_draftable_extensions_process(
    opts: DraftableExtensionOptions,
    app_watcher: Arc<AppEventWatcher>,
) -> Option<DevProcess> {
    let draftable: Vec<_> = opts
        .extensions
        .iter()
        .filter(|e| e.is_draftable())
        .cloned()
        .collect();
    if draftable.is_empty() {
        return None;
    }

    Some(DevProcess::new(
        "extensions",
        DevProcessKind::DraftableExtension,
        move |ctx| run_draftable(ctx.abort, opts, draftable, app_watcher),
    ))
}

async fn run_draftable(
    abort: CancellationToken,
    opts: DraftableExtensionOptions,
    draftable: Vec<ExtensionInstance>,
    app_watcher: Arc<AppEventWatcher>,
) -> Result<(), AppError> {
    let Some(client) = opts.client.clone() else {
        tracing::warn!(
            target: "app_dev",
            "draftable extension push skipped (no developer-platform client)"
        );
        abort.cancelled().await;
        return Ok(());
    };

    let handles: Vec<String> = draftable.iter().map(|e| e.handle.clone()).collect();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let tx_start = tx.clone();
    app_watcher
        .on_start(move |event| {
            let _ = tx_start.send(event);
        })
        .await;
    app_watcher
        .on_event(move |event| {
            let _ = tx.send(event);
        })
        .await;

    let _ = opts.proxy_url;
    loop {
        tokio::select! {
            _ = abort.cancelled() => break,
            maybe = rx.recv() => {
                let Some(event) = maybe else { break };
                for ev in event.extension_events {
                    if !handles.iter().any(|h| h == &ev.extension.handle) {
                        continue;
                    }
                    if !matches!(ev.r#type, EventType::Updated | EventType::Created) {
                        // on_start events still have Created/Updated for initial build
                        if !matches!(ev.r#type, EventType::Updated | EventType::Created) {
                            continue;
                        }
                    }
                    if matches!(ev.build_result, Some(ExtensionBuildResult::Error { .. })) {
                        continue;
                    }
                    let Some(registration_id) = opts.remote_extension_ids.get(&ev.extension.handle) else {
                        tracing::warn!(
                            target: "app_dev",
                            "no remote registration id for {}",
                            ev.extension.handle
                        );
                        continue;
                    };
                    if let Err(e) = update_extension_draft(
                        &ev.extension,
                        client.as_ref(),
                        &opts.api_key,
                        registration_id,
                        opts.app_configuration.clone(),
                        &app_watcher.build_output_path,
                    )
                    .await
                    {
                        tracing::warn!(target: "app_dev", "{e}");
                    }
                }
            }
        }
    }
    Ok(())
}

/// Test helper: which handles would be drafted.
pub fn draftable_handles(extensions: &[ExtensionInstance]) -> Vec<String> {
    extensions
        .iter()
        .filter(|e| e.is_draftable())
        .map(|e| e.handle.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::extensions::create_extension_specification;
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn ext(ty: &str, handle: &str) -> ExtensionInstance {
        let spec = create_extension_specification(ty).unwrap();
        ExtensionInstance::new(
            handle,
            PathBuf::from(format!("/app/extensions/{handle}")),
            PathBuf::from(format!("/app/extensions/{handle}/shopify.extension.toml")),
            HashMap::new(),
            spec,
        )
    }

    fn watcher() -> Arc<AppEventWatcher> {
        Arc::new(AppEventWatcher::new(crate::models::loader::LoadedApp {
            directory: PathBuf::from("/tmp"),
            configuration_path: PathBuf::from("/tmp/shopify.app.toml"),
            configuration: Default::default(),
            hidden_config: Default::default(),
            extensions: vec![],
            webs: vec![],
            identifiers: crate::models::identifiers::Identifiers::new(),
            name: "t".into(),
            errors: vec![],
            dev_application_urls: None,
        }))
    }

    #[test]
    fn skips_when_no_draftable_extensions() {
        let proc = setup_draftable_extensions_process(
            DraftableExtensionOptions {
                api_key: "k".into(),
                proxy_url: "https://example.com".into(),
                extensions: vec![],
                remote_extension_ids: HashMap::new(),
                app_configuration: None,
                client: None,
            },
            watcher(),
        );
        assert!(proc.is_none());
    }

    #[test]
    fn sets_up_process_when_draftable_present() {
        let proc = setup_draftable_extensions_process(
            DraftableExtensionOptions {
                api_key: "k".into(),
                proxy_url: "https://example.com".into(),
                extensions: vec![ext("function", "discount")],
                remote_extension_ids: HashMap::from([("discount".into(), "uuid-1".into())]),
                app_configuration: None,
                client: None,
            },
            watcher(),
        );
        assert!(proc.is_some());
        assert_eq!(proc.unwrap().kind, DevProcessKind::DraftableExtension);
    }

    #[test]
    fn selects_function_theme_and_ui_as_draftable() {
        let handles = draftable_handles(&[
            ext("function", "discount"),
            ext("theme", "theme-ext"),
            ext("ui_extension", "checkout-ui"),
        ]);
        assert!(handles.contains(&"discount".into()));
        assert!(handles.contains(&"theme-ext".into()));
        assert!(handles.contains(&"checkout-ui".into()));
    }
}
