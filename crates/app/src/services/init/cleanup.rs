//! Post-scaffold cleanup of template leftovers.

use crate::error::AppError;
use std::fs;
use std::path::Path;

const JUNK: &[&str] = &[
    ".git",
    ".github",
    ".gitmodules",
    "LICENSE.md",
    "LICENSE",
    "CHANGELOG.md",
];

/// Remove Git metadata and license files copied from the upstream template.
pub fn cleanup_template_files(directory: &Path) -> Result<(), AppError> {
    for name in JUNK {
        let path = directory.join(name);
        if path.is_dir() {
            let _ = fs::remove_dir_all(&path);
        } else if path.is_file() {
            let _ = fs::remove_file(&path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn removes_git_and_license() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".git")).unwrap();
        fs::write(dir.path().join("LICENSE.md"), "MIT").unwrap();
        fs::write(dir.path().join("keep.txt"), "ok").unwrap();
        cleanup_template_files(dir.path()).unwrap();
        assert!(!dir.path().join(".git").exists());
        assert!(!dir.path().join("LICENSE.md").exists());
        assert!(dir.path().join("keep.txt").exists());
    }
}
