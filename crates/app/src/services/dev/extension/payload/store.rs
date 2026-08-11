//! Payload store + raw payload builder.

use super::models::{
    AppPayload, ConnectedPayload, ExtensionsEndpointPayload, UIExtensionPayload, UrlHolder,
};
use super::get_ui_extension_payload;
use crate::error::AppError;
use crate::models::extensions::ExtensionInstance;
use crate::services::dev::extension::utilities::{build_app_url_for_mobile, build_app_url_for_web};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

/// URL subpath → output-relative filesystem path.
pub type AssetResolver = HashMap<String, String>;

#[derive(Debug, Clone)]
pub struct ExtensionsPayloadStoreOptions {
    pub websocket_url: String,
    pub url: String,
    pub api_key: String,
    pub app_name: String,
    pub app_id: Option<String>,
    pub store_fqdn: String,
    pub store_id: String,
    pub granted_scopes: Vec<String>,
    pub checkout_cart_url: Option<String>,
    pub subscription_product_url: Option<String>,
    pub manifest_version: String,
}

pub type UpdateCallback = Arc<dyn Fn(Vec<String>) + Send + Sync>;

pub struct ExtensionsPayloadStore {
    options: ExtensionsPayloadStoreOptions,
    raw_payload: ExtensionsEndpointPayload,
    asset_resolvers: HashMap<String, AssetResolver>,
    on_update: Option<UpdateCallback>,
}

impl ExtensionsPayloadStore {
    pub fn new(
        raw_payload: ExtensionsEndpointPayload,
        options: ExtensionsPayloadStoreOptions,
        asset_resolvers: HashMap<String, AssetResolver>,
    ) -> Self {
        Self {
            options,
            raw_payload,
            asset_resolvers,
            on_update: None,
        }
    }

    pub fn on_update<F>(&mut self, cb: F)
    where
        F: Fn(Vec<String>) + Send + Sync + 'static,
    {
        self.on_update = Some(Arc::new(cb));
    }

    pub fn get_asset_resolver(&self, dev_uuid: &str) -> Option<&AssetResolver> {
        self.asset_resolvers.get(dev_uuid)
    }

    pub fn get_connected_payload(&self) -> ConnectedPayload {
        let raw = self.get_raw_payload();
        ConnectedPayload {
            app: raw.app.clone(),
            app_id: raw.app_id.clone(),
            store: raw.store.clone(),
            extensions: raw.extensions.clone(),
        }
    }

    pub fn get_raw_payload(&self) -> &ExtensionsEndpointPayload {
        &self.raw_payload
    }

    pub fn get_raw_payload_filtered_by_extension_ids(
        &self,
        extension_ids: &[String],
    ) -> ExtensionsEndpointPayload {
        let mut filtered = self.raw_payload.clone();
        filtered.extensions.retain(|ext| extension_ids.contains(&ext.uuid));
        filtered
    }

    pub fn update_app(&mut self, app: Value) {
        if let Some(obj) = app.as_object() {
            if let Some(v) = obj.get("apiKey").and_then(|v| v.as_str()) {
                self.raw_payload.app.api_key = v.to_string();
            }
            if let Some(v) = obj.get("url").and_then(|v| v.as_str()) {
                self.raw_payload.app.url = v.to_string();
            }
            if let Some(v) = obj.get("mobileUrl").and_then(|v| v.as_str()) {
                self.raw_payload.app.mobile_url = v.to_string();
            }
            if let Some(v) = obj.get("title").and_then(|v| v.as_str()) {
                self.raw_payload.app.title = v.to_string();
            }
        }
        self.emit_update(vec![]);
    }

    pub fn update_extensions(&mut self, extensions: Vec<UIExtensionPayload>) {
        let ids: Vec<String> = extensions.iter().map(|e| e.uuid.clone()).collect();
        for found in extensions {
            if let Some(existing) = self
                .raw_payload
                .extensions
                .iter_mut()
                .find(|e| e.uuid == found.uuid)
            {
                *existing = merge_extension_payload(existing, &found);
            }
        }
        self.emit_update(ids);
    }

    pub fn update_extension(
        &mut self,
        extension: &ExtensionInstance,
        options: &ExtensionsPayloadStoreOptions,
        bundle_path: &Path,
        development: Option<super::models::DevelopmentPayload>,
    ) -> Result<(), AppError> {
        let uuid = extension
            .dev_uuid
            .as_deref()
            .ok_or_else(|| AppError::message("extension missing dev_uuid"))?;
        let index = self
            .raw_payload
            .extensions
            .iter()
            .position(|e| e.uuid == uuid);
        let Some(index) = index else {
            return Ok(());
        };

        let current_status = self.raw_payload.extensions[index]
            .development
            .status
            .clone();
        let current_localization = self.raw_payload.extensions[index].localization.clone();
        let resolver = get_or_create_resolver(&mut self.asset_resolvers, uuid);
        let payload = get_ui_extension_payload(
            extension,
            bundle_path,
            options,
            Some(resolver),
            development.or(Some(super::models::DevelopmentPayload {
                status: current_status,
                ..Default::default()
            })),
            current_localization,
        )?;
        self.raw_payload.extensions[index] = payload;
        self.emit_update(vec![uuid.to_string()]);
        Ok(())
    }

    pub fn delete_extension(&mut self, extension: &ExtensionInstance) {
        let Some(uuid) = extension.dev_uuid.as_deref() else {
            return;
        };
        let before = self.raw_payload.extensions.len();
        self.raw_payload.extensions.retain(|e| e.uuid != uuid);
        if self.raw_payload.extensions.len() != before {
            self.asset_resolvers.remove(uuid);
            self.emit_update(vec![uuid.to_string()]);
        }
    }

    pub fn add_extension(
        &mut self,
        extension: &ExtensionInstance,
        bundle_path: &Path,
    ) -> Result<(), AppError> {
        let uuid = extension
            .dev_uuid
            .as_deref()
            .ok_or_else(|| AppError::message("extension missing dev_uuid"))?;
        let resolver = get_or_create_resolver(&mut self.asset_resolvers, uuid);
        let payload =
            get_ui_extension_payload(extension, bundle_path, &self.options, Some(resolver), None, None)?;
        self.raw_payload.extensions.push(payload);
        self.emit_update(vec![uuid.to_string()]);
        Ok(())
    }

    fn emit_update(&self, extension_ids: Vec<String>) {
        if let Some(cb) = &self.on_update {
            cb(extension_ids);
        }
    }
}

pub fn get_extensions_payload_store_raw_payload(
    options: &ExtensionsPayloadStoreOptions,
    extensions: &[ExtensionInstance],
    bundle_path: &Path,
    resolvers: &mut HashMap<String, AssetResolver>,
) -> Result<ExtensionsEndpointPayload, AppError> {
    let mut payloads = Vec::new();
    for ext in extensions.iter().filter(|e| e.is_previewable()) {
        let uuid = ext
            .dev_uuid
            .as_deref()
            .ok_or_else(|| AppError::message("previewable extension missing dev_uuid"))?;
        let resolver = get_or_create_resolver(resolvers, uuid);
        payloads.push(get_ui_extension_payload(
            ext, bundle_path, options, Some(resolver), None, None,
        )?);
    }

    Ok(ExtensionsEndpointPayload {
        app: AppPayload {
            title: options.app_name.clone(),
            api_key: options.api_key.clone(),
            url: build_app_url_for_web(&options.store_fqdn, &options.api_key),
            mobile_url: build_app_url_for_mobile(&options.store_fqdn, &options.api_key),
        },
        app_id: options.app_id.clone(),
        version: options.manifest_version.clone(),
        root: UrlHolder {
            url: join_url(&options.url, "/extensions"),
        },
        socket: UrlHolder {
            url: options.websocket_url.clone(),
        },
        dev_console: UrlHolder {
            url: join_url(&options.url, "/extensions/dev-console"),
        },
        store: options.store_fqdn.clone(),
        extensions: payloads,
    })
}

fn get_or_create_resolver<'a>(
    resolvers: &'a mut HashMap<String, AssetResolver>,
    dev_uuid: &str,
) -> &'a mut AssetResolver {
    resolvers.entry(dev_uuid.to_string()).or_default()
}

fn join_url(base: &str, path: &str) -> String {
    let base = base.trim_end_matches('/');
    format!("{base}{path}")
}

fn merge_extension_payload(
    existing: &UIExtensionPayload,
    found: &UIExtensionPayload,
) -> UIExtensionPayload {
    // Prefer a JSON deep-merge so extensionPoints merge by target when both are arrays of objects.
    let mut dest = serde_json::to_value(existing).unwrap_or(Value::Null);
    let src = serde_json::to_value(found).unwrap_or(Value::Null);
    deep_merge(&mut dest, &src);
    serde_json::from_value(dest).unwrap_or_else(|_| found.clone())
}

fn deep_merge(dest: &mut Value, src: &Value) {
    match (dest, src) {
        (Value::Object(d), Value::Object(s)) => {
            for (k, v) in s {
                if k == "extensionPoints" {
                    if let (Some(Value::Array(dest_arr)), Value::Array(src_arr)) =
                        (d.get_mut(k), v)
                    {
                        merge_extension_points(dest_arr, src_arr);
                        continue;
                    }
                }
                deep_merge(d.entry(k.clone()).or_insert(Value::Null), v);
            }
        }
        (d, s) => *d = s.clone(),
    }
}

fn merge_extension_points(dest: &mut Vec<Value>, src: &[Value]) {
    for src_point in src {
        let Some(target) = src_point.get("target").and_then(|t| t.as_str()) else {
            continue;
        };
        if let Some(existing) = dest
            .iter_mut()
            .find(|p| p.get("target").and_then(|t| t.as_str()) == Some(target))
        {
            // Arrays from source replace destination arrays (upstream deepMergeObjects behavior).
            if let (Value::Object(d), Value::Object(s)) = (existing, src_point) {
                for (k, v) in s {
                    if v.is_array() {
                        d.insert(k.clone(), v.clone());
                    } else {
                        deep_merge(d.entry(k.clone()).or_insert(Value::Null), v);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::extensions::create_extension_specification;
    use crate::models::extensions::ExtensionInstance;
    use crate::services::dev::extension::payload::models::{
        Asset, DevelopmentState, MainAssets, OptionalUrlHolder, SupportedFeatures,
    };
    use std::path::PathBuf;
    use tempfile::tempdir;

    fn sample_options() -> ExtensionsPayloadStoreOptions {
        ExtensionsPayloadStoreOptions {
            websocket_url: "ws://localhost:9293/extensions".into(),
            url: "http://localhost:9293".into(),
            api_key: "api-key".into(),
            app_name: "Test App".into(),
            app_id: Some("app-id".into()),
            store_fqdn: "shop.myshopify.com".into(),
            store_id: "1".into(),
            granted_scopes: vec!["read_products".into()],
            checkout_cart_url: None,
            subscription_product_url: None,
            manifest_version: "3".into(),
        }
    }

    fn ui_ext(handle: &str, uuid: &str) -> ExtensionInstance {
        let spec = create_extension_specification("ui_extension").unwrap();
        let mut ext = ExtensionInstance::new(
            handle,
            PathBuf::from(format!("extensions/{handle}")),
            PathBuf::from(format!("extensions/{handle}/shopify.extension.toml")),
            HashMap::new(),
            spec,
        );
        ext.dev_uuid = Some(uuid.into());
        ext.uid = Some(uuid.trim_start_matches("dev-").into());
        ext
    }

    fn sample_payload(uuid: &str, handle: &str) -> UIExtensionPayload {
        UIExtensionPayload {
            assets: MainAssets {
                main: Asset::new("main", format!("http://localhost/extensions/{uuid}/assets/{handle}.js"), 1),
            },
            supported_features: Some(SupportedFeatures { runs_offline: false }),
            capabilities: None,
            development: DevelopmentState {
                resource: OptionalUrlHolder { url: None },
                root: UrlHolder {
                    url: format!("http://localhost/extensions/{uuid}"),
                },
                hidden: false,
                status: "success".into(),
                localization_status: "".into(),
                error: None,
            },
            extension_points: Value::Null,
            localization: None,
            metafields: None,
            type_name: "ui_extension".into(),
            external_type: "ui_extension".into(),
            api_version: None,
            uuid: uuid.into(),
            version: None,
            surface: "admin".into(),
            title: handle.into(),
            handle: handle.into(),
            name: handle.into(),
            description: None,
            approval_scopes: vec![],
            settings: None,
        }
    }

    #[test]
    fn raw_connected_and_filtered_payloads() {
        let opts = sample_options();
        let dir = tempdir().unwrap();
        let mut resolvers = HashMap::new();
        let mut ext = ui_ext("my-ext", "dev-1");
        let _ = ext.ensure_dev_uuid();
        let raw =
            get_extensions_payload_store_raw_payload(&opts, &[ext], dir.path(), &mut resolvers)
                .unwrap();
        let store = ExtensionsPayloadStore::new(raw.clone(), opts, resolvers);

        assert_eq!(store.get_raw_payload().store, "shop.myshopify.com");
        assert_eq!(store.get_connected_payload().store, "shop.myshopify.com");
        assert!(store
            .get_raw_payload_filtered_by_extension_ids(&["missing".into()])
            .extensions
            .is_empty());
    }

    #[test]
    fn update_app_merges_and_emits() {
        let opts = sample_options();
        let mut raw = get_extensions_payload_store_raw_payload(
            &opts,
            &[],
            Path::new("/tmp"),
            &mut HashMap::new(),
        )
        .unwrap();
        raw.extensions.push(sample_payload("dev-1", "a"));
        let mut store = ExtensionsPayloadStore::new(raw, opts, HashMap::new());
        let emitted = Arc::new(std::sync::Mutex::new(None));
        let emitted2 = emitted.clone();
        store.on_update(move |ids| {
            *emitted2.lock().unwrap() = Some(ids);
        });
        store.update_app(serde_json::json!({"title": "Renamed"}));
        assert_eq!(store.get_raw_payload().app.title, "Renamed");
        assert_eq!(*emitted.lock().unwrap(), Some(vec![]));
    }

    #[test]
    fn delete_extension_drops_resolver() {
        let opts = sample_options();
        let mut resolvers = HashMap::new();
        resolvers.insert("dev-1".into(), {
            let mut m = HashMap::new();
            m.insert("main.js".into(), "main.js".into());
            m
        });
        let mut raw = get_extensions_payload_store_raw_payload(
            &opts,
            &[],
            Path::new("/tmp"),
            &mut HashMap::new(),
        )
        .unwrap();
        raw.extensions.push(sample_payload("dev-1", "a"));
        let mut store = ExtensionsPayloadStore::new(raw, opts, resolvers);
        let ext = ui_ext("a", "dev-1");
        store.delete_extension(&ext);
        assert!(store.get_raw_payload().extensions.is_empty());
        assert!(store.get_asset_resolver("dev-1").is_none());
    }

    #[test]
    fn update_extensions_merges_by_uuid() {
        let opts = sample_options();
        let mut raw = get_extensions_payload_store_raw_payload(
            &opts,
            &[],
            Path::new("/tmp"),
            &mut HashMap::new(),
        )
        .unwrap();
        raw.extensions.push(sample_payload("dev-1", "a"));
        let mut store = ExtensionsPayloadStore::new(raw, opts, HashMap::new());
        let mut updated = sample_payload("dev-1", "a");
        updated.development.hidden = true;
        store.update_extensions(vec![updated]);
        assert!(store.get_raw_payload().extensions[0].development.hidden);
    }
}
