use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

const THEME_STORE_KEY: &str = "themeStore";

#[derive(Debug, Error)]
pub enum LocalStorageError {
    #[error("Unable to read theme local storage {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("Unable to parse theme local storage {path}: {source}")]
    Parse {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("Unable to write theme local storage {path}: {source}")]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("Unable to serialize theme local storage: {0}")]
    Serialize(serde_json::Error),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct StoreData {
    #[serde(flatten)]
    values: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct ThemeLocalStorage {
    theme_store_path: PathBuf,
    development_theme_path: PathBuf,
    host_theme_path: PathBuf,
}

impl Default for ThemeLocalStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl ThemeLocalStorage {
    pub fn new() -> Self {
        let config_dir = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        Self {
            theme_store_path: local_storage_path(&config_dir, "shopify-cli-theme-conf"),
            development_theme_path: local_storage_path(
                &config_dir,
                "shopify-cli-development-theme-config",
            ),
            host_theme_path: local_storage_path(&config_dir, "shopify-cli-host-theme-conf"),
        }
    }

    pub fn with_path(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        Self {
            theme_store_path: path.join("shopify-cli-theme-conf.json"),
            development_theme_path: path.join("shopify-cli-development-theme-config.json"),
            host_theme_path: path.join("shopify-cli-host-theme-conf.json"),
        }
    }

    pub fn path(&self) -> &Path {
        &self.theme_store_path
    }

    pub fn current_theme_store(&self) -> Result<Option<String>, LocalStorageError> {
        get(&self.theme_store_path, THEME_STORE_KEY)
    }

    pub fn store_current_theme_store(&self, store: &str) -> Result<(), LocalStorageError> {
        set(&self.theme_store_path, THEME_STORE_KEY, &store.to_string())
    }

    pub fn remove_current_theme_store(&self) -> Result<(), LocalStorageError> {
        remove(&self.theme_store_path, THEME_STORE_KEY)
    }

    pub fn development_theme_id(&self) -> Result<Option<i64>, LocalStorageError> {
        let Some(store) = self.current_theme_store()? else {
            return Ok(None);
        };
        self.development_theme_id_for_store(&store)
    }

    pub fn development_theme_id_for_store(
        &self,
        store: &str,
    ) -> Result<Option<i64>, LocalStorageError> {
        get_string_i64(&self.development_theme_path, store)
    }

    pub fn store_development_theme_id(&self, theme_id: i64) -> Result<(), LocalStorageError> {
        let Some(store) = self.current_theme_store()? else {
            return Ok(());
        };
        self.store_development_theme_id_for_store(&store, theme_id)
    }

    pub fn store_development_theme_id_for_store(
        &self,
        store: &str,
        theme_id: i64,
    ) -> Result<(), LocalStorageError> {
        set(&self.development_theme_path, store, &theme_id.to_string())
    }

    pub fn remove_development_theme_id(&self) -> Result<(), LocalStorageError> {
        let Some(store) = self.current_theme_store()? else {
            return Ok(());
        };
        self.remove_development_theme_id_for_store(&store)
    }

    pub fn remove_development_theme_id_for_store(
        &self,
        store: &str,
    ) -> Result<(), LocalStorageError> {
        remove(&self.development_theme_path, store)
    }

    pub fn host_theme_id(&self, store: &str) -> Result<Option<i64>, LocalStorageError> {
        get_string_i64(&self.host_theme_path, store)
    }

    pub fn store_host_theme_id(&self, store: &str, theme_id: i64) -> Result<(), LocalStorageError> {
        set(&self.host_theme_path, store, &theme_id.to_string())
    }

    pub fn remove_host_theme_id(&self, store: &str) -> Result<(), LocalStorageError> {
        remove(&self.host_theme_path, store)
    }
}

fn local_storage_path(config_dir: &Path, project_name: &str) -> PathBuf {
    config_dir.join(project_name).join("config.json")
}

fn get<T: serde::de::DeserializeOwned>(
    path: &Path,
    key: &str,
) -> Result<Option<T>, LocalStorageError> {
    let data = load(path)?;
    Ok(data
        .values
        .get(key)
        .cloned()
        .and_then(|value| serde_json::from_value(value).ok()))
}

fn get_string_i64(path: &Path, key: &str) -> Result<Option<i64>, LocalStorageError> {
    Ok(get::<String>(path, key)?.and_then(|value| value.parse().ok()))
}

fn set<T: Serialize>(path: &Path, key: &str, value: &T) -> Result<(), LocalStorageError> {
    let mut data = load(path)?;
    data.values.insert(
        key.to_string(),
        serde_json::to_value(value).map_err(LocalStorageError::Serialize)?,
    );
    save(path, &data)
}

fn remove(path: &Path, key: &str) -> Result<(), LocalStorageError> {
    let mut data = load(path)?;
    data.values.remove(key);
    save(path, &data)
}

fn load(path: &Path) -> Result<StoreData, LocalStorageError> {
    if !path.exists() {
        return Ok(StoreData::default());
    }
    let content = fs::read_to_string(path).map_err(|source| LocalStorageError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    if content.trim().is_empty() {
        return Ok(StoreData::default());
    }
    serde_json::from_str(&content).map_err(|source| LocalStorageError::Parse {
        path: path.to_path_buf(),
        source,
    })
}

fn save(path: &Path, data: &StoreData) -> Result<(), LocalStorageError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| LocalStorageError::Write {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let content = serde_json::to_string_pretty(data).map_err(LocalStorageError::Serialize)?;
    fs::write(path, content).map_err(|source| LocalStorageError::Write {
        path: path.to_path_buf(),
        source,
    })
}

pub fn current_theme_store() -> Option<String> {
    ThemeLocalStorage::new()
        .current_theme_store()
        .ok()
        .flatten()
}

pub fn store_current_theme_store(store: &str) {
    let _ = ThemeLocalStorage::new().store_current_theme_store(store);
}

pub fn remove_current_theme_store() {
    let _ = ThemeLocalStorage::new().remove_current_theme_store();
}

pub fn development_theme_id() -> Option<i64> {
    ThemeLocalStorage::new()
        .development_theme_id()
        .ok()
        .flatten()
}

pub fn development_theme_id_for_store(store: &str) -> Option<i64> {
    ThemeLocalStorage::new()
        .development_theme_id_for_store(store)
        .ok()
        .flatten()
}

pub fn store_development_theme_id(theme_id: i64) {
    let _ = ThemeLocalStorage::new().store_development_theme_id(theme_id);
}

pub fn store_development_theme_id_for_store(store: &str, theme_id: i64) {
    let _ = ThemeLocalStorage::new().store_development_theme_id_for_store(store, theme_id);
}

pub fn remove_development_theme_id() {
    let _ = ThemeLocalStorage::new().remove_development_theme_id();
}

pub fn remove_development_theme_id_for_store(store: &str) {
    let _ = ThemeLocalStorage::new().remove_development_theme_id_for_store(store);
}

pub fn host_theme_id(store: &str) -> Option<i64> {
    ThemeLocalStorage::new().host_theme_id(store).ok().flatten()
}

pub fn store_host_theme_id(store: &str, theme_id: i64) {
    let _ = ThemeLocalStorage::new().store_host_theme_id(store, theme_id);
}

pub fn remove_host_theme_id(store: &str) {
    let _ = ThemeLocalStorage::new().remove_host_theme_id(store);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stores_reads_and_removes_current_theme_store() {
        let temp = tempfile::tempdir().unwrap();
        let storage = ThemeLocalStorage::with_path(temp.path());

        storage
            .store_current_theme_store("shop.myshopify.com")
            .unwrap();
        assert_eq!(
            storage.current_theme_store().unwrap().as_deref(),
            Some("shop.myshopify.com")
        );

        storage.remove_current_theme_store().unwrap();
        assert_eq!(storage.current_theme_store().unwrap(), None);
    }

    #[test]
    fn stores_reads_and_removes_development_theme_id() {
        let temp = tempfile::tempdir().unwrap();
        let storage = ThemeLocalStorage::with_path(temp.path());

        storage
            .store_current_theme_store("shop.myshopify.com")
            .unwrap();
        storage.store_development_theme_id(123).unwrap();
        assert_eq!(storage.development_theme_id().unwrap(), Some(123));
        assert_eq!(
            storage
                .development_theme_id_for_store("shop.myshopify.com")
                .unwrap(),
            Some(123)
        );

        storage.remove_development_theme_id().unwrap();
        assert_eq!(storage.development_theme_id().unwrap(), None);
    }

    #[test]
    fn stores_host_theme_ids_by_store() {
        let temp = tempfile::tempdir().unwrap();
        let storage = ThemeLocalStorage::with_path(temp.path());

        storage
            .store_host_theme_id("one.myshopify.com", 111)
            .unwrap();
        storage
            .store_host_theme_id("two.myshopify.com", 222)
            .unwrap();

        assert_eq!(
            storage.host_theme_id("one.myshopify.com").unwrap(),
            Some(111)
        );
        assert_eq!(
            storage.host_theme_id("two.myshopify.com").unwrap(),
            Some(222)
        );

        storage.remove_host_theme_id("one.myshopify.com").unwrap();
        assert_eq!(storage.host_theme_id("one.myshopify.com").unwrap(), None);
        assert_eq!(
            storage.host_theme_id("two.myshopify.com").unwrap(),
            Some(222)
        );
    }
}
