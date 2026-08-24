use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const CACHE_DIR: &str = "shopify-cli-kit";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry {
    value: serde_json::Value,
    timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Cache {
    #[serde(flatten)]
    entries: std::collections::HashMap<String, CacheEntry>,
}

#[derive(Debug)]
pub enum CacheError {
    Io(std::io::Error),
    Serde(serde_json::Error),
}

impl std::fmt::Display for CacheError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CacheError::Io(e) => write!(f, "cache I/O error: {e}"),
            CacheError::Serde(e) => write!(f, "cache serialization error: {e}"),
        }
    }
}

impl std::error::Error for CacheError {}

impl From<std::io::Error> for CacheError {
    fn from(e: std::io::Error) -> Self {
        CacheError::Io(e)
    }
}

impl From<serde_json::Error> for CacheError {
    fn from(e: serde_json::Error) -> Self {
        CacheError::Serde(e)
    }
}

/// A file-backed cache store with TTL support.
///
/// Data is persisted as JSON at `~/.config/shopify-cli-kit/{name}.json`.
///
/// Each entry stores a serialized value and an epoch-ms timestamp.
pub struct CacheStore {
    path: PathBuf,
}

impl CacheStore {
    /// Create a new cache store named `name`.
    ///
    /// The file is created at `~/.config/shopify-cli-kit/{name}.json`.
    pub fn new(name: &str) -> Self {
        let dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(CACHE_DIR);
        let path = dir.join(format!("{name}.json"));
        Self { path }
    }

    /// Create a cache store at an explicit path (for testing).
    pub fn with_path(path: PathBuf) -> Self {
        Self { path }
    }

    /// Compute a hex SHA-256 hash of the input string.
    pub fn key_hash(input: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(input.as_bytes());
        hex::encode(hasher.finalize())
    }

    /// Retrieve a cached value if it exists.
    pub fn retrieve<V: DeserializeOwned>(&self, key: &str) -> Result<Option<V>, CacheError> {
        let cache = self.load()?;
        let hashed = Self::key_hash(key);
        match cache.entries.get(&hashed) {
            Some(entry) => {
                let value: V = serde_json::from_value(entry.value.clone())?;
                Ok(Some(value))
            }
            None => Ok(None),
        }
    }

    /// Store a value in the cache.
    pub fn store<V: Serialize>(&self, key: &str, value: &V) -> Result<(), CacheError> {
        let mut cache = self.load()?;
        let hashed = Self::key_hash(key);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        cache.entries.insert(
            hashed,
            CacheEntry {
                value: serde_json::to_value(value)?,
                timestamp: now,
            },
        );
        self.save(&cache)
    }

    /// Retrieve a cached value if it exists and hasn't expired.
    /// If missing or expired, call `fetcher`, store the result, and return it.
    pub fn retrieve_or_repopulate<V, F>(
        &self,
        key: &str,
        ttl: Duration,
        fetcher: F,
    ) -> Result<V, CacheError>
    where
        V: DeserializeOwned + Serialize,
        F: FnOnce() -> Result<V, CacheError>,
    {
        let cache = self.load()?;
        let hashed = Self::key_hash(key);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        if let Some(entry) = cache.entries.get(&hashed) {
            if now - entry.timestamp < ttl.as_millis() as u64 {
                let value: V = serde_json::from_value(entry.value.clone())?;
                return Ok(value);
            }
        }

        let value = fetcher()?;
        self.store(key, &value)?;
        Ok(value)
    }

    /// Remove a specific key from the cache.
    pub fn remove(&self, key: &str) -> Result<(), CacheError> {
        let mut cache = self.load()?;
        let hashed = Self::key_hash(key);
        cache.entries.remove(&hashed);
        self.save(&cache)
    }

    /// Clear all entries from the cache.
    pub fn clear(&self) -> Result<(), CacheError> {
        if self.path.exists() {
            fs::remove_file(&self.path)?;
        }
        Ok(())
    }

    fn load(&self) -> Result<Cache, CacheError> {
        if !self.path.exists() {
            return Ok(Cache {
                entries: std::collections::HashMap::new(),
            });
        }
        let data = fs::read_to_string(&self.path)?;
        if data.trim().is_empty() {
            return Ok(Cache {
                entries: std::collections::HashMap::new(),
            });
        }
        Ok(serde_json::from_str(&data)?)
    }

    fn save(&self, cache: &Cache) -> Result<(), CacheError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let data = serde_json::to_string_pretty(cache)?;
        fs::write(&self.path, data)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    fn temp_store() -> (CacheStore, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test-cache.json");
        let store = CacheStore::with_path(path);
        (store, dir)
    }

    #[test]
    fn key_hash_is_deterministic() {
        let a = CacheStore::key_hash("hello world");
        let b = CacheStore::key_hash("hello world");
        assert_eq!(a, b);
    }

    #[test]
    fn different_keys_produce_different_hashes() {
        let a = CacheStore::key_hash("query1");
        let b = CacheStore::key_hash("query2");
        assert_ne!(a, b);
    }

    #[test]
    fn returns_none_on_miss() {
        let (store, _dir) = temp_store();
        let result: Result<Option<String>, CacheError> = store.retrieve("nonexistent");
        assert_eq!(result.unwrap(), None);
    }

    #[test]
    fn store_and_retrieve_round_trip() {
        let (store, _dir) = temp_store();
        store.store("my-key", &"my-value".to_string()).unwrap();
        let result: Result<Option<String>, CacheError> = store.retrieve("my-key");
        assert_eq!(result.unwrap(), Some("my-value".to_string()));
    }

    #[test]
    fn retrieve_or_repopulate_returns_cached_within_ttl() {
        let (store, _dir) = temp_store();
        let called = AtomicBool::new(false);

        let val: String = store
            .retrieve_or_repopulate("key", Duration::from_secs(60), || {
                called.store(true, Ordering::SeqCst);
                Ok("cached-value".to_string())
            })
            .unwrap();
        assert_eq!(val, "cached-value");
        assert!(called.load(Ordering::SeqCst));

        let called2 = AtomicBool::new(false);
        let val2: String = store
            .retrieve_or_repopulate("key", Duration::from_secs(60), || {
                called2.store(true, Ordering::SeqCst);
                Ok("fresh-value".to_string())
            })
            .unwrap();
        assert_eq!(val2, "cached-value");
        assert!(!called2.load(Ordering::SeqCst));
    }

    #[test]
    fn retrieve_or_repopulate_calls_fetcher_after_ttl_expiry() {
        let (store, _dir) = temp_store();

        let _: String = store
            .retrieve_or_repopulate("key", Duration::from_millis(0), || Ok("old".to_string()))
            .unwrap();

        let val: String = store
            .retrieve_or_repopulate("key", Duration::from_millis(0), || Ok("new".to_string()))
            .unwrap();
        assert_eq!(val, "new");
    }

    #[test]
    fn clear_removes_all_entries() {
        let (store, _dir) = temp_store();
        store.store("key1", &"val1".to_string()).unwrap();
        store.clear().unwrap();
        let result: Result<Option<String>, CacheError> = store.retrieve("key1");
        assert_eq!(result.unwrap(), None);
    }

    #[test]
    fn remove_specific_key() {
        let (store, _dir) = temp_store();
        store.store("key-a", &"value-a".to_string()).unwrap();
        store.store("key-b", &"value-b".to_string()).unwrap();
        store.remove("key-a").unwrap();

        let a: Result<Option<String>, CacheError> = store.retrieve("key-a");
        let b: Result<Option<String>, CacheError> = store.retrieve("key-b");
        assert_eq!(a.unwrap(), None);
        assert_eq!(b.unwrap(), Some("value-b".to_string()));
    }

    #[test]
    fn complex_value_round_trip() {
        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
        struct App {
            id: u64,
            name: String,
            tags: Vec<String>,
        }

        let (store, _dir) = temp_store();
        let app = App {
            id: 42,
            name: "my-app".into(),
            tags: vec!["cli".into(), "shopify".into()],
        };
        store.store("app-42", &app).unwrap();
        let retrieved: Result<Option<App>, CacheError> = store.retrieve("app-42");
        assert_eq!(retrieved.unwrap().unwrap(), app);
    }
}
