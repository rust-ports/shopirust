use crate::error::AppError;
use crate::models::config_file_naming::get_app_configuration_shorthand;
use crate::models::loader::LoadedApp;
use cli_api::OrganizationApp;
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct EnvValues {
    pub shopify_api_key: String,
    pub shopify_api_secret: Option<String>,
    pub scopes: String,
}

impl EnvValues {
    pub fn from_apps(app: &LoadedApp, remote_app: &OrganizationApp) -> Self {
        Self {
            shopify_api_key: remote_app.api_key.clone(),
            shopify_api_secret: remote_app
                .api_secret_keys
                .first()
                .map(|k| k.secret.clone()),
            scopes: app_scopes_string(app),
        }
    }

    pub fn as_map(&self) -> HashMap<String, Option<String>> {
        let mut map = HashMap::new();
        map.insert(
            "SHOPIFY_API_KEY".into(),
            Some(self.shopify_api_key.clone()),
        );
        map.insert("SHOPIFY_API_SECRET".into(), self.shopify_api_secret.clone());
        map.insert("SCOPES".into(), Some(self.scopes.clone()));
        map
    }
}

#[derive(Debug, Clone)]
pub struct PullEnvOptions {
    pub env_file: PathBuf,
    pub values: EnvValues,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullEnvResult {
    pub message: String,
    pub content: String,
    pub created: bool,
    pub changed: bool,
}

pub fn get_dot_env_file_name(configuration_path: &str) -> String {
    match get_app_configuration_shorthand(configuration_path) {
        Some(shorthand) => format!(".env.{shorthand}"),
        None => ".env".into(),
    }
}

pub fn app_scopes_string(app: &LoadedApp) -> String {
    app.configuration
        .extra
        .get("access_scopes")
        .and_then(|v| v.get("scopes"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

pub fn format_env_file_content(values: &EnvValues) -> String {
    patch_env_file(None, &values.as_map())
}

pub fn pull_env(options: PullEnvOptions) -> Result<PullEnvResult, AppError> {
    let updates = options.values.as_map();
    let env_file = &options.env_file;

    if env_file.exists() {
        let existing = fs::read_to_string(env_file)?;
        let updated = patch_env_file(Some(&existing), &updates);
        if updated == existing {
            return Ok(PullEnvResult {
                message: format!("No changes to {}", env_file.display()),
                content: existing,
                created: false,
                changed: false,
            });
        }
        write_env_file(env_file, &updated)?;
        let diff = simple_lines_diff(&existing, &updated);
        Ok(PullEnvResult {
            message: format!(
                "Updated {} to be:\n\n{}\n\nHere's what changed:\n\n{}",
                env_file.display(),
                updated,
                diff
            ),
            content: updated,
            created: false,
            changed: true,
        })
    } else {
        let content = patch_env_file(None, &updates);
        write_env_file(env_file, &content)?;
        Ok(PullEnvResult {
            message: format!("Created {}:\n\n{}\n", env_file.display(), content),
            content,
            created: true,
            changed: true,
        })
    }
}

fn write_env_file(path: &Path, content: &str) -> Result<(), AppError> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    fs::write(path, content)?;
    Ok(())
}

/// Patch `.env` content, preserving unrelated keys and comments (mirrors cli-kit util).
pub fn patch_env_file(
    env_file_content: Option<&str>,
    updated_values: &HashMap<String, Option<String>>,
) -> String {
    let mut output_lines: Vec<String> = Vec::new();
    let env_file_lines: Vec<&str> = match env_file_content {
        Some(c) => c.split('\n').collect(),
        None => Vec::new(),
    };

    let mut already_present_keys: Vec<String> = Vec::new();
    let re = regex_lite::Regex::new(r"^([^=:#]+?)[=:](.*)").expect("env line regex");

    for line in &env_file_lines {
        let mut line_to_write = line.to_string();
        if let Some(caps) = re.captures(line) {
            let key = caps.get(1).unwrap().as_str().trim().to_string();
            if let Some(Some(nv)) = updated_values.get(&key) {
                already_present_keys.push(key.clone());
                line_to_write = format!("{key}={nv}");
            }
        }
        output_lines.push(line_to_write);
    }

    // Preserve insertion order of known Shopify keys, then remaining patches.
    for key in ["SHOPIFY_API_KEY", "SHOPIFY_API_SECRET", "SCOPES"] {
        if already_present_keys.iter().any(|k| k == key) {
            continue;
        }
        if let Some(updated_value) = updated_values.get(key) {
            let value = updated_value.as_deref().unwrap_or("");
            output_lines.push(format!("{key}={value}"));
            already_present_keys.push(key.to_string());
        }
    }
    for (patch_key, updated_value) in updated_values {
        if already_present_keys.contains(patch_key) {
            continue;
        }
        let value = updated_value.as_deref().unwrap_or("");
        output_lines.push(format!("{patch_key}={value}"));
    }

    // Avoid trailing empty line duplication when source ended without newline.
    while output_lines.last().is_some_and(|l| l.is_empty()) && output_lines.len() > 1 {
        // keep a single trailing empty only if original had content ending in newline
        break;
    }
    output_lines.join("\n")
}

fn simple_lines_diff(before: &str, after: &str) -> String {
    let before_lines: Vec<&str> = before.lines().collect();
    let after_lines: Vec<&str> = after.lines().collect();
    let mut out = Vec::new();
    let max = before_lines.len().max(after_lines.len());
    for i in 0..max {
        let b = before_lines.get(i).copied();
        let a = after_lines.get(i).copied();
        match (b, a) {
            (Some(b), Some(a)) if b == a => out.push(a.to_string()),
            (Some(b), Some(a)) => {
                out.push(format!("- {b}"));
                out.push(format!("+ {a}"));
            }
            (Some(b), None) => out.push(format!("- {b}")),
            (None, Some(a)) => out.push(format!("+ {a}")),
            (None, None) => {}
        }
    }
    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn sample_values() -> EnvValues {
        EnvValues {
            shopify_api_key: "api-key".into(),
            shopify_api_secret: Some("api-secret".into()),
            scopes: "my-scope".into(),
        }
    }

    #[test]
    fn formats_env_content() {
        let content = format_env_file_content(&sample_values());
        assert_eq!(
            content,
            "SHOPIFY_API_KEY=api-key\nSHOPIFY_API_SECRET=api-secret\nSCOPES=my-scope"
        );
    }

    #[test]
    fn patch_preserves_unrelated_keys() {
        let existing = "FOO=bar\nSHOPIFY_API_KEY=old\n";
        let updated = patch_env_file(Some(existing), &sample_values().as_map());
        assert!(updated.contains("FOO=bar"));
        assert!(updated.contains("SHOPIFY_API_KEY=api-key"));
        assert!(updated.contains("SHOPIFY_API_SECRET=api-secret"));
        assert!(updated.contains("SCOPES=my-scope"));
    }

    #[test]
    fn pull_creates_and_skips_unchanged() {
        let dir = tempdir().unwrap();
        let path = dir.path().join(".env");
        let created = pull_env(PullEnvOptions {
            env_file: path.clone(),
            values: sample_values(),
        })
        .unwrap();
        assert!(created.created);
        assert!(created.changed);
        assert!(path.exists());

        let again = pull_env(PullEnvOptions {
            env_file: path,
            values: sample_values(),
        })
        .unwrap();
        assert!(!again.changed);
        assert!(again.message.starts_with("No changes"));
    }

    #[test]
    fn dot_env_file_name_uses_shorthand() {
        assert_eq!(get_dot_env_file_name("shopify.app.toml"), ".env");
        assert_eq!(
            get_dot_env_file_name("/tmp/shopify.app.staging.toml"),
            ".env.staging"
        );
    }
}
