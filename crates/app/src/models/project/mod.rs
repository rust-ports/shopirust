use crate::constants::{HIDDEN_CONFIG_DIR, HIDDEN_PROJECT_FILE};
use crate::error::AppError;
use crate::models::app::AppHiddenConfig;
use crate::models::config_file_naming::{
    get_app_configuration_file_name, is_valid_format_app_configuration_file_name,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProjectMeta {
    #[serde(default)]
    pub config_file: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigSource {
    Flag,
    Cached,
    Default,
}

#[derive(Debug, Clone)]
pub struct ActiveConfig {
    pub file: PathBuf,
    pub source: ConfigSource,
    pub dotenv: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct Project {
    pub directory: PathBuf,
    pub config_files: Vec<PathBuf>,
    pub hidden_config: AppHiddenConfig,
    pub meta: ProjectMeta,
}

impl Project {
    pub fn load(directory: &Path) -> Result<Self, AppError> {
        let directory = find_app_directory(directory).unwrap_or_else(|| directory.to_path_buf());
        let config_files = discover_config_files(&directory)?;
        let (hidden_config, meta) = load_hidden(&directory)?;
        Ok(Self {
            directory,
            config_files,
            hidden_config,
            meta,
        })
    }

    pub fn active_config_file(&self, override_name: Option<&str>) -> Option<PathBuf> {
        self.select_active_config(override_name).map(|c| c.file)
    }

    pub fn select_active_config(&self, override_name: Option<&str>) -> Option<ActiveConfig> {
        if let Some(name) = override_name {
            let path = self
                .directory
                .join(get_app_configuration_file_name(Some(name)));
            if path.exists() {
                return Some(ActiveConfig {
                    file: path.clone(),
                    source: ConfigSource::Flag,
                    dotenv: resolve_dotenv(&self.directory, &path),
                });
            }
        }
        if let Some(ref cached) = self.meta.config_file {
            let path = self.directory.join(cached);
            if path.exists() {
                return Some(ActiveConfig {
                    file: path.clone(),
                    source: ConfigSource::Cached,
                    dotenv: resolve_dotenv(&self.directory, &path),
                });
            }
        }
        let default = self
            .config_files
            .iter()
            .find(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n == "shopify.app.toml")
            })
            .cloned()
            .or_else(|| self.config_files.first().cloned())?;
        Some(ActiveConfig {
            file: default.clone(),
            source: ConfigSource::Default,
            dotenv: resolve_dotenv(&self.directory, &default),
        })
    }
}

/// Walk up from `start` until a directory containing `shopify.app*.toml` is found.
pub fn find_app_directory(start: &Path) -> Option<PathBuf> {
    let mut current = if start.is_file() {
        start.parent()?.to_path_buf()
    } else {
        start.to_path_buf()
    };
    loop {
        if discover_config_files(&current)
            .ok()
            .is_some_and(|f| !f.is_empty())
        {
            return Some(current);
        }
        if !current.pop() {
            return None;
        }
    }
}

fn discover_config_files(directory: &Path) -> Result<Vec<PathBuf>, AppError> {
    let mut files = Vec::new();
    if !directory.is_dir() {
        return Ok(files);
    }
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if is_valid_format_app_configuration_file_name(&name) {
            files.push(entry.path());
        }
    }
    files.sort();
    Ok(files)
}

fn load_hidden(directory: &Path) -> Result<(AppHiddenConfig, ProjectMeta), AppError> {
    let path = directory.join(HIDDEN_CONFIG_DIR).join(HIDDEN_PROJECT_FILE);
    if !path.exists() {
        return Ok((AppHiddenConfig::default(), ProjectMeta::default()));
    }
    let raw = fs::read_to_string(path)?;
    let value: serde_json::Value = serde_json::from_str(&raw)?;
    let hidden = AppHiddenConfig {
        dev_store_url: value
            .get("dev_store_url")
            .and_then(|v| v.as_str())
            .map(str::to_string),
    };
    let meta = ProjectMeta {
        config_file: value
            .get("configFile")
            .or_else(|| value.get("config_file"))
            .and_then(|v| v.as_str())
            .map(str::to_string),
    };
    Ok((hidden, meta))
}

/// `shopify.app.toml` → `.env`; `shopify.app.staging.toml` → `.env.staging` (no fallback).
pub fn resolve_dotenv(project_dir: &Path, active_config: &Path) -> Option<PathBuf> {
    let name = active_config.file_name()?.to_str()?;
    let env_name = if name == "shopify.app.toml" {
        ".env".to_string()
    } else {
        let slug = name
            .strip_prefix("shopify.app.")
            .and_then(|s| s.strip_suffix(".toml"))?;
        format!(".env.{slug}")
    };
    let path = project_dir.join(env_name);
    path.exists().then_some(path)
}

pub mod active_config {
    use super::*;

    pub fn select_active_config(
        project: &Project,
        config_name: Option<&str>,
    ) -> Result<PathBuf, AppError> {
        project
            .active_config_file(config_name)
            .ok_or_else(|| AppError::message("No app configuration file found in this directory"))
    }
}

pub mod config_selection {
    pub use super::resolve_dotenv;
    pub use super::ConfigSource;
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn walks_up_to_find_app_toml() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("shopify.app.toml"), "name = \"x\"\n").unwrap();
        let nested = dir.path().join("extensions/foo");
        fs::create_dir_all(&nested).unwrap();
        let found = find_app_directory(&nested).unwrap();
        assert_eq!(found, dir.path());
        let project = Project::load(&nested).unwrap();
        assert_eq!(project.directory, dir.path());
        assert_eq!(project.config_files.len(), 1);
    }

    #[test]
    fn flag_overrides_cached_config() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("shopify.app.toml"), "").unwrap();
        fs::write(dir.path().join("shopify.app.staging.toml"), "").unwrap();
        let mut project = Project::load(dir.path()).unwrap();
        project.meta.config_file = Some("shopify.app.staging.toml".into());
        let active = project.select_active_config(Some("production"));
        // production slug file doesn't exist; flag path missing → fall through to cached
        assert!(active.is_none() || active.unwrap().source != ConfigSource::Flag);

        let active = project
            .select_active_config(Some("shopify.app.staging.toml"))
            .unwrap();
        assert_eq!(active.source, ConfigSource::Flag);
        assert!(active.file.ends_with("shopify.app.staging.toml"));
    }

    #[test]
    fn prefers_shopify_app_toml_as_default() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("shopify.app.staging.toml"), "").unwrap();
        fs::write(dir.path().join("shopify.app.toml"), "").unwrap();
        let project = Project::load(dir.path()).unwrap();
        let active = project.select_active_config(None).unwrap();
        assert_eq!(active.source, ConfigSource::Default);
        assert!(active.file.ends_with("shopify.app.toml"));
    }

    #[test]
    fn dotenv_staging_does_not_fall_back() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join(".env"), "A=1").unwrap();
        let staging = dir.path().join("shopify.app.staging.toml");
        fs::write(&staging, "").unwrap();
        assert!(resolve_dotenv(dir.path(), &staging).is_none());
        fs::write(dir.path().join(".env.staging"), "A=2").unwrap();
        assert!(resolve_dotenv(dir.path(), &staging)
            .unwrap()
            .ends_with(".env.staging"));
    }

    #[test]
    fn cached_stale_falls_back_to_default() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("shopify.app.toml"), "").unwrap();
        let mut project = Project::load(dir.path()).unwrap();
        project.meta.config_file = Some("shopify.app.gone.toml".into());
        let active = project.select_active_config(None).unwrap();
        assert_eq!(active.source, ConfigSource::Default);
    }
}
