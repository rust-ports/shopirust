//! Downloadable wasm toolchain binaries (function-runner, javy, trampoline, wasm-opt).

use crate::error::AppError;
use flate2::read::GzDecoder;
use std::fs::{self, File};
use std::io::{copy, Write};
use std::path::{Path, PathBuf};

pub const PREFERRED_FUNCTION_RUNNER_VERSION: &str = "9.1.2";
pub const PREFERRED_JAVY_VERSION: &str = "7.0.1";
pub const PREFERRED_JAVY_PLUGIN_VERSION: &str = "3";
const BINARYEN_VERSION: &str = "123.0.0";
pub const V1_TRAMPOLINE_VERSION: &str = "1.0.2";
pub const V2_TRAMPOLINE_VERSION: &str = "2.0.1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinaryDependencies {
    pub function_runner: String,
    pub javy: String,
    pub javy_plugin: String,
}

/// Derive binary versions from `@shopify/shopify_function` major version.
pub fn derive_javascript_binary_dependencies(version: &str) -> Option<BinaryDependencies> {
    match version {
        "0" | "1" => Some(BinaryDependencies {
            function_runner: "7.0.1".into(),
            javy: "4.0.0".into(),
            javy_plugin: "1".into(),
        }),
        "2" => Some(BinaryDependencies {
            function_runner: PREFERRED_FUNCTION_RUNNER_VERSION.into(),
            javy: PREFERRED_JAVY_VERSION.into(),
            javy_plugin: PREFERRED_JAVY_PLUGIN_VERSION.into(),
        }),
        _ => None,
    }
}

#[derive(Debug, Clone)]
pub struct DownloadableBinary {
    pub name: String,
    pub version: String,
    pub path: PathBuf,
    download_url: String,
    gzip: bool,
}

impl DownloadableBinary {
    pub fn download_url(&self) -> &str {
        &self.download_url
    }
}

fn bin_dir() -> PathBuf {
    let base = dirs::cache_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
    base.join("shopify-cli").join("bin")
}

fn platform_arch(process_platform: &str, process_arch: &str) -> Result<(String, String), AppError> {
    let platform = match process_platform.to_lowercase().as_str() {
        "darwin" | "macos" => "macos",
        "linux" => "linux",
        "win32" | "windows" => "windows",
        other => return Err(AppError::message(format!("Unsupported platform {other}"))),
    }
    .to_string();
    let arch = match process_arch.to_lowercase().as_str() {
        "arm" | "arm64" | "aarch64" => "arm",
        "ia32" | "x86" | "x64" | "x86_64" => "x86_64",
        other => {
            return Err(AppError::message(format!(
                "Unsupported architecture {other}"
            )))
        }
    }
    .to_string();
    Ok((platform, arch))
}

fn version_satisfies_ge(version: &str, major: u64, minor: u64, patch: u64) -> bool {
    let parts: Vec<u64> = version
        .split('.')
        .take(3)
        .filter_map(|p| p.parse().ok())
        .collect();
    let (v_maj, v_min, v_pat) = (
        parts.first().copied().unwrap_or(0),
        parts.get(1).copied().unwrap_or(0),
        parts.get(2).copied().unwrap_or(0),
    );
    (v_maj, v_min, v_pat) >= (major, minor, patch)
}

fn executable_download_url(
    name: &str,
    version: &str,
    git_hub_repo: &str,
    release: &str,
    process_platform: &str,
    process_arch: &str,
    supports_windows_on_arm: bool,
) -> Result<String, AppError> {
    let (platform, arch) = platform_arch(process_platform, process_arch)?;
    let arch_platform = format!("{arch}-{platform}");
    let mut supported = vec![
        "arm-linux",
        "arm-macos",
        "x86_64-macos",
        "x86_64-windows",
        "x86_64-linux",
    ];
    if supports_windows_on_arm {
        supported.push("arm-windows");
    }
    if !supported.contains(&arch_platform.as_str()) {
        return Err(AppError::message(format!(
            "Unsupported platform/architecture combination {process_platform}/{process_arch}"
        )));
    }
    Ok(format!(
        "https://github.com/{git_hub_repo}/releases/download/{release}/{name}-{arch_platform}-v{version}.gz"
    ))
}

fn executable_path(name: &str, version: &str) -> PathBuf {
    let filename = if cfg!(windows) {
        format!("{name}-{version}.exe")
    } else {
        format!("{name}-{version}")
    };
    bin_dir().join(filename)
}

pub fn javy_binary(version: &str) -> Result<DownloadableBinary, AppError> {
    let supports_woa = version_satisfies_ge(version, 7, 0, 0);
    let url = executable_download_url(
        "javy",
        version,
        "bytecodealliance/javy",
        &format!("v{version}"),
        std::env::consts::OS,
        std::env::consts::ARCH,
        supports_woa,
    )?;
    Ok(DownloadableBinary {
        name: "javy".into(),
        version: version.into(),
        path: executable_path("javy", version),
        download_url: url,
        gzip: true,
    })
}

pub fn javy_plugin_binary(version: &str) -> DownloadableBinary {
    let name = format!("shopify_functions_javy_v{version}");
    DownloadableBinary {
        name: name.clone(),
        version: version.into(),
        path: bin_dir().join(format!("{name}.wasm")),
        download_url: format!(
            "https://cdn.shopify.com/shopifycloud/shopify-functions-javy-plugin/{name}.wasm"
        ),
        gzip: false,
    }
}

pub fn function_runner_binary(version: &str) -> Result<DownloadableBinary, AppError> {
    let supports_woa = version_satisfies_ge(version, 9, 1, 1);
    let url = executable_download_url(
        "function-runner",
        version,
        "Shopify/function-runner",
        &format!("v{version}"),
        std::env::consts::OS,
        std::env::consts::ARCH,
        supports_woa,
    )?;
    Ok(DownloadableBinary {
        name: "function-runner".into(),
        version: version.into(),
        path: executable_path("function-runner", version),
        download_url: url,
        gzip: true,
    })
}

pub fn wasm_opt_binary() -> DownloadableBinary {
    DownloadableBinary {
        name: "wasm-opt.cjs".into(),
        version: BINARYEN_VERSION.into(),
        path: bin_dir().join("wasm-opt.cjs"),
        download_url: format!(
            "https://cdn.jsdelivr.net/npm/binaryen@{BINARYEN_VERSION}/bin/wasm-opt"
        ),
        gzip: false,
    }
}

pub fn trampoline_binary(version: &str) -> Result<DownloadableBinary, AppError> {
    let supports_woa = version_satisfies_ge(version, 2, 0, 1);
    let release = format!("shopify_function_trampoline/v{version}");
    let url = executable_download_url(
        "shopify-function-trampoline",
        version,
        "Shopify/shopify-function-wasm-api",
        &release,
        std::env::consts::OS,
        std::env::consts::ARCH,
        supports_woa,
    )?;
    Ok(DownloadableBinary {
        name: "shopify-function-trampoline".into(),
        version: version.into(),
        path: executable_path("shopify-function-trampoline", version),
        download_url: url,
        gzip: true,
    })
}

/// Download a binary if not already present. Writes via a temp file then renames.
pub async fn download_binary(bin: &DownloadableBinary) -> Result<(), AppError> {
    if bin.path.is_file() {
        return Ok(());
    }
    perform_download(bin).await
}

async fn perform_download(bin: &DownloadableBinary) -> Result<(), AppError> {
    if let Some(parent) = bin.path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut last_err = None;
    for _ in 0..3 {
        match download_once(bin).await {
            Ok(()) => return Ok(()),
            Err(e) => last_err = Some(e),
        }
    }
    Err(last_err.unwrap_or_else(|| AppError::message("download failed")))
}

async fn download_once(bin: &DownloadableBinary) -> Result<(), AppError> {
    let response = reqwest::get(&bin.download_url)
        .await
        .map_err(|e| AppError::message(format!("Downloading {} failed: {e}", bin.name)))?;
    if !response.status().is_success() {
        return Err(AppError::message(format!(
            "Downloading {} failed with status code of {}",
            bin.name,
            response.status().as_u16()
        )));
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|e| AppError::message(format!("Downloading {} failed: {e}", bin.name)))?;

    let tmp = tempfile::NamedTempFile::new_in(bin.path.parent().unwrap_or_else(|| Path::new(".")))
        .map_err(|e| AppError::message(e.to_string()))?;

    if bin.gzip {
        let mut decoder = GzDecoder::new(&bytes[..]);
        let mut out = File::create(tmp.path())?;
        copy(&mut decoder, &mut out)?;
    } else {
        let mut out = File::create(tmp.path())?;
        out.write_all(&bytes)?;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(tmp.path())?.permissions();
        perms.set_mode(0o775);
        fs::set_permissions(tmp.path(), perms)?;
    }

    fs::rename(tmp.path(), &bin.path).or_else(|_| {
        fs::copy(tmp.path(), &bin.path)?;
        fs::remove_file(tmp.path())?;
        Ok::<(), std::io::Error>(())
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn javy_url_linux_x64() {
        let url = executable_download_url(
            "javy",
            "7.0.1",
            "bytecodealliance/javy",
            "v7.0.1",
            "linux",
            "x64",
            true,
        )
        .unwrap();
        assert!(url.contains("javy-x86_64-linux-v7.0.1.gz"));
        assert!(url.contains("bytecodealliance/javy"));
    }

    #[test]
    fn javy_url_darwin_arm() {
        let url = executable_download_url(
            "javy",
            "7.0.1",
            "bytecodealliance/javy",
            "v7.0.1",
            "darwin",
            "arm64",
            true,
        )
        .unwrap();
        assert!(url.contains("javy-arm-macos-v7.0.1.gz"));
    }

    #[test]
    fn javy_url_windows_x64() {
        let url = executable_download_url(
            "javy",
            "7.0.1",
            "bytecodealliance/javy",
            "v7.0.1",
            "win32",
            "x64",
            true,
        )
        .unwrap();
        assert!(url.contains("javy-x86_64-windows-v7.0.1.gz"));
    }

    #[test]
    fn unsupported_platform_errors() {
        let err = executable_download_url(
            "javy",
            "7.0.1",
            "bytecodealliance/javy",
            "v7.0.1",
            "aix",
            "x64",
            true,
        )
        .unwrap_err();
        assert!(err.to_string().contains("Unsupported platform"));
    }

    #[test]
    fn unsupported_arch_errors() {
        let err = executable_download_url(
            "javy",
            "7.0.1",
            "bytecodealliance/javy",
            "v7.0.1",
            "darwin",
            "ppc",
            true,
        )
        .unwrap_err();
        assert!(err.to_string().contains("Unsupported architecture"));
    }

    #[test]
    fn old_javy_rejects_windows_arm() {
        let err = executable_download_url(
            "javy",
            "6.0.0",
            "bytecodealliance/javy",
            "v6.0.0",
            "win32",
            "arm",
            false,
        )
        .unwrap_err();
        assert!(err
            .to_string()
            .contains("Unsupported platform/architecture"));
    }

    #[test]
    fn preferred_javy_allows_windows_arm() {
        let url = executable_download_url(
            "javy",
            "7.0.1",
            "bytecodealliance/javy",
            "v7.0.1",
            "win32",
            "arm",
            true,
        )
        .unwrap();
        assert!(url.contains("javy-arm-windows-v7.0.1.gz"));
    }

    #[test]
    fn derive_deps_for_v2() {
        let deps = derive_javascript_binary_dependencies("2").unwrap();
        assert_eq!(deps.function_runner, PREFERRED_FUNCTION_RUNNER_VERSION);
        assert_eq!(deps.javy, PREFERRED_JAVY_VERSION);
        assert_eq!(deps.javy_plugin, PREFERRED_JAVY_PLUGIN_VERSION);
    }

    #[test]
    fn derive_deps_unknown_is_none() {
        assert!(derive_javascript_binary_dependencies("99").is_none());
    }

    #[test]
    fn javy_plugin_url() {
        let bin = javy_plugin_binary("3");
        assert!(bin
            .download_url()
            .contains("shopify_functions_javy_v3.wasm"));
        assert!(!bin.gzip);
    }

    #[test]
    fn function_runner_properties() {
        let bin = function_runner_binary(PREFERRED_FUNCTION_RUNNER_VERSION).unwrap();
        assert_eq!(bin.name, "function-runner");
        assert!(bin.path.to_string_lossy().contains("function-runner-"));
    }
}
