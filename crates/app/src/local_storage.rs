use crate::constants::{HIDDEN_CONFIG_DIR, HIDDEN_PROJECT_FILE};
use crate::error::AppError;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CachedAppInfo {
    pub directory: String,
    pub config_file: Option<String>,
    pub app_id: Option<String>,
    pub title: Option<String>,
    pub org_id: Option<String>,
    pub store_fqdn: Option<String>,
    pub update_urls: Option<bool>,
    pub previous_app_id: Option<String>,
}

fn cache_path(directory: &Path) -> PathBuf {
    directory.join(HIDDEN_CONFIG_DIR).join(HIDDEN_PROJECT_FILE)
}

pub fn get_cached_app_info(directory: &Path) -> Option<CachedAppInfo> {
    let path = cache_path(directory);
    let raw = fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

pub fn set_cached_app_info(info: &CachedAppInfo) -> Result<(), AppError> {
    let dir = PathBuf::from(&info.directory);
    let shopify_dir = dir.join(HIDDEN_CONFIG_DIR);
    fs::create_dir_all(&shopify_dir)?;
    let path = shopify_dir.join(HIDDEN_PROJECT_FILE);

    let mut merged = get_cached_app_info(&dir).unwrap_or_default();
    merged.directory = info.directory.clone();
    // Always take the caller's config_file value (including explicit None clears).
    merged.config_file = info.config_file.clone();
    if info.app_id.is_some() {
        merged.app_id = info.app_id.clone();
    }
    if info.title.is_some() {
        merged.title = info.title.clone();
    }
    if info.org_id.is_some() {
        merged.org_id = info.org_id.clone();
    }
    if info.store_fqdn.is_some() {
        merged.store_fqdn = info.store_fqdn.clone();
    }
    if info.update_urls.is_some() {
        merged.update_urls = info.update_urls;
    }
    if info.previous_app_id.is_some() {
        merged.previous_app_id = info.previous_app_id.clone();
    }

    let json = serde_json::to_string_pretty(&merged)?;
    fs::write(path, json)?;
    Ok(())
}

pub fn clear_current_config_file(directory: &Path) -> Result<(), AppError> {
    let mut info = get_cached_app_info(directory).unwrap_or(CachedAppInfo {
        directory: directory.display().to_string(),
        ..Default::default()
    });
    info.directory = directory.display().to_string();
    info.config_file = None;
    set_cached_app_info(&info)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn cache_round_trip() {
        let dir = tempdir().unwrap();
        set_cached_app_info(&CachedAppInfo {
            directory: dir.path().display().to_string(),
            config_file: Some("shopify.app.prod.toml".into()),
            app_id: Some("1".into()),
            title: Some("Demo".into()),
            org_id: None,
            store_fqdn: None,
            ..Default::default()
        })
        .unwrap();
        let loaded = get_cached_app_info(dir.path()).unwrap();
        assert_eq!(loaded.config_file.as_deref(), Some("shopify.app.prod.toml"));
        clear_current_config_file(dir.path()).unwrap();
        let cleared = get_cached_app_info(dir.path()).unwrap();
        assert!(cleared.config_file.is_none());
    }
}
