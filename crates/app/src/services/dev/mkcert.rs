//! Generate a localhost TLS certificate via `mkcert` (writes `.shopify/localhost.pem`).

use crate::error::AppError;
use crate::prompts::dev::prompt_generate_certificate;
use crate::prompts::Prompter;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const MKCERT_VERSION: &str = "v1.4.4";
pub const MKCERT_REPO: &str = "FiloSottile/mkcert";
pub const MKCERT_BINARY_ENV: &str = "SHOPIFY_CLI_MKCERT_BINARY";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalhostCert {
    pub key: String,
    pub cert: String,
    pub cert_path: String,
}

#[derive(Debug, Clone, Copy)]
pub enum MkcertPlatform {
    DarwinAmd64,
    DarwinArm64,
    LinuxAmd64,
    LinuxArm64,
    WindowsAmd64,
}

impl MkcertPlatform {
    pub fn detect(os: &str, arch: &str) -> Result<Self, AppError> {
        match (os, arch) {
            ("macos" | "darwin", "aarch64") => Ok(Self::DarwinArm64),
            ("macos" | "darwin", _) => Ok(Self::DarwinAmd64),
            ("linux", "aarch64") => Ok(Self::LinuxArm64),
            ("linux", _) => Ok(Self::LinuxAmd64),
            ("windows", _) => Ok(Self::WindowsAmd64),
            (os, arch) => Err(AppError::message(format!(
                "Unsupported platform for mkcert: {os}/{arch}"
            ))),
        }
    }

    pub fn current() -> Result<Self, AppError> {
        Self::detect(std::env::consts::OS, std::env::consts::ARCH)
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

pub type MkcertRunner = fn(&Path, &Path, &Path) -> Result<(), AppError>;

fn system_mkcert_runner(binary: &Path, key_path: &Path, cert_path: &Path) -> Result<(), AppError> {
    if let Some(parent) = key_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let status = Command::new(binary)
        .args([
            "-install",
            "-key-file",
            &key_path.display().to_string(),
            "-cert-file",
            &cert_path.display().to_string(),
            "localhost",
        ])
        .status()
        .map_err(|e| AppError::message(format!("Failed to run mkcert: {e}")))?;
    if !status.success() {
        return Err(AppError::message("mkcert failed to generate a certificate"));
    }
    Ok(())
}

pub fn relative_key_path() -> PathBuf {
    PathBuf::from(".shopify").join("localhost-key.pem")
}

pub fn relative_cert_path() -> PathBuf {
    PathBuf::from(".shopify").join("localhost.pem")
}

/// Resolve the mkcert binary: env → `.shopify/mkcert` → PATH.
pub fn resolve_mkcert_path(
    app_directory: &Path,
    env: &[(String, String)],
    platform: MkcertPlatform,
) -> Option<PathBuf> {
    for (k, v) in env {
        if k == MKCERT_BINARY_ENV && !v.is_empty() {
            return Some(PathBuf::from(v));
        }
    }
    if let Ok(v) = std::env::var(MKCERT_BINARY_ENV) {
        if !v.is_empty() {
            return Some(PathBuf::from(v));
        }
    }
    let default = app_directory
        .join(".shopify")
        .join(platform.binary_name());
    if default.is_file() {
        return Some(default);
    }
    which_mkcert()
}

fn which_mkcert() -> Option<PathBuf> {
    for name in ["mkcert", "mkcert.exe"] {
        if let Ok(output) = Command::new("which").arg(name).output() {
            if output.status.success() {
                let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !path.is_empty() {
                    return Some(PathBuf::from(path));
                }
            }
        }
    }
    None
}

pub fn github_release_url(platform: MkcertPlatform) -> String {
    format!(
        "https://github.com/{MKCERT_REPO}/releases/download/{MKCERT_VERSION}/{}",
        platform.asset_name()
    )
}

/// Generate (or reuse) `.shopify/localhost.pem` + `localhost-key.pem`.
pub async fn generate_certificate(
    app_directory: &Path,
    prompter: Option<&dyn Prompter>,
    env: &[(String, String)],
    platform: MkcertPlatform,
    runner: Option<MkcertRunner>,
) -> Result<LocalhostCert, AppError> {
    let relative_key = relative_key_path();
    let relative_cert = relative_cert_path();
    let key_path = app_directory.join(&relative_key);
    let cert_path = app_directory.join(&relative_cert);

    if key_path.is_file() && cert_path.is_file() {
        return Ok(LocalhostCert {
            key: fs::read_to_string(&key_path)?,
            cert: fs::read_to_string(&cert_path)?,
            cert_path: relative_cert.to_string_lossy().to_string(),
        });
    }

    if let Some(prompter) = prompter {
        if !prompt_generate_certificate(prompter)? {
            return Err(AppError::message(format!(
                "Localhost certificate and key are required at {} and {}",
                relative_cert.display(),
                relative_key.display()
            )));
        }
    }

    let binary = if let Some(path) = resolve_mkcert_path(app_directory, env, platform) {
        path
    } else {
        let dest = app_directory
            .join(".shopify")
            .join(platform.binary_name());
        download_mkcert_binary(&dest, platform).await?;
        dest
    };

    fs::create_dir_all(app_directory.join(".shopify"))?;
    let run = runner.unwrap_or(system_mkcert_runner);
    run(&binary, &key_path, &cert_path)?;

    if !key_path.is_file() || !cert_path.is_file() {
        return Err(AppError::message(
            "mkcert did not write localhost.pem / localhost-key.pem",
        ));
    }

    Ok(LocalhostCert {
        key: fs::read_to_string(&key_path)?,
        cert: fs::read_to_string(&cert_path)?,
        cert_path: relative_cert.to_string_lossy().to_string(),
    })
}

async fn download_mkcert_binary(target: &Path, platform: MkcertPlatform) -> Result<(), AppError> {
    if target.is_file() {
        return Ok(());
    }
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    let url = github_release_url(platform);
    let bytes = reqwest::get(&url)
        .await
        .map_err(|e| AppError::message(format!("Failed to download mkcert: {e}")))?
        .bytes()
        .await
        .map_err(|e| AppError::message(format!("Failed to read mkcert download: {e}")))?;
    fs::write(target, bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(target)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(target, perms)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prompts::InjectedPrompter;
    use tempfile::tempdir;

    fn write_certs(app: &Path) {
        fs::create_dir_all(app.join(".shopify")).unwrap();
        fs::write(app.join(".shopify/localhost-key.pem"), "key").unwrap();
        fs::write(app.join(".shopify/localhost.pem"), "cert").unwrap();
    }

    fn fake_runner(_bin: &Path, key: &Path, cert: &Path) -> Result<(), AppError> {
        if let Some(parent) = key.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(key, "key")?;
        fs::write(cert, "cert")?;
        Ok(())
    }

    #[tokio::test]
    async fn reuses_existing_certificate() {
        let dir = tempdir().unwrap();
        write_certs(dir.path());
        let p = InjectedPrompter::new();
        let cert = generate_certificate(
            dir.path(),
            Some(&p),
            &[],
            MkcertPlatform::LinuxAmd64,
            Some(fake_runner),
        )
        .await
        .unwrap();
        assert_eq!(cert.key, "key");
        assert_eq!(cert.cert, "cert");
        assert_eq!(cert.cert_path, ".shopify/localhost.pem");
    }

    #[tokio::test]
    async fn declines_prompt() {
        let dir = tempdir().unwrap();
        let p = InjectedPrompter::new();
        p.push_confirm(false);
        let err = generate_certificate(
            dir.path(),
            Some(&p),
            &[],
            MkcertPlatform::LinuxAmd64,
            Some(fake_runner),
        )
        .await
        .unwrap_err();
        assert!(err.to_string().contains("required"));
    }

    #[tokio::test]
    async fn env_binary_and_runner() {
        let dir = tempdir().unwrap();
        let p = InjectedPrompter::new();
        p.push_confirm(true);
        let env = vec![(MKCERT_BINARY_ENV.into(), "/path/to/mkcert".into())];
        let cert = generate_certificate(
            dir.path(),
            Some(&p),
            &env,
            MkcertPlatform::LinuxAmd64,
            Some(fake_runner),
        )
        .await
        .unwrap();
        assert_eq!(cert.cert, "cert");
        assert!(dir.path().join(".shopify/localhost.pem").is_file());
    }

    #[tokio::test]
    async fn default_shopify_binary() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".shopify")).unwrap();
        fs::write(dir.path().join(".shopify/mkcert"), "echo").unwrap();
        let p = InjectedPrompter::new();
        p.push_confirm(true);
        let cert = generate_certificate(
            dir.path(),
            Some(&p),
            &[],
            MkcertPlatform::LinuxAmd64,
            Some(fake_runner),
        )
        .await
        .unwrap();
        assert_eq!(cert.key, "key");
    }

    #[test]
    fn platform_asset_names() {
        assert_eq!(
            MkcertPlatform::LinuxAmd64.asset_name(),
            "mkcert-v1.4.4-linux-amd64"
        );
        assert!(github_release_url(MkcertPlatform::DarwinArm64).contains("darwin-arm64"));
        assert_eq!(MkcertPlatform::WindowsAmd64.binary_name(), "mkcert.exe");
    }
}
