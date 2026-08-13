//! Install npm/yarn/pnpm dependencies for an app project (upstream `dependencies.ts`).

use crate::error::AppError;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageManager {
    Npm,
    Yarn,
    Pnpm,
}

impl PackageManager {
    pub fn name(self) -> &'static str {
        match self {
            Self::Npm => "npm",
            Self::Yarn => "yarn",
            Self::Pnpm => "pnpm",
        }
    }

    pub fn install_args(self) -> &'static [&'static str] {
        match self {
            Self::Npm => &["install"],
            Self::Yarn => &["install"],
            Self::Pnpm => &["install"],
        }
    }
}

pub fn detect_package_manager(directory: &Path) -> PackageManager {
    if directory.join("pnpm-lock.yaml").exists() {
        PackageManager::Pnpm
    } else if directory.join("yarn.lock").exists() {
        PackageManager::Yarn
    } else {
        PackageManager::Npm
    }
}

/// Directories (depth ≤ `deep`) that contain a `package.json`.
pub fn package_json_directories(root: &Path, deep: usize) -> Vec<PathBuf> {
    let mut out = Vec::new();
    collect(root, 0, deep, &mut out);
    out.sort();
    out
}

fn collect(dir: &Path, depth: usize, max: usize, out: &mut Vec<PathBuf>) {
    if depth > max {
        return;
    }
    if dir.join("package.json").is_file() {
        out.push(dir.to_path_buf());
    }
    if depth == max || !dir.is_dir() {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name == "node_modules" || name.starts_with('.') {
            continue;
        }
        collect(&path, depth + 1, max, out);
    }
}

pub type InstallRunner = fn(&Path, PackageManager) -> Result<(), AppError>;

fn default_install(dir: &Path, pm: PackageManager) -> Result<(), AppError> {
    let status = Command::new(pm.name())
        .args(pm.install_args())
        .current_dir(dir)
        .status()
        .map_err(|e| AppError::message(format!("Failed to run {}: {e}", pm.name())))?;
    if !status.success() {
        return Err(AppError::message(format!(
            "{} install failed in {}",
            pm.name(),
            dir.display()
        )));
    }
    Ok(())
}

/// Walk `package.json` trees (depth 3) and install, unless `skip`.
pub fn install_app_dependencies(
    directory: &Path,
    skip: bool,
    runner: Option<InstallRunner>,
) -> Result<Vec<PathBuf>, AppError> {
    if skip {
        return Ok(vec![]);
    }
    let pm = detect_package_manager(directory);
    let dirs = package_json_directories(directory, 3);
    let run = runner.unwrap_or(default_install);
    for dir in &dirs {
        run(dir, pm)?;
    }
    Ok(dirs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn skip_does_not_walk() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("package.json"), "{}").unwrap();
        let installed = install_app_dependencies(dir.path(), true, None).unwrap();
        assert!(installed.is_empty());
    }

    #[test]
    fn finds_nested_package_json() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("package.json"), "{}").unwrap();
        let web = dir.path().join("web");
        fs::create_dir_all(&web).unwrap();
        fs::write(web.join("package.json"), "{}").unwrap();
        fs::create_dir_all(dir.path().join("node_modules/pkg")).unwrap();
        fs::write(dir.path().join("node_modules/pkg/package.json"), "{}").unwrap();
        let found = package_json_directories(dir.path(), 3);
        assert_eq!(found.len(), 2);
    }

    #[test]
    fn runner_is_invoked() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static CALLS: AtomicUsize = AtomicUsize::new(0);
        fn counting(_: &Path, _: PackageManager) -> Result<(), AppError> {
            CALLS.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("package.json"), "{}").unwrap();
        CALLS.store(0, Ordering::SeqCst);
        let installed = install_app_dependencies(dir.path(), false, Some(counting)).unwrap();
        assert_eq!(installed.len(), 1);
        assert_eq!(CALLS.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn detects_pnpm() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("pnpm-lock.yaml"), "").unwrap();
        assert_eq!(detect_package_manager(dir.path()), PackageManager::Pnpm);
    }
}
