//! Extension preview server orchestrator + surface/cart helpers.

pub mod localization;
pub mod payload;
pub mod server;
pub mod templates;
pub mod utilities;
pub mod websocket;

use crate::error::AppError;
use crate::models::extensions::ExtensionInstance;
use crate::services::dev::app_events::{AppEvent, AppEventWatcher, EventType};
use payload::models::DevelopmentPayload;
use payload::store::{get_extensions_payload_store_raw_payload, ExtensionsPayloadStoreOptions};
use server::{build_extension_router, serve_extension_server, ServerState};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio_util::sync::CancellationToken;
use websocket::setup_websocket_broadcast;

pub use payload::get_ui_extension_payload;
pub use payload::store::ExtensionsPayloadStore;
pub use utilities::{build_cart_url_if_needed, get_extension_point_target_surface};

/// Options for serving UI extensions during `app dev`.
#[derive(Debug, Clone)]
pub struct ExtensionDevOptions {
    pub extensions: Vec<ExtensionInstance>,
    pub id: Option<String>,
    pub app_name: String,
    pub app_directory: PathBuf,
    pub api_key: String,
    pub url: String,
    pub port: u16,
    pub store_fqdn: String,
    pub store_id: String,
    pub granted_scopes: Vec<String>,
    pub checkout_cart_url: Option<String>,
    pub subscription_product_url: Option<String>,
    pub manifest_version: String,
    pub build_directory: Option<PathBuf>,
}

impl ExtensionDevOptions {
    pub fn websocket_url(&self) -> String {
        get_websocket_url(&self.url)
    }

    pub fn to_store_options(&self, websocket_url: String) -> ExtensionsPayloadStoreOptions {
        ExtensionsPayloadStoreOptions {
            websocket_url,
            url: self.url.clone(),
            api_key: self.api_key.clone(),
            app_name: self.app_name.clone(),
            app_id: self.id.clone(),
            store_fqdn: self.store_fqdn.clone(),
            store_id: self.store_id.clone(),
            granted_scopes: self.granted_scopes.clone(),
            checkout_cart_url: self.checkout_cart_url.clone(),
            subscription_product_url: self.subscription_product_url.clone(),
            manifest_version: self.manifest_version.clone(),
        }
    }
}

/// Convert an http(s) extension server URL into the websocket URL (`/extensions`).
pub fn get_websocket_url(url: &str) -> String {
    let mut parsed = url::Url::parse(url)
        .unwrap_or_else(|_| url::Url::parse(&format!("http://{url}")).expect("fallback url"));
    let scheme = match parsed.scheme() {
        "https" => "wss",
        _ => "ws",
    };
    let _ = parsed.set_scheme(scheme);
    parsed.set_path("/extensions");
    parsed.set_query(None);
    parsed.set_fragment(None);
    parsed.to_string()
}

/// Start the UI extension HTTP + WebSocket servers and wire hot-reload from `app_watcher`.
///
/// Runs until `cancel` is cancelled.
pub async fn dev_ui_extensions(
    mut options: ExtensionDevOptions,
    app_watcher: Arc<AppEventWatcher>,
    cancel: CancellationToken,
) -> Result<(), AppError> {
    for ext in &mut options.extensions {
        let _ = ext.ensure_dev_uuid();
    }

    let websocket_url = options.websocket_url();
    let store_options = options.to_store_options(websocket_url);
    let bundle_path = options
        .build_directory
        .clone()
        .unwrap_or_else(|| app_watcher.build_output_path.clone());

    let mut asset_resolvers: HashMap<String, HashMap<String, String>> = HashMap::new();
    let raw = get_extensions_payload_store_raw_payload(
        &store_options,
        &options.extensions,
        &bundle_path,
        &mut asset_resolvers,
    )?;
    let store = Arc::new(Mutex::new(ExtensionsPayloadStore::new(
        raw,
        store_options,
        asset_resolvers,
    )));

    let extensions: Arc<Mutex<Vec<ExtensionInstance>>> = Arc::new(Mutex::new(
        options
            .extensions
            .iter()
            .filter(|e| e.is_previewable())
            .cloned()
            .collect(),
    ));

    let update_tx = setup_websocket_broadcast(store.clone(), options.manifest_version.clone());

    {
        let mut s = store.lock().unwrap();
        let tx = update_tx.clone();
        s.on_update(move |ids| {
            let _ = tx.send(ids);
        });
    }

    let store_for_events = store.clone();
    let extensions_for_events = extensions.clone();
    let opts_for_events = options.clone();
    let bundle_for_events = bundle_path.clone();

    app_watcher
        .on_event(move |event: AppEvent| {
            handle_app_event(
                &event,
                store_for_events.clone(),
                extensions_for_events.clone(),
                &opts_for_events,
                &bundle_for_events,
            );
        })
        .await;

    let store_start = store.clone();
    let opts_start = options.clone();
    let bundle_start = bundle_path.clone();
    app_watcher
        .on_start(move |event: AppEvent| {
            let mut s = store_start.lock().unwrap();
            for ext_event in &event.extension_events {
                if !ext_event.extension.is_previewable() {
                    continue;
                }
                let development = development_from_build(&ext_event.build_result);
                let _ = s.update_extension(
                    &ext_event.extension,
                    &opts_start.to_store_options(opts_start.websocket_url()),
                    &bundle_start,
                    development,
                );
            }
        })
        .await;

    let state = ServerState {
        manifest_version: options.manifest_version.clone(),
        options,
        payload_store: store,
        extensions,
        bundle_path,
        update_tx,
    };
    let _ = build_extension_router(state.clone());
    serve_extension_server(state.options.port, state, cancel).await
}

fn development_from_build(
    result: &Option<crate::services::dev::app_events::ExtensionBuildResult>,
) -> Option<DevelopmentPayload> {
    match result {
        Some(crate::services::dev::app_events::ExtensionBuildResult::Ok { .. }) => {
            Some(DevelopmentPayload {
                status: "success".into(),
                hidden: None,
                error: None,
                localization_status: None,
            })
        }
        Some(crate::services::dev::app_events::ExtensionBuildResult::Error {
            error, file, ..
        }) => Some(DevelopmentPayload {
            status: "error".into(),
            hidden: None,
            error: Some(payload::models::DevelopmentError {
                message: error.clone(),
                file: file.clone(),
            }),
            localization_status: None,
        }),
        None => Some(DevelopmentPayload {
            status: "success".into(),
            hidden: None,
            error: None,
            localization_status: None,
        }),
    }
}

fn handle_app_event(
    event: &AppEvent,
    store: Arc<Mutex<ExtensionsPayloadStore>>,
    extensions: Arc<Mutex<Vec<ExtensionInstance>>>,
    options: &ExtensionDevOptions,
    bundle_path: &std::path::Path,
) {
    if event.app_was_reloaded {
        let previewable: Vec<_> = event
            .app
            .extensions
            .iter()
            .filter(|e| e.is_previewable())
            .cloned()
            .collect();
        *extensions.lock().unwrap() = previewable;
    }

    let store_opts = options.to_store_options(options.websocket_url());
    let mut s = store.lock().unwrap();

    for ext_event in &event.extension_events {
        match ext_event.r#type {
            EventType::Deleted => {
                s.delete_extension(&ext_event.extension);
                let mut list = extensions.lock().unwrap();
                list.retain(|e| e.dev_uuid != ext_event.extension.dev_uuid);
            }
            EventType::Created => {
                if ext_event.extension.is_previewable() {
                    let _ = s.add_extension(&ext_event.extension, bundle_path);
                    extensions.lock().unwrap().push(ext_event.extension.clone());
                }
            }
            EventType::Updated => {
                if ext_event.extension.is_previewable() {
                    let development = development_from_build(&ext_event.build_result);
                    let _ = s.update_extension(
                        &ext_event.extension,
                        &store_opts,
                        bundle_path,
                        development,
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn websocket_url_https_to_wss() {
        assert_eq!(
            get_websocket_url("https://example.com"),
            "wss://example.com/extensions"
        );
    }

    #[test]
    fn websocket_url_http_to_ws() {
        assert_eq!(
            get_websocket_url("http://localhost:9293"),
            "ws://localhost:9293/extensions"
        );
    }
}
