use crate::error::AppError;
use crate::models::loader::{load_app, LoadAppOptions};
use crate::models::project::Project;
use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct ValidateConfigOptions {
    pub directory: PathBuf,
    pub config_name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ValidationIssue {
    pub file: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ValidateConfigResult {
    pub valid: bool,
    pub issues: Vec<ValidationIssue>,
}

pub fn validate_config(options: ValidateConfigOptions) -> Result<ValidateConfigResult, AppError> {
    let project = Project::load(&options.directory)?;
    let mut issues = Vec::new();

    if project.config_files.is_empty() {
        issues.push(ValidationIssue {
            file: options.directory.display().to_string(),
            message: "No shopify.app.toml configuration file found".into(),
        });
        return Ok(ValidateConfigResult {
            valid: false,
            issues,
        });
    }

    match load_app(LoadAppOptions {
        directory: options.directory,
        config_name: options.config_name,
    }) {
        Ok(app) => {
            if !app.is_linked() {
                issues.push(ValidationIssue {
                    file: app.configuration_path.display().to_string(),
                    message: "client_id is required".into(),
                });
            }
            if app
                .configuration
                .application_url
                .as_ref()
                .map(|u| u.is_empty())
                .unwrap_or(true)
            {
                issues.push(ValidationIssue {
                    file: app.configuration_path.display().to_string(),
                    message: "application_url is required".into(),
                });
            }
            for err in app.errors {
                issues.push(ValidationIssue {
                    file: "extensions".into(),
                    message: err,
                });
            }
            for ext in &app.extensions {
                if ext.handle.is_empty() {
                    issues.push(ValidationIssue {
                        file: ext.configuration_path.display().to_string(),
                        message: "extension handle is required".into(),
                    });
                }
            }
        }
        Err(e) => {
            issues.push(ValidationIssue {
                file: "shopify.app.toml".into(),
                message: e.to_string(),
            });
        }
    }

    Ok(ValidateConfigResult {
        valid: issues.is_empty(),
        issues,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn validate_reports_missing_client_id() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("shopify.app.toml"),
            "name = \"x\"\napplication_url = \"https://example.com\"\n",
        )
        .unwrap();
        let result = validate_config(ValidateConfigOptions {
            directory: dir.path().to_path_buf(),
            config_name: None,
        })
        .unwrap();
        assert!(!result.valid);
        assert!(result
            .issues
            .iter()
            .any(|i| i.message.contains("client_id")));
    }

    #[test]
    fn validate_accepts_linked_app() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("shopify.app.toml"),
            "client_id = \"abc\"\nname = \"x\"\napplication_url = \"https://example.com\"\nembedded = true\n",
        )
        .unwrap();
        let result = validate_config(ValidateConfigOptions {
            directory: dir.path().to_path_buf(),
            config_name: None,
        })
        .unwrap();
        assert!(result.valid, "{:?}", result.issues);
    }
}
