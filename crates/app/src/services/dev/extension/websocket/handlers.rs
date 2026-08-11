//! WebSocket message handlers (unit-testable without a live socket).

use super::models::{EventType, IncomingMessage, LogPayload};
use crate::services::dev::extension::payload::store::ExtensionsPayloadStore;
use crate::services::dev::extension::payload::models::UIExtensionPayload;
use serde_json::{json, Value};

pub fn build_connected_payload(
    store: &ExtensionsPayloadStore,
    manifest_version: &str,
) -> Value {
    json!({
        "event": "connected",
        "data": store.get_connected_payload(),
        "version": manifest_version,
    })
}

pub fn build_update_payload(
    store: &ExtensionsPayloadStore,
    manifest_version: &str,
    extension_ids: &[String],
) -> Value {
    json!({
        "event": EventType::Update,
        "version": manifest_version,
        "data": store.get_raw_payload_filtered_by_extension_ids(extension_ids),
    })
}

pub fn build_outgoing_dispatch(
    incoming: &Value,
    store: &ExtensionsPayloadStore,
    manifest_version: &str,
) -> Value {
    let raw = store.get_raw_payload();
    let mut out = incoming.clone();
    if let Some(obj) = out.as_object_mut() {
        obj.insert("version".into(), json!(manifest_version));
        if let Some(data) = obj.get_mut("data").and_then(|d| d.as_object_mut()) {
            data.insert("extensions".into(), json!([]));
            data.insert("store".into(), json!(raw.store));
            data.insert(
                "app".into(),
                json!({ "apiKey": raw.app.api_key }),
            );
        }
    }
    out
}

/// Handle an inbound WS text frame. Returns an optional outbound message (dispatch).
pub fn handle_incoming_message(
    text: &str,
    store: &mut ExtensionsPayloadStore,
    manifest_version: &str,
) -> Option<Value> {
    let parsed: IncomingMessage = serde_json::from_str(text).ok()?;
    match parsed.event.as_str() {
        "update" => {
            let payload_key = store.get_raw_payload().app.api_key.clone();
            if let Some(app) = parsed.data.get("app") {
                let event_key = app.get("apiKey").and_then(|v| v.as_str()).unwrap_or("");
                if payload_key != event_key {
                    return None;
                }
                store.update_app(app.clone());
            }
            if let Some(extensions) = parsed.data.get("extensions").and_then(|v| v.as_array()) {
                let parsed_exts: Vec<UIExtensionPayload> = extensions
                    .iter()
                    .filter_map(|v| serde_json::from_value(v.clone()).ok())
                    .collect();
                store.update_extensions(parsed_exts);
            }
            None
        }
        "dispatch" => {
            let full = json!({
                "event": "dispatch",
                "data": parsed.data,
            });
            Some(build_outgoing_dispatch(&full, store, manifest_version))
        }
        "log" => {
            if let Ok(log) = serde_json::from_value::<LogPayload>(parsed.data) {
                let _ = format_log_output(&log);
            }
            None
        }
        _ => None,
    }
}

pub fn parse_log_message(message: &str) -> String {
    match serde_json::from_str::<Value>(message) {
        Ok(Value::Array(args)) => args
            .iter()
            .map(|arg| match arg {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            })
            .collect::<Vec<_>>()
            .join(" "),
        Ok(_) => message.to_string(),
        Err(_) => message.to_string(),
    }
}

pub fn format_log_output(payload: &LogPayload) -> String {
    let formatted = parse_log_message(&payload.message);
    match payload.log_type.as_str() {
        "debug" | "warn" | "error" => {
            format!("{}: {formatted}", payload.log_type.to_uppercase())
        }
        "log" | "info" => formatted,
        other => format!("{}: {formatted}", other.to_uppercase()),
    }
}

/// Whether an HTTP upgrade request path should open a websocket.
pub fn should_upgrade_websocket(path: &str) -> bool {
    path == "/extensions"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::dev::extension::payload::models::{
        AppPayload, Asset, DevelopmentState, ExtensionsEndpointPayload, MainAssets,
        OptionalUrlHolder, UIExtensionPayload, UrlHolder,
    };
    use crate::services::dev::extension::payload::store::ExtensionsPayloadStoreOptions;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    fn store_with_key(api_key: &str) -> ExtensionsPayloadStore {
        let opts = ExtensionsPayloadStoreOptions {
            websocket_url: "ws://localhost/extensions".into(),
            url: "http://localhost".into(),
            api_key: api_key.into(),
            app_name: "App".into(),
            app_id: None,
            store_fqdn: "shop.myshopify.com".into(),
            store_id: "1".into(),
            granted_scopes: vec![],
            checkout_cart_url: None,
            subscription_product_url: None,
            manifest_version: "3".into(),
        };
        let raw = ExtensionsEndpointPayload {
            app: AppPayload {
                api_key: api_key.into(),
                url: "u".into(),
                mobile_url: "m".into(),
                title: "App".into(),
            },
            app_id: None,
            store: "shop.myshopify.com".into(),
            extensions: vec![UIExtensionPayload {
                assets: MainAssets {
                    main: Asset::new("main", "http://x/a.js", 0),
                },
                supported_features: None,
                capabilities: None,
                development: DevelopmentState {
                    resource: OptionalUrlHolder { url: None },
                    root: UrlHolder {
                        url: "http://x".into(),
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
                uuid: "dev-1".into(),
                version: None,
                surface: "admin".into(),
                title: "a".into(),
                handle: "a".into(),
                name: "a".into(),
                description: None,
                approval_scopes: vec![],
                settings: None,
            }],
            version: "3".into(),
            root: UrlHolder {
                url: "http://localhost/extensions".into(),
            },
            dev_console: UrlHolder {
                url: "http://localhost/extensions/dev-console".into(),
            },
            socket: UrlHolder {
                url: "ws://localhost/extensions".into(),
            },
        };
        ExtensionsPayloadStore::new(raw, opts, HashMap::new())
    }

    #[test]
    fn upgrade_only_extensions_path() {
        assert!(should_upgrade_websocket("/extensions"));
        assert!(!should_upgrade_websocket("/other"));
    }

    #[test]
    fn connected_payload_shape() {
        let store = store_with_key("key");
        let msg = build_connected_payload(&store, "3");
        assert_eq!(msg["event"], "connected");
        assert_eq!(msg["version"], "3");
        assert_eq!(msg["data"]["store"], "shop.myshopify.com");
    }

    #[test]
    fn update_handler_filters_ids() {
        let store = store_with_key("key");
        let msg = build_update_payload(&store, "3", &["dev-1".into()]);
        assert_eq!(msg["event"], "update");
        assert_eq!(msg["data"]["extensions"].as_array().unwrap().len(), 1);

        let msg = build_update_payload(&store, "3", &["missing".into()]);
        assert!(msg["data"]["extensions"].as_array().unwrap().is_empty());
    }

    #[test]
    fn incoming_update_requires_matching_api_key() {
        let mut store = store_with_key("test-app-api-key");
        let bad = r#"{"event":"update","data":{"app":{"apiKey":"other"},"extensions":[]}}"#;
        assert!(handle_incoming_message(bad, &mut store, "3").is_none());
        assert_eq!(store.get_raw_payload().app.api_key, "test-app-api-key");

        let good = r#"{"event":"update","data":{"app":{"apiKey":"test-app-api-key","title":"New"},"extensions":[]}}"#;
        assert!(handle_incoming_message(good, &mut store, "3").is_none());
        assert_eq!(store.get_raw_payload().app.title, "New");
    }

    #[test]
    fn dispatch_fans_out_with_store_and_app() {
        let mut store = store_with_key("key");
        let msg = r#"{"event":"dispatch","data":{"type":"focus","payload":[{"uuid":"dev-1"}]}}"#;
        let out = handle_incoming_message(msg, &mut store, "3").unwrap();
        assert_eq!(out["event"], "dispatch");
        assert_eq!(out["data"]["extensions"], json!([]));
        assert_eq!(out["data"]["store"], "shop.myshopify.com");
        assert_eq!(out["data"]["app"]["apiKey"], "key");
        assert_eq!(out["version"], "3");
    }

    #[test]
    fn log_does_not_notify() {
        let mut store = store_with_key("key");
        let msg = r#"{"event":"log","data":{"type":"info","message":"[\"hi\"]","extensionName":"ext"}}"#;
        assert!(handle_incoming_message(msg, &mut store, "3").is_none());
    }

    #[test]
    fn parse_log_message_cases() {
        assert_eq!(parse_log_message(r#"["a", 1]"#), "a 1");
        assert_eq!(parse_log_message(r#"{"x":1}"#), r#"{"x":1}"#);
        assert_eq!(parse_log_message("plain"), "plain");
    }

    #[test]
    fn format_log_levels() {
        let err = LogPayload {
            log_type: "error".into(),
            message: r#"["boom"]"#.into(),
            extension_name: "e".into(),
        };
        assert!(format_log_output(&err).starts_with("ERROR:"));
        let info = LogPayload {
            log_type: "info".into(),
            message: r#"["ok"]"#.into(),
            extension_name: "e".into(),
        };
        assert_eq!(format_log_output(&info), "ok");
    }

    #[test]
    fn emit_callback_on_store_update() {
        let mut store = store_with_key("key");
        let seen = Arc::new(Mutex::new(Vec::new()));
        let seen2 = seen.clone();
        store.on_update(move |ids| {
            seen2.lock().unwrap().push(ids);
        });
        store.update_app(json!({"title": "T"}));
        assert_eq!(seen.lock().unwrap().len(), 1);
    }
}
