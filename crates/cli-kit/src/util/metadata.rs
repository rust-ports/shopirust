use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

static PUBLIC_METADATA: std::sync::LazyLock<Mutex<HashMap<String, serde_json::Value>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));
static SENSITIVE_METADATA: std::sync::LazyLock<Mutex<HashMap<String, serde_json::Value>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

pub fn add_public_metadata(key: &str, value: serde_json::Value) {
    if let Ok(mut meta) = PUBLIC_METADATA.lock() {
        meta.insert(key.to_string(), value);
    }
}

pub fn add_sensitive_metadata(key: &str, value: serde_json::Value) {
    if let Ok(mut meta) = SENSITIVE_METADATA.lock() {
        meta.insert(key.to_string(), value);
    }
}

pub fn get_public_metadata() -> HashMap<String, serde_json::Value> {
    PUBLIC_METADATA
        .lock()
        .map(|m| m.clone())
        .unwrap_or_default()
}

pub fn get_sensitive_metadata() -> HashMap<String, serde_json::Value> {
    SENSITIVE_METADATA
        .lock()
        .map(|m| m.clone())
        .unwrap_or_default()
}

/// Apply store FQDN telemetry fields (matches `@shopify/store` `recordStoreFqdnMetadata`).
pub fn record_store_fqdn_metadata(store_fqdn: &str, validated: bool, store_id: Option<&str>) {
    let meta = store::attribution::record_store_fqdn_metadata(store_fqdn, validated, store_id);
    add_sensitive_metadata(
        "store_fqdn",
        serde_json::Value::String(meta.store_fqdn),
    );
    add_public_metadata(
        "store_fqdn_hash",
        serde_json::Value::String(meta.store_fqdn_hash),
    );
    add_public_metadata(
        "store_fqdn_validated",
        serde_json::Value::Bool(meta.store_fqdn_validated),
    );
    add_public_metadata(
        "store_domain",
        serde_json::Value::String(meta.store_domain),
    );
    if let Some(id) = meta.store_id {
        add_public_metadata("store_id", serde_json::Value::Number(id.into()));
    }
}

pub async fn run_with_timer<F, T>(key: &str, f: F) -> T
where
    F: std::future::Future<Output = T>,
{
    let start = Instant::now();
    let result = f.await;
    let elapsed = start.elapsed().as_millis() as u64;
    add_public_metadata(
        &format!("{}_timing_ms", key),
        serde_json::Value::Number(elapsed.into()),
    );
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_and_get_metadata() {
        add_public_metadata("test_key", serde_json::Value::String("test_val".into()));
        let meta = get_public_metadata();
        assert_eq!(
            meta.get("test_key").and_then(|v| v.as_str()),
            Some("test_val")
        );
    }

    #[test]
    fn records_store_fqdn_metadata() {
        record_store_fqdn_metadata("shop.myshopify.com", true, Some("99"));
        let public = get_public_metadata();
        let sensitive = get_sensitive_metadata();
        assert_eq!(
            sensitive.get("store_fqdn").and_then(|v| v.as_str()),
            Some("shop.myshopify.com")
        );
        assert_eq!(
            public.get("store_domain").and_then(|v| v.as_str()),
            Some("shop.myshopify.com")
        );
        assert_eq!(
            public.get("store_fqdn_validated").and_then(|v| v.as_bool()),
            Some(true)
        );
        assert_eq!(public.get("store_id").and_then(|v| v.as_i64()), Some(99));
        assert!(public.contains_key("store_fqdn_hash"));
    }

    #[tokio::test]
    async fn test_run_with_timer_adds_timing() {
        let result = run_with_timer("my_op", async {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            42
        })
        .await;
        assert_eq!(result, 42);
        let meta = get_public_metadata();
        assert!(meta.contains_key("my_op_timing_ms"));
    }
}
