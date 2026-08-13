//! Config-file name / selection prompts (upstream `prompts/config.ts`).

use super::{PromptItem, Prompter};
use crate::error::AppError;
use crate::models::config_file_naming::{
    get_app_configuration_file_name, slugify, APP_CONFIG_MAX_SLUG_LEN,
};
use std::path::Path;

/// Max slug length so `shopify.app.{slug}.toml` stays under common FS limits (238 upstream).
pub fn validate_config_name(value: &str) -> Result<(), AppError> {
    let slug = slugify(value);
    if slug.len() > APP_CONFIG_MAX_SLUG_LEN {
        return Err(AppError::message("The file name is too long."));
    }
    Ok(())
}

/// Prompt for a configuration file name, offering to overwrite if it already exists.
pub fn select_config_name(
    prompter: &dyn Prompter,
    directory: &Path,
    default_name: &str,
) -> Result<String, AppError> {
    loop {
        let raw = prompter.text("Configuration file name:", Some(default_name))?;
        validate_config_name(&raw)?;
        let file_name = get_app_configuration_file_name(Some(&raw));
        if directory.join(&file_name).exists() {
            let ask_again = prompter.confirm(&format!(
                "Configuration file {file_name} already exists. Do you want to choose a different configuration name?"
            ))?;
            if ask_again {
                continue;
            }
        }
        return Ok(file_name);
    }
}

/// Prompt to pick among existing `shopify.app*.toml` files.
pub fn select_config_file(
    prompter: &dyn Prompter,
    files: &[String],
) -> Result<String, AppError> {
    if files.is_empty() {
        return Err(AppError::message(
            "Could not find any shopify.app.toml file in the directory.",
        ));
    }
    if files.len() == 1 {
        return Ok(files[0].clone());
    }
    let items: Vec<_> = files
        .iter()
        .map(|f| PromptItem::new(f.clone(), f.clone()))
        .collect();
    prompter.select("Configuration file", &items)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prompts::InjectedPrompter;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn validate_rejects_too_long() {
        let long = "a".repeat(300);
        assert!(validate_config_name(&long).is_err());
    }

    #[test]
    fn select_name_uses_default_when_file_missing() {
        let dir = tempdir().unwrap();
        let p = InjectedPrompter::new();
        p.push_text("staging");
        let name = select_config_name(&p, dir.path(), "").unwrap();
        assert_eq!(name, "shopify.app.staging.toml");
    }

    #[test]
    fn select_name_overwrite_existing() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("shopify.app.staging.toml"), "").unwrap();
        let p = InjectedPrompter::new();
        p.push_text("staging");
        p.push_confirm(false);
        let name = select_config_name(&p, dir.path(), "").unwrap();
        assert_eq!(name, "shopify.app.staging.toml");
    }

    #[test]
    fn select_file_single() {
        let p = InjectedPrompter::new();
        let chosen = select_config_file(&p, &["shopify.app.toml".into()]).unwrap();
        assert_eq!(chosen, "shopify.app.toml");
    }

    #[test]
    fn select_file_prompt() {
        let p = InjectedPrompter::new();
        p.push_select("shopify.app.prod.toml");
        let chosen = select_config_file(
            &p,
            &["shopify.app.toml".into(), "shopify.app.prod.toml".into()],
        )
        .unwrap();
        assert_eq!(chosen, "shopify.app.prod.toml");
    }
}
