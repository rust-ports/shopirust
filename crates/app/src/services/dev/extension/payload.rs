//! UI extension payload generation.

pub mod models;
pub mod store;

use crate::error::AppError;
use crate::models::extensions::ExtensionInstance;
use crate::services::dev::extension::get_extension_point_target_surface;
use models::{
    Asset, DevelopmentState, MainAssets, OptionalUrlHolder, SupportedFeatures, UIExtensionPayload,
    UrlHolder,
};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;
use store::{AssetResolver, ExtensionsPayloadStoreOptions};

pub use models::{
    AppPayload, Asset as PayloadAsset, ConnectedPayload, DevNewExtensionPoint, DevelopmentError,
    DevelopmentPayload, DevelopmentState as PayloadDevelopmentState, ExtensionsEndpointPayload,
    MainAssets as PayloadMainAssets, OptionalUrlHolder as PayloadOptionalUrlHolder,
    SupportedFeatures as PayloadSupportedFeatures, UIExtensionPayload as PayloadUIExtension,
    UrlHolder as PayloadUrlHolder,
};
pub use store::{
    get_extensions_payload_store_raw_payload, AssetResolver as PayloadAssetResolver,
    ExtensionsPayloadStore, ExtensionsPayloadStoreOptions as PayloadStoreOptions,
};

/// Options threaded into payload generation (development overrides).
pub fn get_ui_extension_payload(
    extension: &ExtensionInstance,
    bundle_path: &Path,
    options: &ExtensionsPayloadStoreOptions,
    resolver: Option<&mut AssetResolver>,
    current_development: Option<models::DevelopmentPayload>,
    current_localization: Option<Value>,
) -> Result<UIExtensionPayload, AppError> {
    let mut resolver = resolver;
    if let Some(r) = resolver.as_mut() {
        r.clear();
    }

    let uuid = extension
        .dev_uuid
        .as_deref()
        .ok_or_else(|| AppError::message("extension missing dev_uuid"))?;
    let extension_output_path = extension.get_output_path_for_directory(bundle_path);
    let url = format!("{}/extensions/{uuid}", options.url.trim_end_matches('/'));

    // If custom relative output, build dir is parent; else output path itself (dir or file).
    let build_directory = if extension.output_path.is_some() {
        extension_output_path
            .parent()
            .unwrap_or(extension_output_path.as_path())
            .to_path_buf()
    } else if extension_output_path.is_file() || extension_output_path.extension().is_some() {
        extension_output_path
            .parent()
            .unwrap_or(&extension_output_path)
            .to_path_buf()
    } else {
        extension_output_path.clone()
    };

    let extension_points =
        get_extension_points(extension, &url, &build_directory, resolver.as_deref_mut())?;

    let main_asset = if is_new_extension_points_schema(&extension_points) {
        extension_points
            .as_array()
            .and_then(|arr| arr.first())
            .and_then(|ep| ep.get("assets"))
            .and_then(|a| a.get("main"))
            .and_then(|m| serde_json::from_value::<Asset>(m.clone()).ok())
            .unwrap_or_else(|| {
                Asset::new(
                    "main",
                    format!("{url}/assets/{}", extension.output_file_name()),
                    file_mtime_ms(&extension_output_path),
                )
            })
    } else {
        Asset::new(
            "main",
            format!("{url}/assets/{}", extension.output_file_name()),
            file_mtime_ms(&extension_output_path),
        )
    };

    if let Some(r) = resolver.as_mut() {
        // Ensure main asset is resolvable even without a manifest.
        let key = extension.output_file_name();
        if !r.contains_key(&key) {
            r.insert(key.clone(), key);
        }
    }

    let metafields = extension
        .configuration
        .get("metafields")
        .filter(|v| v.as_array().is_some_and(|a| !a.is_empty()))
        .cloned();

    let runs_offline = extension
        .configuration
        .get("supported_features")
        .and_then(|v| v.get("runs_offline"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let capabilities = extension
        .configuration
        .get("capabilities")
        .cloned()
        .or_else(|| {
            Some(json!({
                "blockProgress": false,
                "networkAccess": false,
                "apiAccess": false,
                "collectBuyerConsent": {
                    "smsMarketing": false,
                    "customerPrivacy": false,
                },
                "iframe": { "sources": [] },
            }))
        });

    let status = current_development
        .as_ref()
        .map(|d| d.status.clone())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "success".into());
    let hidden = current_development
        .as_ref()
        .and_then(|d| d.hidden)
        .unwrap_or(false);
    let localization_status = current_development
        .as_ref()
        .and_then(|d| d.localization_status.clone())
        .unwrap_or_default();
    let error = current_development.and_then(|d| d.error);

    Ok(UIExtensionPayload {
        assets: MainAssets { main: main_asset },
        supported_features: Some(SupportedFeatures { runs_offline }),
        capabilities,
        development: DevelopmentState {
            resource: OptionalUrlHolder {
                url: resource_url(extension.type_name(), options),
            },
            root: UrlHolder { url: url.clone() },
            hidden,
            status,
            localization_status,
            error,
        },
        extension_points,
        localization: current_localization.or_else(|| {
            crate::services::dev::extension::localization::get_localization(extension, None)
                .ok()
                .and_then(|(loc, _)| loc)
        }),
        metafields,
        type_name: extension.type_name().to_string(),
        external_type: extension.external_type().to_string(),
        api_version: extension.api_version().map(str::to_string),
        uuid: uuid.to_string(),
        version: None,
        surface: extension.surface().to_string(),
        title: extension.name(),
        handle: extension.handle.clone(),
        name: extension.name(),
        description: extension
            .configuration
            .get("description")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        approval_scopes: options.granted_scopes.clone(),
        settings: extension.configuration.get("settings").cloned(),
    })
}

pub fn is_new_extension_points_schema(extension_points: &Value) -> bool {
    match extension_points {
        Value::Array(arr) => arr.iter().all(|ep| ep.is_object()),
        _ => false,
    }
}

fn get_extension_points(
    extension: &ExtensionInstance,
    url: &str,
    build_directory: &Path,
    mut resolver: Option<&mut AssetResolver>,
) -> Result<Value, AppError> {
    let mut points = extension
        .configuration
        .get("extension_points")
        .or_else(|| extension.configuration.get("targeting"))
        .cloned()
        .unwrap_or(Value::Null);

    if extension.type_name() == "checkout_post_purchase" {
        points = json!([{ "target": "purchase.post.render" }]);
    }

    if !is_new_extension_points_schema(&points) {
        return Ok(points);
    }

    let manifest = read_bundle_manifest(build_directory)?;
    let arr = points.as_array().cloned().unwrap_or_default();
    let mut result = Vec::new();

    for mut ep in arr {
        let target = ep
            .get("target")
            .and_then(|t| t.as_str())
            .unwrap_or("")
            .to_string();
        let surface = get_extension_point_target_surface(&target);
        if let Some(obj) = ep.as_object_mut() {
            obj.insert("surface".into(), json!(surface));
            obj.insert("root".into(), json!({ "url": format!("{url}/{target}") }));
            if !obj.contains_key("resource") {
                obj.insert("resource".into(), json!({ "url": "" }));
            }
        }

        if let Some(manifest_entry) = manifest.as_ref().and_then(|m| m.get(&target)) {
            let mapped = map_manifest_assets(
                manifest_entry,
                &target,
                &ep,
                url,
                extension,
                build_directory,
                resolver.as_deref_mut(),
            )?;
            if let (Value::Object(dest), Value::Object(src)) = (&mut ep, mapped) {
                for (k, v) in src {
                    if k == "assets" {
                        let existing = dest.entry("assets").or_insert_with(|| json!({}));
                        if let (Value::Object(d), Value::Object(s)) = (existing, &v) {
                            for (ak, av) in s {
                                d.insert(ak.clone(), av.clone());
                            }
                        }
                    } else {
                        dest.insert(k, v);
                    }
                }
            }
        }
        result.push(ep);
    }

    Ok(Value::Array(result))
}

fn read_bundle_manifest(
    build_directory: &Path,
) -> Result<Option<HashMap<String, Value>>, AppError> {
    let path = build_directory.join("manifest.json");
    if !path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(&path)?;
    let parsed: HashMap<String, Value> = serde_json::from_str(&content).map_err(|e| {
        AppError::message(format!(
            "Invalid manifest.json in {}: {e}",
            build_directory.display()
        ))
    })?;
    Ok(Some(parsed))
}

fn map_manifest_assets(
    manifest_entry: &Value,
    target: &str,
    extension_point: &Value,
    url: &str,
    extension: &ExtensionInstance,
    build_directory: &Path,
    mut resolver: Option<&mut AssetResolver>,
) -> Result<Value, AppError> {
    let Some(obj) = manifest_entry.as_object() else {
        return Ok(json!({}));
    };
    let mut assets: HashMap<String, Asset> = HashMap::new();

    for (identifier, value) in obj {
        if identifier == "intents" {
            // Intents left as-is for now; schema assets registered when present as strings.
            continue;
        }
        if (identifier == "main" || identifier == "should_render") && value.is_string() {
            let filepath = value.as_str().unwrap();
            let asset = get_asset_payload(
                identifier,
                &format!("{target}/{identifier}"),
                filepath,
                url,
                extension,
                resolver.as_deref_mut(),
                None,
            )?;
            assets.insert(identifier.clone(), asset);
            continue;
        }
        if identifier == "assets" {
            if let Some(files) = value.as_array() {
                let paths: Vec<&str> = files.iter().filter_map(|v| v.as_str()).collect();
                if let Some(r) = resolver.as_mut() {
                    for file in &paths {
                        r.insert(format!("{target}/{file}"), (*file).to_string());
                    }
                }
                let last = paths
                    .iter()
                    .map(|f| file_mtime_ms(&build_directory.join(f)))
                    .max()
                    .unwrap_or(0);
                assets.insert(
                    identifier.clone(),
                    Asset::new(identifier.clone(), format!("{url}/assets/{target}/"), last),
                );
            }
            continue;
        }
        // default mapper
        let filepath = if let Some(s) = value.as_str() {
            s.to_string()
        } else if let Some(s) = extension_point.get(identifier).and_then(|v| v.as_str()) {
            s.to_string()
        } else {
            continue;
        };
        let asset = get_asset_payload(
            identifier,
            &format!("{target}/{identifier}"),
            &filepath,
            url,
            extension,
            resolver.as_deref_mut(),
            extension_point
                .get(identifier)
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .filter(|_| value.is_string()),
        )?;
        assets.insert(identifier.clone(), asset);
    }

    Ok(json!({ "assets": assets }))
}

fn get_asset_payload(
    name: &str,
    url_subpath: &str,
    filepath: &str,
    url: &str,
    extension: &ExtensionInstance,
    resolver: Option<&mut AssetResolver>,
    source_path: Option<String>,
) -> Result<Asset, AppError> {
    let ext = Path::new(filepath)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| format!(".{e}"))
        .unwrap_or_default();
    let url_subpath_with_ext = format!("{url_subpath}{ext}");
    if let Some(r) = resolver {
        r.insert(url_subpath_with_ext.clone(), filepath.to_string());
    }
    let mtime_path = source_path
        .map(|s| extension.directory.join(s))
        .unwrap_or_else(|| extension.directory.join(filepath));
    Ok(Asset::new(
        name,
        format!("{url}/assets/{url_subpath_with_ext}"),
        file_mtime_ms(&mtime_path),
    ))
}

fn resource_url(ext_type: &str, options: &ExtensionsPayloadStoreOptions) -> Option<String> {
    match ext_type {
        "checkout_ui_extension" | "checkout_post_purchase" => options.checkout_cart_url.clone(),
        "subscription_ui_extension" => options.subscription_product_url.clone(),
        _ => None,
    }
}

pub fn file_mtime_ms(path: &Path) -> u64 {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn resolve_output_dir(output_path: &Path) -> PathBuf {
    let ext_ok = match output_path.extension() {
        None => true,
        Some(e) => e == "wasm" || e == "js" || e == "mjs",
    };
    let name_has_dot = output_path
        .file_name()
        .map(|n| n.to_string_lossy().contains('.'))
        .unwrap_or(false);
    if (output_path.is_dir() || (ext_ok && name_has_dot))
        && (output_path.is_file() || output_path.extension().is_some())
    {
        return output_path.parent().unwrap_or(output_path).to_path_buf();
    }
    output_path.to_path_buf()
}

/// True when `candidate` is equal to or nested under `root`.
pub fn is_subpath(root: &Path, candidate: &Path) -> bool {
    let Ok(root) = root.canonicalize() else {
        return false;
    };
    let Some(cand) = canonicalize_with_missing_tail(candidate) else {
        return false;
    };
    cand.starts_with(&root)
}

/// Canonicalize the existing portion of a path while retaining a non-existent
/// tail. This avoids comparing `/var/...` to `/private/var/...` on macOS when
/// the candidate output file has not been created yet.
fn canonicalize_with_missing_tail(path: &Path) -> Option<PathBuf> {
    let mut current = path;
    let mut missing = Vec::new();
    loop {
        match current.canonicalize() {
            Ok(mut canonical) => {
                for component in missing.iter().rev() {
                    canonical.push(component);
                }
                return Some(canonical);
            }
            Err(_) => {
                missing.push(current.file_name()?.to_os_string());
                current = current.parent()?;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::extensions::create_extension_specification;
    use tempfile::tempdir;

    fn sample_options() -> ExtensionsPayloadStoreOptions {
        ExtensionsPayloadStoreOptions {
            websocket_url: "ws://localhost:9293/extensions".into(),
            url: "http://localhost:9293".into(),
            api_key: "api-key".into(),
            app_name: "Test".into(),
            app_id: None,
            store_fqdn: "shop.myshopify.com".into(),
            store_id: "1".into(),
            granted_scopes: vec!["read_products".into()],
            checkout_cart_url: Some("/cart/1:1".into()),
            subscription_product_url: None,
            manifest_version: "3".into(),
        }
    }

    fn make_ui_ext(dir: &Path) -> ExtensionInstance {
        let spec = create_extension_specification("ui_extension").unwrap();
        let mut config = HashMap::new();
        config.insert(
            "extension_points".into(),
            json!([{ "target": "admin.product-details.block.render" }]),
        );
        config.insert("name".into(), json!("My Ext"));
        let mut ext = ExtensionInstance::new(
            "my-ext",
            dir.to_path_buf(),
            dir.join("shopify.extension.toml"),
            config,
            spec,
        );
        ext.uid = Some("uid-1".into());
        let _ = ext.ensure_dev_uuid();
        ext
    }

    #[test]
    fn payload_shape_basics() {
        let dir = tempdir().unwrap();
        let ext_dir = dir.path().join("ext");
        fs::create_dir_all(&ext_dir).unwrap();
        let bundle = dir.path().join("bundle");
        fs::create_dir_all(bundle.join("my-ext")).unwrap();
        let out = bundle.join("my-ext/my-ext.js");
        fs::write(&out, "console.log(1)").unwrap();

        let mut ext = make_ui_ext(&ext_dir);
        ext.output_path = Some(PathBuf::from("my-ext/my-ext.js"));
        let opts = sample_options();
        let mut resolver = AssetResolver::new();
        let payload =
            get_ui_extension_payload(&ext, &bundle, &opts, Some(&mut resolver), None, None)
                .unwrap();

        assert_eq!(payload.handle, "my-ext");
        assert_eq!(payload.name, "My Ext");
        assert_eq!(payload.approval_scopes, vec!["read_products"]);
        assert!(payload
            .assets
            .main
            .url
            .contains("/extensions/dev-uid-1/assets/"));
        assert_eq!(payload.development.status, "success");
        assert!(!payload.development.hidden);
        assert!(is_new_extension_points_schema(&payload.extension_points));
    }

    #[test]
    fn resolver_cleared_across_regenerations() {
        let dir = tempdir().unwrap();
        let ext_dir = dir.path().join("ext");
        fs::create_dir_all(&ext_dir).unwrap();
        let bundle = dir.path().join("bundle/my-ext");
        fs::create_dir_all(&bundle).unwrap();
        fs::write(
            bundle.join("manifest.json"),
            r#"{
            "admin.product-details.block.render": { "main": "main.js" }
        }"#,
        )
        .unwrap();
        fs::write(bundle.join("main.js"), "x").unwrap();

        let ext = make_ui_ext(&ext_dir);
        let opts = sample_options();
        let mut resolver = AssetResolver::new();
        resolver.insert("stale".into(), "gone".into());
        let _ = get_ui_extension_payload(
            &ext,
            dir.path().join("bundle").as_path(),
            &opts,
            Some(&mut resolver),
            None,
            None,
        )
        .unwrap();
        assert!(!resolver.contains_key("stale"));
    }

    #[test]
    fn post_purchase_mocks_target() {
        let dir = tempdir().unwrap();
        let ext_dir = dir.path().join("ext");
        fs::create_dir_all(&ext_dir).unwrap();
        let spec = create_extension_specification("checkout_post_purchase")
            .or_else(|| create_extension_specification("ui_extension"))
            .unwrap();
        let mut config = HashMap::new();
        config.insert("type".into(), json!("checkout_post_purchase"));
        let mut ext = ExtensionInstance::new("pp", ext_dir, PathBuf::from("t.toml"), config, spec);
        let _ = ext.ensure_dev_uuid();
        let opts = sample_options();
        let payload = get_ui_extension_payload(&ext, dir.path(), &opts, None, None, None).unwrap();
        let target = payload.extension_points[0]["target"].as_str().unwrap();
        assert_eq!(target, "purchase.post.render");
    }

    #[test]
    fn is_subpath_rejects_escape() {
        let dir = tempdir().unwrap();
        let root = dir.path().join("out");
        fs::create_dir_all(&root).unwrap();
        assert!(is_subpath(&root, &root.join("a.js")));
        assert!(!is_subpath(&root, dir.path()));
    }

    #[test]
    fn is_subpath_accepts_a_nonexistent_child_of_a_canonical_root() {
        let dir = tempdir().unwrap();
        let root = dir.path().join("out");
        fs::create_dir_all(&root).unwrap();
        assert!(is_subpath(&root, &root.join("nested").join("a.js")));
    }
}
