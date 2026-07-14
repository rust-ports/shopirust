#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageManager {
    Npm,
    Yarn,
    Pnpm,
}

impl PackageManager {
    pub fn name(&self) -> &'static str {
        match self {
            PackageManager::Npm => "npm",
            PackageManager::Yarn => "yarn",
            PackageManager::Pnpm => "pnpm",
        }
    }
}

pub fn detect_package_manager(path: &std::path::Path) -> PackageManager {
    let lockfile = |name: &str| path.join(name).exists();
    if lockfile("pnpm-lock.yaml") {
        PackageManager::Pnpm
    } else if lockfile("yarn.lock") {
        PackageManager::Yarn
    } else {
        PackageManager::Npm
    }
}

pub fn format_package_manager_command(pm: PackageManager, cmd: &str) -> String {
    match pm {
        PackageManager::Npm => format!("npm run {}", cmd),
        PackageManager::Yarn => format!("yarn {}", cmd),
        PackageManager::Pnpm => format!("pnpm run {}", cmd),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_detect_package_manager_defaults_to_npm() {
        let dir = TempDir::new().unwrap();
        assert_eq!(detect_package_manager(dir.path()), PackageManager::Npm);
    }

    #[test]
    fn test_detect_package_manager_pnpm() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("pnpm-lock.yaml"), "").unwrap();
        assert_eq!(detect_package_manager(dir.path()), PackageManager::Pnpm);
    }

    #[test]
    fn test_detect_package_manager_yarn() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("yarn.lock"), "").unwrap();
        assert_eq!(detect_package_manager(dir.path()), PackageManager::Yarn);
    }

    #[test]
    fn test_format_package_manager_command() {
        assert_eq!(
            format_package_manager_command(PackageManager::Npm, "dev"),
            "npm run dev"
        );
        assert_eq!(
            format_package_manager_command(PackageManager::Yarn, "dev"),
            "yarn dev"
        );
        assert_eq!(
            format_package_manager_command(PackageManager::Pnpm, "dev"),
            "pnpm run dev"
        );
    }

    #[test]
    fn test_pm_name() {
        assert_eq!(PackageManager::Npm.name(), "npm");
        assert_eq!(PackageManager::Yarn.name(), "yarn");
        assert_eq!(PackageManager::Pnpm.name(), "pnpm");
    }
}
