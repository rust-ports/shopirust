//! Locate or download the `mkcert` binary (same pattern as cloudflared).

use cli_core::error::CliError;
use std::fs;
use std::path::{Path, PathBuf};

pub const MKCERT_VERSION: &str = "v1.4.4";
pub const MKCERT_REPO: &str = "FiloSottile/mkcert";
const LICENSE_URL: &str =
    "https://raw.githubusercontent.com/FiloSottile/mkcert/refs/tags/v1.4.4/LICENSE";

/// Environment variable that hard-codes the mkcert binary path.
pub const MKCERT_BINARY_ENV: &str = "SHOPIFY_CLI_MKCERT_BINARY";

#[derive(Debug, Clone, Copy)]
pub enum MkcertPlatform {
    DarwinAmd64,
    DarwinArm64,
    LinuxAmd64,
    LinuxArm64,
    WindowsAmd64,
}

impl MkcertPlatform {
    pub fn current() -> Result<Self, CliError> {
        from_os_arch(std::env::consts::OS, std::env::consts::ARCH)
    }

    pub fn asset_name(self) -> &'static str {
        match self {
            Self::DarwinAmd64 => "mkcert-v1.4.4-darwin-amd64",
            Self::DarwinArm64 => "mkcert-v1.4.4-darwin-arm64",
            Self::LinuxAmd64 => "mkcert-v1.4.4-linux-amd64",
            Self::LinuxArm64 => "mkcert-v1.4.4-linux-arm64",
            Self::WindowsAmd64 => "mkcert-v1.4.4-windows-amd64.exe",
        }
    }

    pub fn binary_name(self) -> &'static str {
        match self {
            Self::WindowsAmd64 => "mkcert.exe",
            _ => "mkcert",
        }
    }
}

pub fn from_os_arch(os: &str, arch: &str) -> Result<MkcertPlatform, CliError> {
    match (os, arch) {
        ("macos" | "darwin", "aarch64") => Ok(MkcertPlatform::DarwinArm64),
        ("macos" | "darwin", _) => Ok(MkcertPlatform::DarwinAmd64),
        ("linux", "aarch64") => Ok(MkcertPlatform::LinuxArm64),
        ("linux", _) => Ok(MkcertPlatform::LinuxAmd64),
        ("windows", _) => Ok(MkcertPlatform::WindowsAmd64),
        (os, arch) => Err(CliError::abort(format!(
            "Unsupported platform for mkcert: {os}/{arch}"
        ))),
    }
}

pub fn github_release_url(platform: MkcertPlatform) -> String {
    format!(
        "https://github.com/{MKCERT_REPO}/releases/download/{MKCERT_VERSION}/{}",
        platform.asset_name()
    )
}

/// Resolve mkcert: env → `.shopify/mkcert` → PATH → download into `.shopify`.
pub async fn get_mkcert_path(
    dot_shopify: &Path,
    env_override: Option<&str>,
    platform: MkcertPlatform,
) -> Result<PathBuf, CliError> {
    if let Some(path) = env_override.filter(|s| !s.is_empty()) {
        return Ok(PathBuf::from(path));
    }
    if let Ok(path) = std::env::var(MKCERT_BINARY_ENV) {
        if !path.is_empty() {
            return Ok(PathBuf::from(path));
        }
    }

    let default_path = dot_shopify.join(platform.binary_name());
    if default_path.is_file() {
        return Ok(default_path);
    }

    if let Some(on_path) = which_mkcert() {
        return Ok(PathBuf::from(on_path));
    }

    fs::create_dir_all(dot_shopify).map_err(|e| CliError::abort(e.to_string()))?;
    download_mkcert(&default_path, platform).await?;
    Ok(default_path)
}

pub async fn download_mkcert(target: &Path, platform: MkcertPlatform) -> Result<(), CliError> {
    if target.is_file() {
        return Ok(());
    }
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|e| CliError::abort(e.to_string()))?;
    }
    let url = github_release_url(platform);
    let bytes = reqwest::get(&url)
        .await
        .map_err(|e| CliError::abort(format!("Failed to download mkcert: {e}")))?
        .bytes()
        .await
        .map_err(|e| CliError::abort(format!("Failed to read mkcert download: {e}")))?;
    fs::write(target, bytes).map_err(|e| CliError::abort(e.to_string()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(target)
            .map_err(|e| CliError::abort(e.to_string()))?
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(target, perms).map_err(|e| CliError::abort(e.to_string()))?;
    }
    Ok(())
}

pub async fn download_mkcert_license(dot_shopify: &Path) -> Result<bool, CliError> {
    let license_path = dot_shopify.join("mkcert-LICENSE");
    if license_path.is_file() {
        return Ok(true);
    }
    fs::create_dir_all(dot_shopify).map_err(|e| CliError::abort(e.to_string()))?;
    let response = match reqwest::get(LICENSE_URL).await {
        Ok(r) => r,
        Err(_) => return Ok(false),
    };
    if !response.status().is_success() {
        return Ok(false);
    }
    let text = match response.text().await {
        Ok(t) => t,
        Err(_) => return Ok(false),
    };
    fs::write(license_path, text).map_err(|e| CliError::abort(e.to_string()))?;
    Ok(true)
}

fn which_mkcert() -> Option<String> {
    for name in ["mkcert", "mkcert.exe"] {
        if let Ok(output) = std::process::Command::new("which").arg(name).output() {
            if output.status.success() {
                let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !path.is_empty() {
                    return Some(path);
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_names() {
        assert_eq!(
            MkcertPlatform::LinuxAmd64.asset_name(),
            "mkcert-v1.4.4-linux-amd64"
        );
        assert_eq!(MkcertPlatform::WindowsAmd64.binary_name(), "mkcert.exe");
        assert!(github_release_url(MkcertPlatform::DarwinArm64).contains("darwin-arm64"));
    }

    #[test]
    fn from_os_arch_linux() {
        assert!(matches!(
            from_os_arch("linux", "x86_64").unwrap(),
            MkcertPlatform::LinuxAmd64
        ));
        assert!(matches!(
            from_os_arch("linux", "aarch64").unwrap(),
            MkcertPlatform::LinuxArm64
        ));
    }

    #[tokio::test]
    async fn env_override_wins() {
        let dir = tempfile::tempdir().unwrap();
        let path = get_mkcert_path(dir.path(), Some("/custom/mkcert"), MkcertPlatform::LinuxAmd64)
            .await
            .unwrap();
        assert_eq!(path, PathBuf::from("/custom/mkcert"));
    }

    #[tokio::test]
    async fn default_path_if_present() {
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join("mkcert");
        fs::write(&bin, b"fake").unwrap();
        let path = get_mkcert_path(dir.path(), None, MkcertPlatform::LinuxAmd64)
            .await
            .unwrap();
        assert_eq!(path, bin);
    }
}
