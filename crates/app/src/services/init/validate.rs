//! Template directory validation before scaffold.

use crate::error::AppError;
use std::path::Path;

const REQUIRED_FILES: &[&str] = &["package.json"];

/// Ensure a local template looks like a Shopify app template.
pub fn validate_template_directory(path: &Path) -> Result<(), AppError> {
    if !path.is_dir() {
        return Err(AppError::message(format!(
            "Template path does not exist: {}",
            path.display()
        )));
    }
    for file in REQUIRED_FILES {
        if !path.join(file).exists() {
            return Err(AppError::message(format!(
                "The template is missing required file {file}"
            )));
        }
    }
    Ok(())
}

/// Reject reserved / invalid hyphenated directory names.
pub fn validate_app_name(hyphenated: &str) -> Result<(), AppError> {
    if hyphenated.is_empty() {
        return Err(AppError::message("App name cannot be empty"));
    }
    if hyphenated == "." || hyphenated == ".." {
        return Err(AppError::message("App name cannot be '.' or '..'"));
    }
    if hyphenated.starts_with('-') || hyphenated.ends_with('-') {
        return Err(AppError::message(
            "App name cannot start or end with a hyphen",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn requires_package_json() {
        let dir = tempdir().unwrap();
        assert!(validate_template_directory(dir.path()).is_err());
        fs::write(dir.path().join("package.json"), "{}").unwrap();
        validate_template_directory(dir.path()).unwrap();
    }

    #[test]
    fn rejects_bad_names() {
        assert!(validate_app_name("").is_err());
        assert!(validate_app_name(".").is_err());
        assert!(validate_app_name("-oops").is_err());
        validate_app_name("my-app").unwrap();
    }
}
