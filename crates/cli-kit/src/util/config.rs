use crate::util::crypto;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkedProject {
    pub project_id: String,
    pub organization_id: Option<String>,
    pub environment_id: String,
    pub service_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    pub linked_project: Option<LinkedProject>,
    pub scopes: Option<String>,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CliConfig {
    pub apps: HashMap<String, AppConfig>,
    pub last_user_id: Option<String>,
}

impl CliConfig {
    fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("shopify")
            .join("cli")
            .join("config.json")
    }

    pub fn load() -> Result<Self, std::io::Error> {
        let path = Self::config_path();
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(&path)?;
        serde_json::from_str(&content)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }

    pub fn save(&self) -> Result<(), std::io::Error> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, content)
    }

    pub fn get_app(&self, dir: &Path) -> Option<&AppConfig> {
        let key = canonical_path_key(dir);
        self.apps.get(&key)
    }

    pub fn set_app(&mut self, dir: &Path, config: AppConfig) {
        let key = canonical_path_key(dir);
        self.apps.insert(key, config);
    }

    pub fn remove_app(&mut self, dir: &Path) {
        let key = canonical_path_key(dir);
        self.apps.remove(&key);
    }
}

fn canonical_path_key(dir: &Path) -> String {
    match std::fs::canonicalize(dir) {
        Ok(p) => crypto::hash_string(&p.to_string_lossy()),
        Err(_) => crypto::hash_string(&dir.to_string_lossy()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_cli_config_default() {
        let config = CliConfig::default();
        assert!(config.apps.is_empty());
        assert!(config.last_user_id.is_none());
    }

    #[test]
    fn test_set_and_get_app() {
        let dir = TempDir::new().unwrap();
        let mut config = CliConfig::default();
        let app = AppConfig {
            linked_project: Some(LinkedProject {
                project_id: "proj_123".into(),
                organization_id: Some("org_456".into()),
                environment_id: "env_789".into(),
                service_id: None,
            }),
            scopes: Some("write_products".into()),
            name: Some("my-app".into()),
        };
        config.set_app(dir.path(), app);
        let retrieved = config.get_app(dir.path());
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name.as_deref(), Some("my-app"));
    }

    #[test]
    fn test_remove_app() {
        let dir = TempDir::new().unwrap();
        let mut config = CliConfig::default();
        config.set_app(dir.path(), AppConfig::default());
        assert!(config.get_app(dir.path()).is_some());
        config.remove_app(dir.path());
        assert!(config.get_app(dir.path()).is_none());
    }

    #[test]
    fn test_save_and_load_roundtrip() {
        let config = CliConfig {
            last_user_id: Some("user_1".into()),
            ..Default::default()
        };
        config.save().unwrap();
        let loaded = CliConfig::load().unwrap();
        assert!(loaded.last_user_id.is_some());
    }
}
