use crate::constants::{HIDDEN_CONFIG_DIR, HIDDEN_PROJECT_FILE};
use crate::error::AppError;
use crate::models::app::AppHiddenConfig;
use crate::models::config_file_naming::is_valid_format_app_configuration_file_name;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProjectMeta {
    #[serde(default)]
    pub config_file: Option<String>,
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
        let directory = directory.to_path_buf();
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
        if let Some(name) = override_name {
            let path = self.directory.join(crate::models::config_file_naming::get_app_configuration_file_name(Some(name)));
            if path.exists() {
                return Some(path);
            }
        }
        if let Some(ref cached) = self.meta.config_file {
            let path = self.directory.join(cached);
            if path.exists() {
                return Some(path);
            }
        }
        self.config_files.first().cloned()
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
