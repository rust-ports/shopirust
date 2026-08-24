//! Package-manager helpers used by `app init`.

use crate::error::AppError;
use std::path::Path;
use std::process::Command;

/// Detect npm/yarn/pnpm/bun from the environment or lockfiles.
pub fn detect_package_manager(directory: &Path, preferred: Option<&str>) -> String {
    if let Some(pref) = preferred {
        if !pref.is_empty() {
            return pref.to_string();
        }
    }
    if directory.join("pnpm-lock.yaml").exists() {
        return "pnpm".into();
    }
    if directory.join("yarn.lock").exists() {
        return "yarn".into();
    }
    if directory.join("bun.lockb").exists() || directory.join("bun.lock").exists() {
        return "bun".into();
    }
    "npm".into()
}

/// Run `install` for the given package manager.
pub fn install_dependencies(directory: &Path, package_manager: &str) -> Result<(), AppError> {
    let status = Command::new(package_manager)
        .arg("install")
        .current_dir(directory)
        .status()
        .map_err(|e| AppError::message(format!("Failed to run {package_manager} install: {e}")))?;
    if !status.success() {
        return Err(AppError::message(format!(
            "{package_manager} install failed"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn prefers_explicit() {
        let dir = tempdir().unwrap();
        assert_eq!(detect_package_manager(dir.path(), Some("yarn")), "yarn");
    }

    #[test]
    fn detects_lockfiles() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("pnpm-lock.yaml"), "").unwrap();
        assert_eq!(detect_package_manager(dir.path(), None), "pnpm");
    }

    #[test]
    fn defaults_to_npm() {
        let dir = tempdir().unwrap();
        assert_eq!(detect_package_manager(dir.path(), None), "npm");
    }
}
