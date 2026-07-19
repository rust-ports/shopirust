use crate::util::crypto::non_random_uuid;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// A human-friendly time interval with optional days/hours/minutes/seconds.
///
/// Used as a parameter type for cache TTLs, rate-limit windows, etc.
#[derive(Debug, Clone, Copy, Default)]
pub struct TimeInterval {
    pub days: Option<u64>,
    pub hours: Option<u64>,
    pub minutes: Option<u64>,
    pub seconds: Option<u64>,
}

impl TimeInterval {
    /// Create a [`TimeInterval`] from a number of seconds.
    pub fn from_secs(secs: u64) -> Self {
        Self {
            seconds: Some(secs),
            ..Default::default()
        }
    }

    /// Create a [`TimeInterval`] from a number of minutes.
    pub fn from_mins(mins: u64) -> Self {
        Self {
            minutes: Some(mins),
            ..Default::default()
        }
    }

    /// Create a [`TimeInterval`] from a number of hours.
    pub fn from_hours(hours: u64) -> Self {
        Self {
            hours: Some(hours),
            ..Default::default()
        }
    }
}

/// Convert a [`TimeInterval`] to its equivalent in milliseconds.
pub fn time_interval_to_ms(interval: TimeInterval) -> u64 {
    let total_secs = interval.days.unwrap_or(0) * 86400
        + interval.hours.unwrap_or(0) * 3600
        + interval.minutes.unwrap_or(0) * 60
        + interval.seconds.unwrap_or(0);
    total_secs * 1000
}

/// Monotonic milliseconds since UNIX epoch (used for cache expiry).
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

// ── Sliding-window rate limiter ──────────────────────────────────────

/// Sliding-window rate limiter that caps disk-write frequency.
///
/// Protects the config file from being written too many times per second
/// (default: 100 ops/sec window). Callers `acquire()` before writing and
/// sleep for the returned duration if the rate limit is exceeded.
#[derive(Debug)]
pub struct IoRateLimiter {
    max_ops: usize,
    window_ms: u64,
    timestamps: Mutex<Vec<u64>>,
}

impl IoRateLimiter {
    /// Create a limiter allowing at most `max_ops` per `window_ms`.
    pub fn new(max_ops: usize, window_ms: u64) -> Self {
        Self {
            max_ops,
            window_ms,
            timestamps: Mutex::new(Vec::new()),
        }
    }

    /// Check if the operation is within the rate limit.
    ///
    /// Returns `Some(wait_ms)` if the caller should sleep before proceeding
    /// to stay under the limit, or `None` if the operation can proceed.
    pub fn acquire(&self) -> Option<u64> {
        let mut timestamps = self.timestamps.lock().unwrap();
        let now = now_ms();
        let window_start = now.saturating_sub(self.window_ms);

        timestamps.retain(|&t| t > window_start);

        if timestamps.len() < self.max_ops {
            timestamps.push(now);
            None
        } else {
            let oldest = timestamps[0];
            let wait = self.window_ms.saturating_sub(now.saturating_sub(oldest));
            Some(wait)
        }
    }
}

impl Default for IoRateLimiter {
    fn default() -> Self {
        Self::new(100, 1000)
    }
}

// ── Cache entry ──────────────────────────────────────────────────────

/// A single cache entry with a value and insertion timestamp.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry {
    value: serde_json::Value,
    timestamp: u64,
}

/// Flat key-value map of [`CacheEntry`] items stored inside [`ConfSchema`].
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CacheData {
    #[serde(flatten)]
    entries: HashMap<String, CacheEntry>,
}

// ── ConfSchema ───────────────────────────────────────────────────────

/// Root shape of the CLI config JSON file.
///
/// Persisted at `~/.config/shopify-cli-{project}/config.json`.
/// Mirrors the upstream `ConfSchema` pattern: session stores, cache, and
/// pending device-auth records live under well-known top-level keys while
/// arbitrary cache entries are stored in the `cache` sub-object.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConfSchema {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_store: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dev_session_store: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_dev_session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache: Option<CacheData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending_device_auth: Option<PendingDeviceAuth>,
}

/// A partially-completed device-authorization flow.
///
/// Stored so the CLI can resume polling after a restart or crash before
/// the user completes the browser-based identity handshake.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingDeviceAuth {
    pub device_code: String,
    pub interval: u64,
    pub expires_at: u64,
    pub verification_uri_complete: String,
    pub scopes: Vec<String>,
}

// ── LocalStorage ─────────────────────────────────────────────────────

/// File-backed, strongly-typed persistent config store.
///
/// Wraps a JSON file on disk (one per project) and exposes a
/// `get`/`set`/`delete`/`clear` API. Writes are rate-limited via
/// [`IoRateLimiter`] to avoid hammering the disk during rapid sequential
/// writes (e.g., token refresh loops).
///
/// Distinguished keys (`"session_store"`, `"current_session_id"`, etc.)
/// map to first-class [`ConfSchema`] fields. Everything else goes into
/// the `cache` sub-object.
#[derive(Debug)]
pub struct LocalStorage {
    path: PathBuf,
    rate_limiter: IoRateLimiter,
}

impl LocalStorage {
    /// Create a [`LocalStorage`] under `~/.config/shopify-cli-{project_name}/config.json`.
    ///
    /// Each project gets its own directory so session and cache data for
    /// different Shopify apps don't collide.
    pub fn new(project_name: &str) -> Self {
        let dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(format!("shopify-cli-{project_name}"));
        let path = dir.join("config.json");
        Self {
            path,
            rate_limiter: IoRateLimiter::default(),
        }
    }

    /// Create a [`LocalStorage`] at an explicit file path (useful in tests).
    pub fn with_path(path: PathBuf) -> Self {
        Self {
            path,
            rate_limiter: IoRateLimiter::default(),
        }
    }

    /// The on-disk path of the config file.
    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    /// Deserialise the value stored at `key`.
    ///
    /// Returns `None` if the key doesn't exist or the stored value can't
    /// be deserialised into `T`.
    pub fn get<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        let schema = self.load_schema().unwrap_or_default();
        Self::extract_value(&schema, key)
    }

    /// Serialise `value` and store it at `key`.
    ///
    /// Respects the [`IoRateLimiter`] — may block the calling thread for
    /// a few milliseconds if the write rate exceeds 100 ops/sec.
    pub fn set<T: Serialize>(&self, key: &str, value: &T) {
        if let Some(wait) = self.rate_limiter.acquire() {
            std::thread::sleep(Duration::from_millis(wait));
        }
        let mut schema = self.load_schema().unwrap_or_default();
        Self::inject_value(&mut schema, key, value);
        let _ = self.save_schema(&schema);
    }

    /// Remove a key from the store.
    pub fn delete(&self, key: &str) {
        let mut schema = self.load_schema().unwrap_or_default();
        Self::remove_key(&mut schema, key);
        let _ = self.save_schema(&schema);
    }

    /// Delete the entire config file from disk.
    pub fn clear(&self) {
        let _ = fs::remove_file(&self.path);
    }

    // ── Internal helpers ──────────────────────────────────────────

    fn load_schema(&self) -> Result<ConfSchema, std::io::Error> {
        if !self.path.exists() {
            return Ok(ConfSchema::default());
        }
        let data = fs::read_to_string(&self.path)?;
        if data.trim().is_empty() {
            return Ok(ConfSchema::default());
        }
        serde_json::from_str(&data)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }

    fn save_schema(&self, schema: &ConfSchema) -> Result<(), std::io::Error> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let data = serde_json::to_string_pretty(schema)?;
        fs::write(&self.path, data)?;
        Ok(())
    }

    fn extract_value<T: DeserializeOwned>(schema: &ConfSchema, key: &str) -> Option<T> {
        let value = match key {
            "session_store" => schema.session_store.as_ref()?,
            "current_session_id" => {
                return schema
                    .current_session_id
                    .clone()
                    .and_then(|s| serde_json::from_value(serde_json::Value::String(s)).ok())
            }
            "dev_session_store" => schema.dev_session_store.as_ref()?,
            "current_dev_session_id" => {
                return schema
                    .current_dev_session_id
                    .clone()
                    .and_then(|s| serde_json::from_value(serde_json::Value::String(s)).ok())
            }
            "pending_device_auth" => {
                return serde_json::to_value(&schema.pending_device_auth)
                    .ok()
                    .and_then(|v| serde_json::from_value(v).ok())
            }
            _ => {
                if let Some(ref cache) = schema.cache {
                    let entry = cache.entries.get(key)?;
                    return serde_json::from_value(entry.value.clone()).ok();
                }
                return None;
            }
        };
        serde_json::from_value(value.clone()).ok()
    }

    fn inject_value<T: Serialize>(schema: &mut ConfSchema, key: &str, value: &T) {
        let json_value = serde_json::to_value(value).unwrap_or(serde_json::Value::Null);
        match key {
            "session_store" => schema.session_store = Some(json_value),
            "current_session_id" => {
                schema.current_session_id = json_value.as_str().map(String::from)
            }
            "dev_session_store" => schema.dev_session_store = Some(json_value),
            "current_dev_session_id" => {
                schema.current_dev_session_id = json_value.as_str().map(String::from)
            }
            "pending_device_auth" => {
                schema.pending_device_auth = serde_json::from_value(json_value).ok()
            }
            _ => {
                let cache = schema.cache.get_or_insert_with(CacheData::default);
                cache.entries.insert(
                    key.to_string(),
                    CacheEntry {
                        value: json_value,
                        timestamp: now_ms(),
                    },
                );
            }
        }
    }

    fn remove_key(schema: &mut ConfSchema, key: &str) {
        match key {
            "session_store" => schema.session_store = None,
            "current_session_id" => schema.current_session_id = None,
            "dev_session_store" => schema.dev_session_store = None,
            "current_dev_session_id" => schema.current_dev_session_id = None,
            "pending_device_auth" => schema.pending_device_auth = None,
            _ => {
                if let Some(ref mut cache) = schema.cache {
                    cache.entries.remove(key);
                }
            }
        }
    }
}

// ── Cache-aside operations ──────────────────────────────────────────

/// Type alias for GraphQL request cache keys.
pub type GraphQLRequestKey = String;

/// Produce a deterministic composite cache key from several string parts.
///
/// Uses [`non_random_uuid`] (a UUID v5-style deterministic hash) so the
/// same inputs always produce the same key. Upstream equivalent:
/// `nonRandomUUID(parts.join("-"))`.
pub fn composite_cache_key(parts: &[&str]) -> String {
    let joined = parts.join("-");
    non_random_uuid(&joined)
}

/// Read from cache, or call `fetcher` to populate it.
///
/// If `store` is `None`, a static default [`LocalStorage`] for
/// `"shopify-cli"` is used automatically.
///
/// TTL is checked via the entry timestamp stored alongside the value.
pub fn cache_retrieve_or_repopulate<V, F>(
    key: &str,
    ttl_ms: u64,
    fetcher: F,
    store: Option<&LocalStorage>,
) -> V
where
    V: DeserializeOwned + Serialize,
    F: FnOnce() -> V,
{
    let storage = store.unwrap_or_else(|| {
        static STORE: std::sync::OnceLock<LocalStorage> = std::sync::OnceLock::new();
        STORE.get_or_init(|| LocalStorage::new("shopify-cli"))
    });

    if let Some(cached) = storage.get::<serde_json::Value>(key) {
        if let Ok(entry) = serde_json::from_value::<CacheEntry>(cached.clone()) {
            if now_ms() - entry.timestamp < ttl_ms {
                if let Ok(val) = serde_json::from_value(entry.value) {
                    return val;
                }
            }
        }
    }

    let value = fetcher();
    let entry = CacheEntry {
        value: serde_json::to_value(&value).unwrap_or(serde_json::Value::Null),
        timestamp: now_ms(),
    };
    storage.set(key, &entry);
    value
}

/// Store a string value in the cache section.
pub fn cache_store(key: &str, value: &str, store: Option<&LocalStorage>) {
    let storage = store.unwrap_or_else(|| {
        static STORE: std::sync::OnceLock<LocalStorage> = std::sync::OnceLock::new();
        STORE.get_or_init(|| LocalStorage::new("shopify-cli"))
    });
    let entry = CacheEntry {
        value: serde_json::Value::String(value.to_string()),
        timestamp: now_ms(),
    };
    storage.set(key, &entry);
}

/// Retrieve a string value from the cache section.
pub fn cache_retrieve(key: &str, store: Option<&LocalStorage>) -> Option<String> {
    let storage = store.unwrap_or_else(|| {
        static STORE: std::sync::OnceLock<LocalStorage> = std::sync::OnceLock::new();
        STORE.get_or_init(|| LocalStorage::new("shopify-cli"))
    });
    let entry: CacheEntry = storage.get(key)?;
    match entry.value {
        serde_json::Value::String(s) => Some(s),
        _ => serde_json::to_string(&entry.value).ok(),
    }
}

/// Delete all entries from the cache section.
pub fn cache_clear(store: Option<&LocalStorage>) {
    let storage = store.unwrap_or_else(|| {
        static STORE: std::sync::OnceLock<LocalStorage> = std::sync::OnceLock::new();
        STORE.get_or_init(|| LocalStorage::new("shopify-cli"))
    });
    storage.delete("cache");
}

// ── Session persistence ─────────────────────────────────────────────

/// Read the serialised session store from the config file.
pub fn get_session(store: &LocalStorage) -> Option<String> {
    store.get::<String>("session_store")
}

/// Write serialised session data to the config file.
pub fn set_session(json: &str, store: &LocalStorage) {
    store.set("session_store", &json.to_string());
}

/// Remove the session store from the config file.
pub fn remove_session(store: &LocalStorage) {
    store.delete("session_store");
}

/// Read the currently-active session identifier.
pub fn get_current_session_id(store: &LocalStorage) -> Option<String> {
    store.get::<String>("current_session_id")
}

/// Persist the currently-active session identifier.
pub fn set_current_session_id(id: &str, store: &LocalStorage) {
    store.set("current_session_id", &id.to_string());
}

/// Clear the currently-active session identifier.
pub fn remove_current_session_id(store: &LocalStorage) {
    store.delete("current_session_id");
}

// ── Most-recent-occurrence ──────────────────────────────────────────

/// Execute a task only if the last recorded execution is older than `timeout_ms`.
///
/// Wraps [`cache_retrieve_or_repopulate`] with a namespaced key
/// `"most-recent-occurrence::{key}"`.
pub fn most_recent_occurrence<V, F>(
    key: &str,
    timeout_ms: u64,
    task: F,
    store: Option<&LocalStorage>,
) -> V
where
    V: DeserializeOwned + Serialize,
    F: FnOnce() -> V,
{
    cache_retrieve_or_repopulate(
        &format!("most-recent-occurrence::{key}"),
        timeout_ms,
        task,
        store,
    )
}

// ── Pending device auth ─────────────────────────────────────────────

/// Read the saved device-authorization state.
pub fn get_pending_device_auth(store: &LocalStorage) -> Option<PendingDeviceAuth> {
    store.get::<PendingDeviceAuth>("pending_device_auth")
}

/// Save device-authorization state for resumption after restart.
pub fn set_pending_device_auth(auth: &PendingDeviceAuth, store: &LocalStorage) {
    store.set("pending_device_auth", auth);
}

/// Remove any saved device-authorization state.
pub fn clear_pending_device_auth(store: &LocalStorage) {
    store.delete("pending_device_auth");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_interval_to_ms() {
        assert_eq!(time_interval_to_ms(TimeInterval::from_secs(5)), 5000);
        assert_eq!(time_interval_to_ms(TimeInterval::from_mins(2)), 120000);
        assert_eq!(time_interval_to_ms(TimeInterval::from_hours(1)), 3600000);
        assert_eq!(
            time_interval_to_ms(TimeInterval {
                days: Some(1),
                hours: Some(1),
                minutes: Some(1),
                seconds: Some(1),
            }),
            90061000
        );
    }

    #[test]
    fn test_rate_limiter_allows_under_limit() {
        let limiter = IoRateLimiter::new(5, 1000);
        assert!(limiter.acquire().is_none());
        assert!(limiter.acquire().is_none());
    }

    #[test]
    fn test_local_storage_set_get() {
        let dir = tempfile::tempdir().unwrap();
        let store = LocalStorage::with_path(dir.path().join("config.json"));
        store.set("name", &"test-value".to_string());
        let val: Option<String> = store.get("name");
        assert_eq!(val, Some("test-value".to_string()));
    }

    #[test]
    fn test_local_storage_get_missing() {
        let dir = tempfile::tempdir().unwrap();
        let store = LocalStorage::with_path(dir.path().join("config.json"));
        let val: Option<String> = store.get("nonexistent");
        assert_eq!(val, None);
    }

    #[test]
    fn test_local_storage_delete() {
        let dir = tempfile::tempdir().unwrap();
        let store = LocalStorage::with_path(dir.path().join("config.json"));
        store.set("key", &"value".to_string());
        store.delete("key");
        let val: Option<String> = store.get("key");
        assert_eq!(val, None);
    }

    #[test]
    fn test_local_storage_clear() {
        let dir = tempfile::tempdir().unwrap();
        let store = LocalStorage::with_path(dir.path().join("config.json"));
        store.set("key1", &"val1".to_string());
        store.set("key2", &"val2".to_string());
        store.clear();
        let val1: Option<String> = store.get("key1");
        assert_eq!(val1, None);
    }

    #[test]
    fn test_local_storage_path() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.json");
        let store = LocalStorage::with_path(path.clone());
        assert_eq!(store.path(), &path);
    }

    #[test]
    fn test_session_persistence() {
        let dir = tempfile::tempdir().unwrap();
        let store = LocalStorage::with_path(dir.path().join("config.json"));

        assert!(get_session(&store).is_none());

        set_session(r#"{"test": "data"}"#, &store);
        assert_eq!(get_session(&store), Some(r#"{"test": "data"}"#.to_string()));

        remove_session(&store);
        assert!(get_session(&store).is_none());
    }

    #[test]
    fn test_current_session_id() {
        let dir = tempfile::tempdir().unwrap();
        let store = LocalStorage::with_path(dir.path().join("config.json"));

        assert!(get_current_session_id(&store).is_none());

        set_current_session_id("session-123", &store);
        assert_eq!(
            get_current_session_id(&store),
            Some("session-123".to_string())
        );

        remove_current_session_id(&store);
        assert!(get_current_session_id(&store).is_none());
    }

    #[test]
    fn test_pending_device_auth() {
        let dir = tempfile::tempdir().unwrap();
        let store = LocalStorage::with_path(dir.path().join("config.json"));

        assert!(get_pending_device_auth(&store).is_none());

        let auth = PendingDeviceAuth {
            device_code: "code-123".into(),
            interval: 5,
            expires_at: 9999999999,
            verification_uri_complete: "https://example.com".into(),
            scopes: vec!["scope1".into()],
        };
        set_pending_device_auth(&auth, &store);

        let retrieved = get_pending_device_auth(&store).unwrap();
        assert_eq!(retrieved.device_code, "code-123");
        assert_eq!(retrieved.interval, 5);

        clear_pending_device_auth(&store);
        assert!(get_pending_device_auth(&store).is_none());
    }

    #[test]
    fn test_cache_retrieve_or_repopulate() {
        let dir = tempfile::tempdir().unwrap();
        let store = LocalStorage::with_path(dir.path().join("config.json"));

        let val: String = cache_retrieve_or_repopulate(
            "test-key",
            60000,
            || "fresh-value".to_string(),
            Some(&store),
        );
        assert_eq!(val, "fresh-value");

        let val2: String = cache_retrieve_or_repopulate(
            "test-key",
            60000,
            || "should-not-be-called".to_string(),
            Some(&store),
        );
        assert_eq!(val2, "fresh-value");
    }

    #[test]
    fn test_cache_retrieve_or_repopulate_expired() {
        let dir = tempfile::tempdir().unwrap();
        let store = LocalStorage::with_path(dir.path().join("config.json"));

        let _: String = cache_retrieve_or_repopulate(
            "exp-key",
            0,
            || "old".to_string(),
            Some(&store),
        );

        let val: String = cache_retrieve_or_repopulate(
            "exp-key",
            0,
            || "new".to_string(),
            Some(&store),
        );
        assert_eq!(val, "new");
    }

    #[test]
    fn test_composite_cache_key() {
        let key1 = composite_cache_key(&["query", "vars", "v1"]);
        let key2 = composite_cache_key(&["query", "vars", "v1"]);
        assert_eq!(key1, key2);

        let key3 = composite_cache_key(&["query", "vars", "v2"]);
        assert_ne!(key1, key3);
    }

    #[test]
    fn test_local_storage_new_project_name() {
        let dir = tempfile::tempdir().unwrap();
        let store = LocalStorage::with_path(dir.path().join("shopify-cli-test/config.json"));
        store.set("test", &42u64);
        let val: Option<u64> = store.get("test");
        assert_eq!(val, Some(42));
    }
}
