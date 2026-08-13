//! Deploy-config builders for every local extension specification.

use crate::error::AppError;
use crate::models::extensions::schemas::{
    config_without_first_class_fields, require_string, validate_base_fields,
};
use crate::models::extensions::specification::{ExtensionSpecification, UidStrategy};
use crate::models::extensions::transform::{
    app_access_transform, app_config_transform, app_home_transform, branding_transform,
    point_of_sale_transform, prepend_application_url, transform_app_proxy_forward,
    transform_webhooks_forward,
};
use crate::utilities::locales::load_locales_config;
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Context for deploy-config generation.
#[derive(Debug, Clone, Default)]
pub struct DeployConfigContext {
    pub app_configuration: Option<Value>,
    pub api_key: String,
    pub module_id: Option<String>,
}

/// Build the platform deploy payload for an extension.
pub async fn build_deploy_config(
    spec: &ExtensionSpecification,
    configuration: &HashMap<String, Value>,
    directory: &Path,
    ctx: &DeployConfigContext,
) -> Result<Option<Value>, AppError> {
    let config = Value::Object(
        configuration
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect(),
    );
    match spec.identifier.as_str() {
        // Config modules use transform, not deployConfig — still expose as deploy payload.
        "branding" => Ok(Some(app_config_transform(
            &config,
            &branding_transform(),
            false,
        ))),
        "app_access" => Ok(Some(app_config_transform(
            &config,
            &app_access_transform(),
            false,
        ))),
        "app_home" => Ok(Some(app_config_transform(
            &config,
            &app_home_transform(),
            false,
        ))),
        "point_of_sale" => Ok(Some(app_config_transform(
            &config,
            &point_of_sale_transform(),
            false,
        ))),
        "app_proxy" => {
            let app_url = application_url(ctx);
            Ok(Some(transform_app_proxy_forward(&config, &app_url)))
        }
        "webhooks" => Ok(Some(transform_webhooks_forward(&config))),
        "webhook_subscription" => Ok(Some(transform_webhook_subscription_forward(&config, ctx))),
        "privacy_compliance_webhooks" => {
            Ok(Some(transform_privacy_compliance_forward(&config, ctx)))
        }
        "events" => Ok(Some(transform_events_forward(&config, ctx))),
        "function" => deploy_function(&config, directory, ctx).await,
        "theme" => Ok(Some(json!({ "theme_extension": { "files": {} } }))),
        "ui_extension" => {
            crate::models::extensions::specifications::deploy_ui_extension(&config, directory)
                .await
        }
        "checkout_ui_extension" => deploy_checkout_ui(&config, directory).await,
        "checkout_post_purchase" => Ok(Some(json!({
            "metafields": config.get("metafields").cloned().unwrap_or_else(|| json!([]))
        }))),
        "pos_ui_extension" => Ok(Some(json!({
            "name": config.get("name"),
            "description": config.get("description"),
            "renderer_version": dependency_version(directory, "@shopify/retail-ui-extensions")
                .unwrap_or_else(|| "0.0.0".into()),
        }))),
        "product_subscription" => Ok(Some(json!({
            "renderer_version": dependency_version(directory, "@shopify/admin-ui-extensions")
                .unwrap_or_else(|| "0.0.0".into()),
        }))),
        "web_pixel_extension" => deploy_web_pixel(&config),
        "tax_calculation" => Ok(Some(json!({
            "production_api_base_url": config.get("production_api_base_url"),
            "benchmark_api_base_url": config.get("benchmark_api_base_url"),
            "calculate_taxes_api_endpoint": config.get("calculate_taxes_api_endpoint"),
            "metafields": config.get("metafields"),
            "cart_line_properties": config.get("cart_line_properties"),
            "api_version": config.get("api_version"),
            "metafield_identifiers": config.pointer("/input/metafield_identifiers").cloned(),
        }))),
        "editor_extension_collection" => deploy_editor_collection(&config, directory).await,
        "flow_action" => deploy_flow_action(&config, directory, ctx).await,
        "flow_trigger" => deploy_flow_trigger(&config, directory).await,
        "flow_template" => deploy_flow_template(&config, directory).await,
        "payments_extension" => {
            crate::models::extensions::specifications::deploy_payments(&config)
        }
        "admin_link" | "channel_config" | "order_attribution_config" => {
            deploy_contract(spec, &config, directory).await
        }
        _ => Ok(Some(config_without_first_class_fields(&config))),
    }
}

/// Local → remote transform for config modules (same as deploy for most).
pub fn transform_local_to_remote(
    spec: &ExtensionSpecification,
    local: &Value,
    app_configuration: Option<&Value>,
) -> Value {
    let ctx = DeployConfigContext {
        app_configuration: app_configuration.cloned(),
        ..Default::default()
    };
    match spec.identifier.as_str() {
        "branding" => app_config_transform(local, &branding_transform(), false),
        "app_access" => app_config_transform(local, &app_access_transform(), false),
        "app_home" => app_config_transform(local, &app_home_transform(), false),
        "point_of_sale" => app_config_transform(local, &point_of_sale_transform(), false),
        "app_proxy" => transform_app_proxy_forward(local, &application_url(&ctx)),
        "webhooks" => transform_webhooks_forward(local),
        "webhook_subscription" => transform_webhook_subscription_forward(local, &ctx),
        "privacy_compliance_webhooks" => transform_privacy_compliance_forward(local, &ctx),
        "events" => transform_events_forward(local, &ctx),
        _ => local.clone(),
    }
}

pub fn transform_remote_to_local(spec: &ExtensionSpecification, remote: &Value) -> Value {
    use crate::models::extensions::transform::{
        transform_app_proxy_reverse, transform_webhooks_reverse,
    };
    match spec.identifier.as_str() {
        "branding" => app_config_transform(remote, &branding_transform(), true),
        "app_access" => app_config_transform(remote, &app_access_transform(), true),
        "app_home" => app_config_transform(remote, &app_home_transform(), true),
        "point_of_sale" => app_config_transform(remote, &point_of_sale_transform(), true),
        "app_proxy" => transform_app_proxy_reverse(remote),
        "webhooks" => transform_webhooks_reverse(remote),
        _ => remote.clone(),
    }
}

/// Validate configuration against the specification's base rules.
pub fn validate_configuration(
    spec: &ExtensionSpecification,
    configuration: &HashMap<String, Value>,
    directory: &Path,
) -> Result<(), AppError> {
    let config = Value::Object(
        configuration
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect(),
    );
    let require_handle = matches!(
        spec.identifier.as_str(),
        "flow_action" | "flow_trigger" | "flow_template"
    );
    if !matches!(
        spec.experience,
        crate::models::extensions::specification::ExtensionExperience::Configuration
    ) || require_handle
    {
        validate_base_fields(&config, require_handle)?;
    }

    match spec.identifier.as_str() {
        "function" => {
            require_string(&config, "name")?;
            require_string(&config, "api_version")?;
        }
        "ui_extension" => {
            crate::models::extensions::specifications::validate_ui_extension(&config, directory)?;
        }
        "payments_extension" => {
            crate::models::extensions::specifications::validate_payments(&config)?;
        }
        "checkout_ui_extension" | "pos_ui_extension" => {
            require_string(&config, "name")?;
        }
        "web_pixel_extension" => {
            require_string(&config, "runtime_context")?;
            if config.get("configuration").is_some() {
                return Err(AppError::message(
                    "The property configuration is deprecated and no longer supported. It has been replaced by settings.",
                ));
            }
        }
        "tax_calculation" => {
            require_string(&config, "production_api_base_url")?;
            require_string(&config, "calculate_taxes_api_endpoint")?;
        }
        "flow_action" => {
            require_string(&config, "name")?;
            require_string(&config, "runtime_url")?;
        }
        "flow_trigger" | "flow_template" | "editor_extension_collection" => {
            require_string(&config, "name")?;
        }
        "branding" => {
            let name = require_string(&config, "name")?;
            if name.len() > 30 {
                return Err(AppError::message("String must be less than 30 characters"));
            }
        }
        "app_home" => {
            require_string(&config, "application_url")?;
            if config.get("embedded").and_then(|v| v.as_bool()).is_none() {
                return Err(AppError::message("embedded is required"));
            }
        }
        "app_access" if config.pointer("/auth/redirect_urls").is_none() => {
            return Err(AppError::message("auth.redirect_urls is required"));
        }
        "theme" => validate_theme_extension(directory)?,
        _ => {}
    }
    Ok(())
}

/// Patch local config with app-dev URLs (app_access / app_home / app_proxy / flow_action).
pub fn patch_with_app_dev_urls(
    spec: &ExtensionSpecification,
    configuration: &mut HashMap<String, Value>,
    urls: &AppDevUrls,
) {
    match spec.identifier.as_str() {
        "app_access" => {
            if let Some(ref whitelist) = urls.redirect_url_whitelist {
                configuration.insert("auth".into(), json!({ "redirect_urls": whitelist }));
            }
        }
        "app_home" => {
            if let Some(ref app_url) = urls.application_url {
                configuration.insert("application_url".into(), json!(app_url));
            }
        }
        "app_proxy" => {
            if let Some(ref proxy) = urls.app_proxy {
                configuration.insert(
                    "app_proxy".into(),
                    json!({
                        "url": proxy.url,
                        "subpath": proxy.subpath,
                        "prefix": proxy.prefix,
                    }),
                );
            }
        }
        "flow_action" => {
            for key in [
                "runtime_url",
                "validation_url",
                "config_page_url",
                "config_page_preview_url",
            ] {
                if let Some(Value::String(url)) = configuration.get(key) {
                    if url.starts_with('/') {
                        if let Some(ref app_url) = urls.application_url {
                            configuration
                                .insert(key.into(), json!(prepend_application_url(url, app_url)));
                        }
                    }
                }
            }
        }
        _ => {}
    }
}

#[derive(Debug, Clone, Default)]
pub struct AppDevUrls {
    pub application_url: Option<String>,
    pub redirect_url_whitelist: Option<Vec<String>>,
    pub app_proxy: Option<AppProxyUrls>,
}

#[derive(Debug, Clone)]
pub struct AppProxyUrls {
    pub url: String,
    pub subpath: String,
    pub prefix: String,
}

fn application_url(ctx: &DeployConfigContext) -> String {
    ctx.app_configuration
        .as_ref()
        .and_then(|c| c.get("application_url"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

fn transform_webhook_subscription_forward(local: &Value, ctx: &DeployConfigContext) -> Value {
    let mut out = local.clone();
    if let Some(uri) = out.get("uri").and_then(|v| v.as_str()) {
        let absolute = prepend_application_url(uri, &application_url(ctx));
        if let Some(obj) = out.as_object_mut() {
            obj.insert("uri".into(), json!(absolute));
        }
    }
    out
}

fn transform_privacy_compliance_forward(local: &Value, ctx: &DeployConfigContext) -> Value {
    let app_url = application_url(ctx);
    let webhooks = local.get("webhooks").cloned().unwrap_or(json!({}));
    let api_version = webhooks.get("api_version").cloned();

    let mut customers_redact = None;
    let mut customers_data = None;
    let mut shop_redact = None;

    if let Some(pc) = webhooks.get("privacy_compliance") {
        customers_redact = pc
            .get("customer_deletion_url")
            .and_then(|v| v.as_str())
            .map(|u| prepend_application_url(u, &app_url));
        customers_data = pc
            .get("customer_data_request_url")
            .and_then(|v| v.as_str())
            .map(|u| prepend_application_url(u, &app_url));
        shop_redact = pc
            .get("shop_deletion_url")
            .and_then(|v| v.as_str())
            .map(|u| prepend_application_url(u, &app_url));
    }

    if let Some(subs) = webhooks.get("subscriptions").and_then(|v| v.as_array()) {
        for sub in subs {
            let topics = sub
                .get("compliance_topics")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            let uri = sub
                .get("uri")
                .and_then(|v| v.as_str())
                .map(|u| prepend_application_url(u, &app_url));
            for t in topics {
                match t.as_str() {
                    Some("customers/redact") => customers_redact = uri.clone(),
                    Some("customers/data_request") => customers_data = uri.clone(),
                    Some("shop/redact") => shop_redact = uri.clone(),
                    _ => {}
                }
            }
        }
    }

    if customers_redact.is_none() && customers_data.is_none() && shop_redact.is_none() {
        return json!({});
    }

    let mut out = Map::new();
    if let Some(v) = api_version {
        out.insert("api_version".into(), v);
    }
    if let Some(u) = customers_redact {
        out.insert("customers_redact_url".into(), json!(u));
    }
    if let Some(u) = customers_data {
        out.insert("customers_data_request_url".into(), json!(u));
    }
    if let Some(u) = shop_redact {
        out.insert("shop_redact_url".into(), json!(u));
    }
    Value::Object(out)
}

fn transform_events_forward(local: &Value, ctx: &DeployConfigContext) -> Value {
    let app_url = application_url(ctx);
    let mut events = local.get("events").cloned().unwrap_or(json!({}));
    let key = if events.get("subscriptions").is_some() {
        "subscriptions"
    } else if events.get("subscription").is_some() {
        "subscription"
    } else {
        ""
    };
    if !key.is_empty() {
        if let Some(subs) = events.get_mut(key).and_then(|v| v.as_array_mut()) {
            for sub in subs.iter_mut() {
                if let Some(uri) = sub.get("uri").and_then(|v| v.as_str()) {
                    let absolute = prepend_application_url(uri, &app_url);
                    if let Some(obj) = sub.as_object_mut() {
                        obj.insert("uri".into(), json!(absolute));
                    }
                }
            }
        }
    }
    json!({ "events": events })
}

async fn deploy_function(
    config: &Value,
    directory: &Path,
    ctx: &DeployConfigContext,
) -> Result<Option<Value>, AppError> {
    let module_id = ctx
        .module_id
        .clone()
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    let input_query_path = directory.join("input.graphql");
    let input_query = if input_query_path.is_file() {
        Some(fs::read_to_string(input_query_path)?)
    } else {
        None
    };

    let mut targets = None;
    if let Some(Value::Array(items)) = config.get("targeting") {
        let mut out = Vec::new();
        for item in items {
            let target = item
                .get("target")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            let export = item.get("export").cloned();
            let mut input_query = None;
            if let Some(rel) = item.get("input_query").and_then(|v| v.as_str()) {
                let path = directory.join(rel);
                if !path.is_file() {
                    return Err(AppError::message(format!(
                        "No input query file at {}.",
                        path.display()
                    )));
                }
                input_query = Some(fs::read_to_string(path)?);
            }
            out.push(json!({
                "handle": target,
                "export": export,
                "input_query": input_query,
            }));
        }
        targets = Some(Value::Array(out));
    }

    let mut ui = None;
    if let Some(ui_cfg) = config.get("ui") {
        let mut ui_obj = Map::new();
        if let Some(paths) = ui_cfg.get("paths") {
            ui_obj.insert(
                "app_bridge".into(),
                json!({
                    "details_path": paths.get("details"),
                    "create_path": paths.get("create"),
                }),
            );
        }
        if let Some(handle) = ui_cfg.get("handle") {
            ui_obj.insert("ui_extension_handle".into(), handle.clone());
        }
        if !ui_obj.is_empty() {
            ui = Some(Value::Object(ui_obj));
        }
    }

    let api_type = config
        .get("type")
        .and_then(|v| v.as_str())
        .filter(|t| *t != "function")
        .map(|s| s.to_string());

    let enable_creation_ui = config
        .pointer("/ui/enable_create")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let input_query_variables = config
        .pointer("/input/variables")
        .map(|vars| json!({ "single_json_metafield": vars }));

    let localization = load_locales_config(directory, "function")?;

    Ok(Some(json!({
        "title": config.get("name"),
        "module_id": module_id,
        "description": config.get("description"),
        "app_key": ctx.api_key,
        "api_type": api_type,
        "api_version": config.get("api_version"),
        "input_query": input_query,
        "input_query_variables": input_query_variables,
        "ui": ui,
        "enable_creation_ui": enable_creation_ui,
        "localization": localization,
        "targets": targets,
    })))
}

async fn deploy_checkout_ui(config: &Value, directory: &Path) -> Result<Option<Value>, AppError> {
    let localization = load_locales_config(directory, "checkout_ui")?;
    Ok(Some(json!({
        "extension_points": config.get("extension_points"),
        "capabilities": config.get("capabilities"),
        "supported_features": config.get("supported_features"),
        "metafields": config.get("metafields").cloned().unwrap_or_else(|| json!([])),
        "name": config.get("name"),
        "settings": config.get("settings"),
        "localization": localization,
    })))
}

fn deploy_web_pixel(config: &Value) -> Result<Option<Value>, AppError> {
    Ok(Some(json!({
        "runtime_context": config.get("runtime_context"),
        "customer_privacy": config.get("customer_privacy"),
        "runtime_configuration_definition": config.get("settings"),
    })))
}

async fn deploy_editor_collection(
    config: &Value,
    directory: &Path,
) -> Result<Option<Value>, AppError> {
    let mut in_collection = Vec::new();
    if let Some(Value::Array(includes)) = config.get("includes") {
        for item in includes {
            if let Some(h) = item.as_str() {
                in_collection.push(json!({ "handle": h }));
            }
        }
    }
    if let Some(Value::Array(include)) = config.get("include") {
        for item in include {
            if let Some(h) = item.get("handle") {
                in_collection.push(json!({ "handle": h }));
            }
        }
    }
    let name = config
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("collection");
    let localization = load_locales_config(directory, name)?;
    Ok(Some(json!({
        "name": config.get("name"),
        "handle": config.get("handle"),
        "in_collection": in_collection,
        "localization": localization,
    })))
}

async fn deploy_flow_action(
    config: &Value,
    directory: &Path,
    ctx: &DeployConfigContext,
) -> Result<Option<Value>, AppError> {
    let app_url = application_url(ctx);
    let resolve = |key: &str| -> Option<String> {
        config
            .get(key)
            .and_then(|v| v.as_str())
            .map(|u| prepend_application_url(u, &app_url))
    };
    let schema_patch = load_schema_patch(directory, config.get("schema"))?;
    let fields = serialize_flow_fields(config.pointer("/settings/fields"));
    Ok(Some(json!({
        "title": config.get("name"),
        "description": config.get("description"),
        "url": resolve("runtime_url"),
        "fields": fields,
        "validation_url": resolve("validation_url"),
        "custom_configuration_page_url": resolve("config_page_url"),
        "custom_configuration_page_preview_url": resolve("config_page_preview_url"),
        "schema_patch": schema_patch,
        "return_type_ref": config.get("return_type_ref"),
    })))
}

async fn deploy_flow_trigger(config: &Value, directory: &Path) -> Result<Option<Value>, AppError> {
    let schema_patch = load_schema_patch(directory, config.get("schema"))?;
    let fields = serialize_flow_fields(config.pointer("/settings/fields"));
    Ok(Some(json!({
        "title": config.get("name"),
        "description": config.get("description"),
        "fields": fields,
        "schema_patch": schema_patch,
    })))
}

async fn deploy_flow_template(config: &Value, directory: &Path) -> Result<Option<Value>, AppError> {
    let module_rel = config
        .pointer("/template/module")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::message("template.module is required"))?;
    let module_path = directory.join(module_rel);
    if !module_path.is_file() {
        return Err(AppError::message(format!(
            "Flow template module not found: {}",
            module_path.display()
        )));
    }
    let bytes = fs::read(&module_path)?;
    let definition = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &bytes);
    let name = config
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("template");
    let localization = load_locales_config(directory, name)?;
    let template = config.get("template").cloned().unwrap_or(json!({}));
    Ok(Some(json!({
        "template_handle": config.get("handle"),
        "name": config.get("name"),
        "description": config.get("description"),
        "categories": template.get("categories"),
        "require_app": template.get("require_app"),
        "discoverable": template.get("discoverable"),
        "allow_one_click_activate": template.get("allow_one_click_activate"),
        "enabled": template.get("enabled"),
        "definition": definition,
        "localization": localization,
    })))
}

async fn deploy_contract(
    spec: &ExtensionSpecification,
    config: &Value,
    directory: &Path,
) -> Result<Option<Value>, AppError> {
    let mut parsed = config_without_first_class_fields(config);
    if spec
        .features
        .contains(&crate::models::extensions::specification::ExtensionFeature::Localization)
    {
        if let Some(localization) = load_locales_config(directory, &spec.identifier)? {
            if let Some(obj) = parsed.as_object_mut() {
                obj.insert("localization".into(), localization);
            }
        }
    }
    Ok(Some(parsed))
}

fn serialize_flow_fields(fields: Option<&Value>) -> Value {
    let Some(Value::Array(items)) = fields else {
        return Value::Array(vec![]);
    };
    Value::Array(items.clone())
}

fn load_schema_patch(directory: &Path, schema: Option<&Value>) -> Result<Option<String>, AppError> {
    let Some(rel) = schema.and_then(|v| v.as_str()) else {
        return Ok(None);
    };
    let path = directory.join(rel);
    if !path.is_file() {
        return Err(AppError::message(format!(
            "Schema file not found: {}",
            path.display()
        )));
    }
    Ok(Some(fs::read_to_string(path)?))
}

fn dependency_version(directory: &Path, package: &str) -> Option<String> {
    let pkg_path = directory.join("package.json");
    let content = fs::read_to_string(pkg_path).ok()?;
    let json: Value = serde_json::from_str(&content).ok()?;
    for section in ["dependencies", "devDependencies"] {
        if let Some(v) = json
            .get(section)
            .and_then(|d| d.get(package))
            .and_then(|v| v.as_str())
        {
            return Some(
                v.trim_start_matches('^')
                    .trim_start_matches('~')
                    .to_string(),
            );
        }
    }
    None
}

fn validate_theme_extension(directory: &Path) -> Result<(), AppError> {
    const BUNDLE_LIMIT: u64 = 10 * 1024 * 1024;
    const LIQUID_LIMIT: u64 = 500 * 1024;
    let supported = [
        (
            "assets",
            &[
                ".jpg", ".jpeg", ".json", ".js", ".css", ".png", ".svg", ".wasm",
            ][..],
        ),
        ("blocks", &[".liquid"][..]),
        ("locales", &[".json"][..]),
        ("snippets", &[".liquid"][..]),
    ];

    let mut total = 0u64;
    let mut liquid = 0u64;

    for (bucket, exts) in supported {
        let dir = directory.join(bucket);
        if !dir.is_dir() {
            continue;
        }
        for entry in walkdir_files(&dir) {
            let rel = entry.strip_prefix(directory).unwrap_or(&entry);
            let ext = entry
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| format!(".{e}"))
                .unwrap_or_default();
            if !exts.iter().any(|e| *e == ext) {
                return Err(AppError::message(format!(
                    "Invalid filename in your theme app extension: {}",
                    rel.display()
                )));
            }
            let size = fs::metadata(&entry)?.len();
            total += size;
            if matches!(bucket, "blocks" | "snippets") {
                liquid += size;
            }
        }
    }

    // Reject unsupported top-level dirs with files
    if let Ok(entries) = fs::read_dir(directory) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if entry.path().is_dir()
                && !["assets", "blocks", "locales", "snippets", ".git"].contains(&name.as_str())
            {
                // Only error if the dir has files
                if walkdir_files(&entry.path()).next().is_some() {
                    return Err(AppError::message(format!(
                        "Your theme app extension includes files in an unsupported directory, {name}"
                    )));
                }
            }
        }
    }

    if total > BUNDLE_LIMIT {
        return Err(AppError::message(
            "Your theme app extension exceeds the file size limit (10 MB).",
        ));
    }
    if liquid > LIQUID_LIMIT {
        return Err(AppError::message(
            "Your theme app extension exceeds the total liquid file size limit (500 kB).",
        ));
    }
    let _ = UidStrategy::Uuid;
    Ok(())
}

fn walkdir_files(dir: &Path) -> Box<dyn Iterator<Item = std::path::PathBuf>> {
    let mut files = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                files.extend(walkdir_files(&path));
            } else if path.is_file() {
                files.push(path);
            }
        }
    }
    Box::new(files.into_iter())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::extensions::specification::{
        ExtensionExperience, ExtensionFeature, ExtensionSpecification,
    };
    use std::collections::HashMap;
    use tempfile::tempdir;

    fn meta(id: &str) -> ExtensionSpecification {
        ExtensionSpecification {
            identifier: id.into(),
            external_identifier: format!("{id}_external"),
            external_name: id.into(),
            partners_web_identifier: id.into(),
            surface: "admin".into(),
            experience: ExtensionExperience::Extension,
            registration_limit: 50,
            additional_identifiers: vec![],
            group: None,
            features: vec![],
            uid_strategy: UidStrategy::Uuid,
            graph_ql_type: None,
            dependency: None,
        }
    }

    #[tokio::test]
    async fn function_deploy_config_reads_input_query() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("input.graphql"), "query { cart { id } }").unwrap();
        let mut cfg = HashMap::new();
        cfg.insert("name".into(), json!("Discount"));
        cfg.insert("type".into(), json!("function"));
        cfg.insert("api_version".into(), json!("2024-10"));
        let ctx = DeployConfigContext {
            api_key: "key".into(),
            module_id: Some("mid".into()),
            ..Default::default()
        };
        let out = build_deploy_config(&meta("function"), &cfg, dir.path(), &ctx)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(out.get("title").and_then(|v| v.as_str()), Some("Discount"));
        assert_eq!(out.get("module_id").and_then(|v| v.as_str()), Some("mid"));
        assert!(out
            .get("input_query")
            .and_then(|v| v.as_str())
            .unwrap()
            .contains("cart"));
    }

    #[tokio::test]
    async fn branding_uses_transform() {
        let mut cfg = HashMap::new();
        cfg.insert("name".into(), json!("My App"));
        cfg.insert("handle".into(), json!("my-app"));
        let out = build_deploy_config(
            &ExtensionSpecification {
                experience: ExtensionExperience::Configuration,
                uid_strategy: UidStrategy::Single,
                ..meta("branding")
            },
            &cfg,
            Path::new("."),
            &DeployConfigContext::default(),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(
            out.get("app_handle").and_then(|v| v.as_str()),
            Some("my-app")
        );
    }

    #[tokio::test]
    async fn contract_strips_first_class() {
        let mut cfg = HashMap::new();
        cfg.insert("type".into(), json!("admin_link"));
        cfg.insert("handle".into(), json!("link"));
        cfg.insert("name".into(), json!("Link"));
        let mut spec = meta("admin_link");
        spec.features = vec![ExtensionFeature::Localization];
        let out = build_deploy_config(&spec, &cfg, Path::new("."), &DeployConfigContext::default())
            .await
            .unwrap()
            .unwrap();
        assert!(out.get("type").is_none());
        assert_eq!(out.get("name").and_then(|v| v.as_str()), Some("Link"));
    }
}
