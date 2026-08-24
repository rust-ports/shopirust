//! Text and JSON rendering for app log payloads.

use cli_api::AppLogData;
use serde_json::Value;

pub const ONE_MILLION: f64 = 1_000_000.0;
pub const LOG_TYPE_FUNCTION_RUN: &str = "function_run";
pub const LOG_TYPE_RESPONSE_FROM_CACHE: &str = "function_network_access.response_from_cache";
pub const LOG_TYPE_REQUEST_EXECUTION_IN_BACKGROUND: &str =
    "function_network_access.request_execution_in_background";
pub const LOG_TYPE_REQUEST_EXECUTION: &str = "function_network_access.request_execution";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Json,
    Text,
}

/// Parse payload JSON; keep snake_case keys from the wire (render camelCases for JSON out).
pub fn parse_app_log_payload(payload: &str, _log_type: &str) -> Value {
    serde_json::from_str(payload).unwrap_or(Value::Null)
}

pub fn snake_to_camel(key: &str) -> String {
    let mut out = String::with_capacity(key.len());
    let mut up = false;
    for ch in key.chars() {
        if ch == '_' || ch == '-' {
            up = true;
        } else if up {
            out.extend(ch.to_uppercase());
            up = false;
        } else {
            out.push(ch);
        }
    }
    out
}

/// Convert object keys to camelCase (upstream `camelcase-keys.ts`).
///
/// When `deep` is false, only the top-level object's keys are converted (top-level
/// arrays are left unchanged). When `deep` is true, nested objects and arrays recurse.
pub fn camelcase_keys(value: Value, deep: bool) -> Value {
    match value {
        Value::Object(map) => {
            let mut out = serde_json::Map::new();
            for (k, v) in map {
                let converted = if deep { camelcase_keys(v, true) } else { v };
                out.insert(snake_to_camel(&k), converted);
            }
            Value::Object(out)
        }
        Value::Array(arr) if deep => {
            Value::Array(arr.into_iter().map(|v| camelcase_keys(v, true)).collect())
        }
        other => other,
    }
}

/// Upstream `toFormattedAppLogJson`.
pub fn to_formatted_app_log_json(
    app_log: &AppLogData,
    app_log_payload: &Value,
    store_name: &str,
    pretty: bool,
) -> String {
    let mut obj = serde_json::Map::new();
    obj.insert("shopId".into(), Value::from(app_log.shop_id));
    obj.insert("apiClientId".into(), Value::from(app_log.api_client_id));
    obj.insert("logType".into(), Value::String(app_log.log_type.clone()));
    obj.insert("source".into(), Value::String(app_log.source.clone()));
    obj.insert(
        "sourceNamespace".into(),
        Value::String(app_log.source_namespace.clone()),
    );
    obj.insert("status".into(), Value::String(app_log.status.clone()));
    obj.insert(
        "logTimestamp".into(),
        Value::String(app_log.log_timestamp.clone()),
    );
    obj.insert(
        "localTime".into(),
        Value::String(format_local_date(&app_log.log_timestamp)),
    );
    obj.insert("storeName".into(), Value::String(store_name.to_string()));

    let mut payload = camelcase_keys(app_log_payload.clone(), true);
    if app_log.log_type == LOG_TYPE_FUNCTION_RUN {
        if let Some(logs) = payload.get("logs").and_then(|v| v.as_str()) {
            let lines: Vec<Value> = logs
                .lines()
                .filter(|l| !l.is_empty())
                .map(|l| Value::String(l.to_string()))
                .collect();
            if let Some(obj) = payload.as_object_mut() {
                obj.insert("logs".into(), Value::Array(lines));
            }
        }
    }
    obj.insert("payload".into(), payload);

    let value = Value::Object(obj);
    if pretty {
        serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{}".into())
    } else {
        serde_json::to_string(&value).unwrap_or_else(|_| "{}".into())
    }
}

fn format_local_date(iso: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(iso)
        .map(|dt| dt.with_timezone(&chrono::Local).to_rfc3339())
        .unwrap_or_else(|_| iso.to_string())
}

/// Human-readable one-line summary + optional details (text mode).
pub fn format_log_text(app_log: &AppLogData, store_name: &str) -> String {
    let payload = parse_app_log_payload(&app_log.payload, &app_log.log_type);
    let store_short = store_name.split('.').next().unwrap_or(store_name);
    let status_label = if app_log.status == "success" {
        "Success"
    } else {
        "Failure"
    };
    let description = description_for_log(app_log, &payload);
    let ts = format_local_date(&app_log.log_timestamp);

    let mut out = format!(
        "{ts} {store_short} {} {status_label} {description}\n",
        app_log.source
    );

    match app_log.log_type.as_str() {
        LOG_TYPE_FUNCTION_RUN => {
            if let Some(logs) = payload.get("logs").and_then(|v| v.as_str()) {
                for line in logs.lines().filter(|l| !l.is_empty()) {
                    out.push_str(&format!("    {line}\n"));
                }
            }
            out.push_str(&format_iqv_block(&payload));
            if let Some(input) = payload.get("input") {
                if !input.is_null() {
                    let bytes = payload
                        .get("input_bytes")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0);
                    out.push_str(&format!(
                        "    Input ({bytes} bytes):\n      {}\n",
                        pretty_json(input)
                    ));
                }
            }
            if let Some(output) = payload.get("output") {
                if !output.is_null() {
                    let bytes = payload
                        .get("output_bytes")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0);
                    out.push_str(&format!(
                        "    Output ({bytes} bytes):\n      {}\n",
                        pretty_json(output)
                    ));
                }
            }
        }
        LOG_TYPE_RESPONSE_FROM_CACHE => {
            if let Some(epoch) = payload.get("cache_entry_epoch_ms").and_then(json_i64) {
                out.push_str(&format!(
                    "    Cache write time: {}\n",
                    epoch_ms_to_iso(epoch)
                ));
            }
            if let Some(ttl) = payload.get("cache_ttl_ms").and_then(|v| v.as_f64()) {
                out.push_str(&format!("    Cache TTL: {} s\n", ttl / 1000.0));
            }
            out.push_str(&format!(
                "    HTTP request:\n      {}\n",
                pretty_json(payload.get("http_request").unwrap_or(&Value::Null))
            ));
            out.push_str(&format!(
                "    HTTP response:\n      {}\n",
                pretty_json(payload.get("http_response").unwrap_or(&Value::Null))
            ));
        }
        LOG_TYPE_REQUEST_EXECUTION_IN_BACKGROUND => {
            out.push_str(&format!(
                "    Reason: {}\n",
                background_execution_reason(payload.get("reason").and_then(|v| v.as_str()))
            ));
            out.push_str(&format!(
                "    HTTP request:\n      {}\n",
                pretty_json(payload.get("http_request").unwrap_or(&Value::Null))
            ));
        }
        LOG_TYPE_REQUEST_EXECUTION => {
            if let Some(attempt) = payload.get("attempt") {
                out.push_str(&format!("    Attempt: {}\n", display_json_scalar(attempt)));
            }
            if let Some(connect) = payload.get("connect_time_ms").and_then(json_i64) {
                out.push_str(&format!("    Connect time: {connect} ms\n"));
            }
            if let Some(write_read) = payload.get("write_read_time_ms").and_then(json_i64) {
                out.push_str(&format!("    Write read time: {write_read} ms\n"));
            }
            out.push_str(&format!(
                "    HTTP request:\n      {}\n",
                pretty_json(payload.get("http_request").unwrap_or(&Value::Null))
            ));
            if let Some(resp) = payload.get("http_response") {
                if !resp.is_null() {
                    out.push_str(&format!(
                        "    HTTP response:\n      {}\n",
                        pretty_json(resp)
                    ));
                }
            }
            if let Some(err) = payload.get("error").and_then(|v| v.as_str()) {
                out.push_str(&format!("    Error: {err}\n"));
            }
        }
        _ => {
            out.push_str(&format!("    {}\n", pretty_json(&payload)));
        }
    }

    out
}

fn description_for_log(app_log: &AppLogData, payload: &Value) -> String {
    match app_log.log_type.as_str() {
        LOG_TYPE_FUNCTION_RUN => {
            let export = payload
                .get("export")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            let fuel = payload
                .get("fuel_consumed")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0)
                / ONE_MILLION;
            format!("export \"{export}\" executed in {fuel:.4}M instructions")
        }
        LOG_TYPE_RESPONSE_FROM_CACHE => "network access response retrieved from cache".into(),
        LOG_TYPE_REQUEST_EXECUTION_IN_BACKGROUND => {
            "network access request executing in background".into()
        }
        LOG_TYPE_REQUEST_EXECUTION => {
            let connect = payload.get("connect_time_ms").and_then(|v| v.as_i64());
            let write = payload.get("write_read_time_ms").and_then(|v| v.as_i64());
            match (connect, write) {
                (Some(c), Some(w)) => format!("network access request executed in {} ms", c + w),
                _ => "network access request executed".into(),
            }
        }
        _ => app_log.log_type.clone(),
    }
}

fn pretty_json(value: &Value) -> String {
    if let Some(s) = value.as_str() {
        if let Ok(parsed) = serde_json::from_str::<Value>(s) {
            return serde_json::to_string_pretty(&parsed).unwrap_or_else(|_| s.to_string());
        }
        return s.to_string();
    }
    serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
}

fn format_iqv_block(payload: &Value) -> String {
    let namespace = payload
        .get("input_query_variables_metafield_namespace")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty());
    let key = payload
        .get("input_query_variables_metafield_key")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty());
    let (Some(namespace), Some(key)) = (namespace, key) else {
        return String::new();
    };
    let value = payload.get("input_query_variables_metafield_value");
    let rendered = match value {
        Some(v) if !v.is_null() => pretty_json(v),
        _ => "Metafield is not set".to_string(),
    };
    format!(
        "    Input Query Variables:\n      Namespace: {namespace}\n      Key: {key}\n      {rendered}\n"
    )
}

fn background_execution_reason(reason: Option<&str>) -> &'static str {
    match reason {
        Some("no_cached_response") => "No cached response available",
        Some("cached_response_about_to_expire") => "Cache is about to expire",
        _ => "Unknown reason",
    }
}

fn epoch_ms_to_iso(ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ms)
        .map(|dt| dt.to_rfc3339_opts(chrono::SecondsFormat::Millis, true))
        .unwrap_or_else(|| ms.to_string())
}

fn json_i64(value: &Value) -> Option<i64> {
    value.as_i64().or_else(|| value.as_f64().map(|n| n as i64))
}

fn display_json_scalar(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_format_camelcases_payload() {
        let log = AppLogData {
            shop_id: 1,
            api_client_id: 2,
            payload: r#"{"fuel_consumed":1000,"export":"run","logs":"a\nb\n"}"#.into(),
            log_type: LOG_TYPE_FUNCTION_RUN.into(),
            source: "fn".into(),
            source_namespace: "extensions".into(),
            cursor: "c".into(),
            status: "success".into(),
            log_timestamp: "2024-05-23T19:17:00.240053Z".into(),
        };
        let payload = parse_app_log_payload(&log.payload, &log.log_type);
        let json = to_formatted_app_log_json(&log, &payload, "shop.myshopify.com", false);
        assert!(json.contains("\"fuelConsumed\""));
        assert!(json.contains("\"storeName\":\"shop.myshopify.com\""));
        assert!(json.contains("\"logs\":[\"a\",\"b\"]"));
    }

    #[test]
    fn camelcase_converts_snake_case_keys() {
        let v = serde_json::json!({"foo_bar": 1, "baz_qux": 2});
        assert_eq!(
            camelcase_keys(v, false),
            serde_json::json!({"fooBar": 1, "bazQux": 2})
        );
    }

    #[test]
    fn camelcase_converts_kebab_case_keys() {
        let v = serde_json::json!({"foo-bar": 1, "baz-qux": 2});
        assert_eq!(
            camelcase_keys(v, false),
            serde_json::json!({"fooBar": 1, "bazQux": 2})
        );
    }

    #[test]
    fn camelcase_leaves_camel_case_unchanged() {
        let v = serde_json::json!({"alreadyCamel": 1});
        assert_eq!(
            camelcase_keys(v, false),
            serde_json::json!({"alreadyCamel": 1})
        );
    }

    #[test]
    fn camelcase_handles_null() {
        let v = serde_json::json!({"foo_bar": null});
        assert_eq!(
            camelcase_keys(v, false),
            serde_json::json!({"fooBar": null})
        );
    }

    #[test]
    fn camelcase_top_level_array_unchanged_without_deep() {
        let v = serde_json::json!([{"foo_bar": 1}]);
        assert_eq!(
            camelcase_keys(v, false),
            serde_json::json!([{"foo_bar": 1}])
        );
    }

    #[test]
    fn camelcase_does_not_recurse_by_default() {
        let v = serde_json::json!({"foo_bar": {"nested_key": 1}});
        assert_eq!(
            camelcase_keys(v, false),
            serde_json::json!({"fooBar": {"nested_key": 1}})
        );
    }

    #[test]
    fn camelcase_recurses_with_deep() {
        let v = serde_json::json!({"foo_bar": {"nested_key": 1}});
        assert_eq!(
            camelcase_keys(v, true),
            serde_json::json!({"fooBar": {"nestedKey": 1}})
        );
    }

    #[test]
    fn camelcase_recurses_into_arrays_with_deep() {
        let v = serde_json::json!({"arr": [{"nested_key": 1}]});
        assert_eq!(
            camelcase_keys(v, true),
            serde_json::json!({"arr": [{"nestedKey": 1}]})
        );
    }

    #[test]
    fn camelcase_top_level_arrays_with_deep() {
        let v = serde_json::json!([{"foo_bar": 1}]);
        assert_eq!(camelcase_keys(v, true), serde_json::json!([{"fooBar": 1}]));
    }

    #[test]
    fn camelcase_primitives_unchanged() {
        assert_eq!(camelcase_keys(Value::Null, true), Value::Null);
        assert_eq!(
            camelcase_keys(Value::String("hello".into()), true),
            Value::String("hello".into())
        );
    }

    #[test]
    fn camelcase_empty_object() {
        assert_eq!(
            camelcase_keys(serde_json::json!({}), false),
            serde_json::json!({})
        );
    }

    #[test]
    fn json_format_includes_shop_and_timestamps() {
        let log = AppLogData {
            shop_id: 1,
            api_client_id: 2,
            payload: r#"{"message":"Log 1"}"#.into(),
            log_type: "app".into(),
            source: "src".into(),
            source_namespace: "ns".into(),
            cursor: "c".into(),
            status: "success".into(),
            log_timestamp: "2024-09-14T05:00:00.000Z".into(),
        };
        let payload = parse_app_log_payload(&log.payload, &log.log_type);
        let json = to_formatted_app_log_json(&log, &payload, "storeName", false);
        assert!(json.contains("\"shopId\":1"));
        assert!(json.contains("\"storeName\":\"storeName\""));
        assert!(json.contains("\"logTimestamp\":\"2024-09-14T05:00:00.000Z\""));
        assert!(json.contains("\"payload\":{\"message\":\"Log 1\"}"));
    }

    #[test]
    fn text_format_function_run() {
        let log = AppLogData {
            shop_id: 1,
            api_client_id: 2,
            payload: r#"{"export":"run","fuel_consumed":2000000,"logs":"hello\n"}"#.into(),
            log_type: LOG_TYPE_FUNCTION_RUN.into(),
            source: "discount".into(),
            source_namespace: "extensions".into(),
            cursor: "c".into(),
            status: "success".into(),
            log_timestamp: "2024-05-23T19:17:00.240053Z".into(),
        };
        let text = format_log_text(&log, "shop.myshopify.com");
        assert!(text.contains("Success"));
        assert!(text.contains("discount"));
        assert!(text.contains("export \"run\""));
        assert!(text.contains("hello"));
    }

    #[test]
    fn text_format_function_run_iqv_metafield() {
        let log = AppLogData {
            shop_id: 1,
            api_client_id: 2,
            payload: r#"{"export":"run","fuel_consumed":1,"input_query_variables_metafield_namespace":"n","input_query_variables_metafield_key":"k","input_query_variables_metafield_value":"{\"a\":1}"}"#.into(),
            log_type: LOG_TYPE_FUNCTION_RUN.into(),
            source: "discount".into(),
            source_namespace: "extensions".into(),
            cursor: "c".into(),
            status: "success".into(),
            log_timestamp: "2024-05-23T19:17:00.240053Z".into(),
        };
        let text = format_log_text(&log, "shop.myshopify.com");
        assert!(text.contains("Input Query Variables:"));
        assert!(text.contains("Namespace: n"));
        assert!(text.contains("Key: k"));
        assert!(text.contains("\"a\""));
    }

    #[test]
    fn text_format_cache_ttl_and_write_time() {
        let log = AppLogData {
            shop_id: 1,
            api_client_id: 2,
            payload: r#"{"cache_entry_epoch_ms":1683904621000,"cache_ttl_ms":5000,"http_request":{"url":"https://x"},"http_response":{"status":200}}"#.into(),
            log_type: LOG_TYPE_RESPONSE_FROM_CACHE.into(),
            source: "discount".into(),
            source_namespace: "extensions".into(),
            cursor: "c".into(),
            status: "success".into(),
            log_timestamp: "2024-05-23T19:17:00.240053Z".into(),
        };
        let text = format_log_text(&log, "shop.myshopify.com");
        assert!(
            text.contains("Cache write time:"),
            "missing cache write time in: {text}"
        );
        assert!(text.contains("Cache TTL: 5 s"), "missing ttl in: {text}");
        assert!(text.contains("HTTP request:"));
    }

    #[test]
    fn text_format_background_reason() {
        let log = AppLogData {
            shop_id: 1,
            api_client_id: 2,
            payload: r#"{"reason":"no_cached_response","http_request":{"url":"https://x"}}"#.into(),
            log_type: LOG_TYPE_REQUEST_EXECUTION_IN_BACKGROUND.into(),
            source: "discount".into(),
            source_namespace: "extensions".into(),
            cursor: "c".into(),
            status: "success".into(),
            log_timestamp: "2024-05-23T19:17:00.240053Z".into(),
        };
        let text = format_log_text(&log, "shop.myshopify.com");
        assert!(text.contains("Reason: No cached response available"));
    }

    #[test]
    fn text_format_request_attempt_and_timings() {
        let log = AppLogData {
            shop_id: 1,
            api_client_id: 2,
            payload: r#"{"attempt":1,"connect_time_ms":10,"write_read_time_ms":20,"http_request":{"url":"https://x"},"http_response":{"status":200}}"#.into(),
            log_type: LOG_TYPE_REQUEST_EXECUTION.into(),
            source: "discount".into(),
            source_namespace: "extensions".into(),
            cursor: "c".into(),
            status: "success".into(),
            log_timestamp: "2024-05-23T19:17:00.240053Z".into(),
        };
        let text = format_log_text(&log, "shop.myshopify.com");
        assert!(text.contains("Attempt: 1"));
        assert!(text.contains("Connect time: 10 ms"));
        assert!(text.contains("Write read time: 20 ms"));
        assert!(text.contains("executed in 30 ms"));
    }
}
