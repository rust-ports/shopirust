use crate::error::AppError;
use crate::local_storage::{clear_current_config_file, set_cached_app_info, CachedAppInfo};
use crate::models::config_file_naming::get_app_configuration_file_name;
use crate::models::loader::{load_app, LoadAppOptions};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct UseConfigOptions {
    pub directory: PathBuf,
    pub config_name: Option<String>,
    pub reset: bool,
}

#[derive(Debug, Clone)]
pub struct UseConfigResult {
    pub config_file: Option<String>,
    pub message: String,
}

pub fn use_config(options: UseConfigOptions) -> Result<UseConfigResult, AppError> {
    let directory = options.directory;
    if options.reset {
        clear_current_config_file(&directory)?;
        return Ok(UseConfigResult {
            config_file: None,
            message: "Cleared current configuration.".into(),
        });
    }

    let config_file = resolve_config_file_name(&directory, options.config_name.as_deref())?;
    let loaded = load_app(LoadAppOptions {
        directory: directory.clone(),
        config_name: Some(config_file.clone()),
    })?;

    if !loaded.is_linked() {
        return Err(AppError::message(format!(
            "Configuration file {config_file} needs a client_id."
        )));
    }

    set_cached_app_info(&CachedAppInfo {
        directory: directory.display().to_string(),
        config_file: Some(config_file.clone()),
        app_id: loaded.configuration.client_id.clone(),
        title: loaded.configuration.name.clone(),
        org_id: None,
        store_fqdn: loaded.hidden_config.dev_store_url.clone(),
    })?;

    Ok(UseConfigResult {
        message: format!("Using configuration file {config_file}"),
        config_file: Some(config_file),
    })
}

fn resolve_config_file_name(directory: &Path, config_name: Option<&str>) -> Result<String, AppError> {
    if let Some(name) = config_name {
        let file = get_app_configuration_file_name(Some(name));
        if directory.join(&file).exists() {
            return Ok(file);
        }
        return Err(AppError::message(format!(
            "Could not find configuration file {file}"
        )));
    }

    // Prefer default shopify.app.toml, else first discovered config.
    let default = get_app_configuration_file_name(None);
    if directory.join(&default).exists() {
        return Ok(default);
    }
    let project = crate::models::project::Project::load(directory)?;
    project
        .config_files
        .first()
        .and_then(|p| p.file_name().map(|s| s.to_string_lossy().into_owned()))
        .ok_or_else(|| AppError::message("No app configuration files found"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn use_sets_cached_config() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("shopify.app.toml"),
            "client_id = \"abc\"\nname = \"Demo\"\n",
        )
        .unwrap();
        let result = use_config(UseConfigOptions {
            directory: dir.path().to_path_buf(),
            config_name: None,
            reset: false,
        })
        .unwrap();
        assert_eq!(result.config_file.as_deref(), Some("shopify.app.toml"));
    }

    #[test]
    fn use_reset_clears() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("shopify.app.toml"),
            "client_id = \"abc\"\n",
        )
        .unwrap();
        use_config(UseConfigOptions {
            directory: dir.path().to_path_buf(),
            config_name: None,
            reset: false,
        })
        .unwrap();
        let result = use_config(UseConfigOptions {
            directory: dir.path().to_path_buf(),
            config_name: None,
            reset: true,
        })
        .unwrap();
        assert!(result.config_file.is_none());
    }
}
