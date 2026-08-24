use clap::Args;
use cli_core::error::{CliError, CliErrorKind};
use flate2::read::GzDecoder;
use sha2::{Digest, Sha256};
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::process::Command;
use tar::Archive;

pub const BRIDGE_RUNNER_ENV: &str = "SHOPIFY_CLI_BRIDGE_RUNNER";
pub const BRIDGE_URL_ENV: &str = "SHOPIFY_CLI_BRIDGE_URL";
const BUNDLED_BRIDGE_DIR: &str = "bridge";
const BUNDLED_BRIDGE_RUNNER: &str = "bridge-runner";
const BUNDLED_BRIDGE_NODE_CLI: &str = "node-cli";

pub fn bridge_platform() -> String {
    crate::util::system::host_npm_platform_arch()
}

pub fn bridge_cache_dir() -> PathBuf {
    PathBuf::from(crate::constants::cache_path())
        .join("bridge")
        .join(format!("v{}", env!("CARGO_PKG_VERSION")))
        .join(bridge_platform())
}

pub fn bridge_archive_url() -> String {
    std::env::var(BRIDGE_URL_ENV).unwrap_or_else(|_| {
        format!(
            "https://github.com/rust-ports/shopirust/releases/download/v{}/shopify-rust-bridge-{}.tar.gz",
            env!("CARGO_PKG_VERSION"),
            bridge_platform()
        )
    })
}

#[derive(Debug, Clone, Default, Args)]
pub struct BridgeArgs {
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct BridgeCommand {
    command_id: &'static str,
    args: Vec<String>,
}

impl BridgeCommand {
    pub fn new(command_id: &'static str, args: Vec<String>) -> Self {
        Self { command_id, args }
    }

    pub async fn run(self) -> Result<(), CliError> {
        run_bridge(self.command_id, &self.args).await
    }
}

pub async fn run_bridge(command_id: &str, args: &[String]) -> Result<(), CliError> {
    let runner = resolve_bridge_runner().map_err(|_| {
        CliError::abort(format!(
            "`shopify {}` needs the optional Node compatibility bridge, but it is not installed.",
            command_id.replace(':', " ")
        ))
        .with_next_steps(
            "Run `shopify bridge install` to download the verified bridge, install a packaged release, or set SHOPIFY_CLI_BRIDGE_RUNNER for development.",
        )
    })?;

    let mut command = Command::new(&runner);
    command.arg(command_id).args(args);
    if let Some(global) = cli_core::runner::current_global_flags() {
        if let Some(path) = global.path {
            command.env("SHOPIFY_FLAG_PATH", path);
        }
        if global.verbose {
            command.env("SHOPIFY_FLAG_VERBOSE", "true");
        }
        if global.no_color {
            command.env("SHOPIFY_FLAG_NO_COLOR", "true");
        }
    }
    let status = command.status().map_err(|error| {
        CliError::abort(format!("Failed to run bridge runner `{runner}`: {error}"))
    })?;

    if status.success() {
        return Ok(());
    }

    Err(CliError {
        kind: CliErrorKind::AbortSilent,
        message: String::new(),
        next_steps: None,
        exit_code: status.code().unwrap_or(1),
    })
}

pub fn bridge_version() -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    bridge_version_for_exe(&exe)
}

fn resolve_bridge_runner() -> Result<String, ()> {
    if let Ok(runner) = std::env::var(BRIDGE_RUNNER_ENV) {
        if !runner.trim().is_empty() {
            return Ok(runner);
        }
    }

    let exe = std::env::current_exe().map_err(|_| ())?;
    bundled_bridge_runner_for_exe(&exe)
        .filter(|path| path.is_file())
        .or_else(cached_bridge_runner)
        .map(|path| path.to_string_lossy().into_owned())
        .ok_or(())
}

pub fn cached_bridge_runner() -> Option<PathBuf> {
    let runner_name = if cfg!(windows) {
        format!("{BUNDLED_BRIDGE_RUNNER}.cmd")
    } else {
        BUNDLED_BRIDGE_RUNNER.to_string()
    };
    let path = bridge_cache_dir()
        .join(BUNDLED_BRIDGE_DIR)
        .join(runner_name);
    path.is_file().then_some(path)
}

pub async fn install_bridge(url: Option<&str>) -> Result<PathBuf, CliError> {
    let url = url.map(str::to_owned).unwrap_or_else(bridge_archive_url);
    let checksum_url = format!("{url}.sha256");
    let archive = reqwest::get(&url)
        .await
        .map_err(|error| CliError::abort(format!("Unable to download bridge archive: {error}")))?
        .error_for_status()
        .map_err(|error| CliError::abort(format!("Unable to download bridge archive: {error}")))?
        .bytes()
        .await
        .map_err(|error| CliError::abort(format!("Unable to read bridge archive: {error}")))?;
    let checksum = reqwest::get(&checksum_url)
        .await
        .map_err(|error| CliError::abort(format!("Unable to download bridge checksum: {error}")))?
        .error_for_status()
        .map_err(|error| CliError::abort(format!("Unable to download bridge checksum: {error}")))?
        .text()
        .await
        .map_err(|error| CliError::abort(format!("Unable to read bridge checksum: {error}")))?;
    let expected = checksum
        .split_whitespace()
        .next()
        .filter(|value| value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .ok_or_else(|| {
            CliError::abort("Bridge checksum file does not contain a SHA-256 digest.")
        })?;
    let actual = hex::encode(Sha256::digest(&archive));
    if !actual.eq_ignore_ascii_case(expected) {
        return Err(CliError::abort(
            "Bridge checksum verification failed; the archive was not installed.",
        ));
    }

    let destination = bridge_cache_dir();
    let parent = destination
        .parent()
        .ok_or_else(|| CliError::abort("Invalid bridge cache path."))?;
    std::fs::create_dir_all(parent)
        .map_err(|error| CliError::abort(format!("Unable to create bridge cache: {error}")))?;
    let temporary = parent.join(format!(
        ".{}-{}.partial",
        bridge_platform(),
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&temporary)
        .map_err(|error| CliError::abort(format!("Unable to prepare bridge install: {error}")))?;
    let extract = (|| -> Result<(), CliError> {
        let decoder = GzDecoder::new(Cursor::new(archive));
        let mut tar = Archive::new(decoder);
        for entry in tar
            .entries()
            .map_err(|error| CliError::abort(format!("Invalid bridge archive: {error}")))?
        {
            let mut entry = entry.map_err(|error| {
                CliError::abort(format!("Invalid bridge archive entry: {error}"))
            })?;
            if !entry.unpack_in(&temporary).map_err(|error| {
                CliError::abort(format!("Unable to extract bridge archive: {error}"))
            })? {
                return Err(CliError::abort("Bridge archive contains an unsafe path."));
            }
        }
        if !temporary.join(BUNDLED_BRIDGE_DIR).is_dir() {
            return Err(CliError::abort(
                "Bridge archive has an invalid layout (missing bridge directory).",
            ));
        }
        Ok(())
    })();
    if let Err(error) = extract {
        let _ = std::fs::remove_dir_all(&temporary);
        return Err(error);
    }
    if destination.exists() {
        std::fs::remove_dir_all(&destination)
            .map_err(|error| CliError::abort(format!("Unable to replace bridge: {error}")))?;
    }
    std::fs::rename(&temporary, &destination)
        .map_err(|error| CliError::abort(format!("Unable to install bridge: {error}")))?;
    cached_bridge_runner().ok_or_else(|| {
        CliError::abort("Bridge archive installed but does not contain a runnable bridge runner.")
    })
}

pub fn uninstall_cached_bridge() -> Result<bool, CliError> {
    let destination = bridge_cache_dir();
    if !destination.exists() {
        return Ok(false);
    }
    std::fs::remove_dir_all(&destination)
        .map_err(|error| CliError::abort(format!("Unable to remove bridge: {error}")))?;
    Ok(true)
}

fn bundled_bridge_runner_for_exe(exe: &Path) -> Option<PathBuf> {
    let dir = exe.parent()?;
    let runner_name = if cfg!(windows) {
        format!("{BUNDLED_BRIDGE_RUNNER}.cmd")
    } else {
        BUNDLED_BRIDGE_RUNNER.to_string()
    };
    Some(dir.join(BUNDLED_BRIDGE_DIR).join(runner_name))
}

fn bridge_version_for_exe(exe: &Path) -> Option<String> {
    let dir = exe.parent()?;
    let bridge_dir = dir.join(BUNDLED_BRIDGE_DIR).join(BUNDLED_BRIDGE_NODE_CLI);
    let package_json = [
        bridge_dir.join("package.json"),
        bridge_dir.join("packages").join("cli").join("package.json"),
    ]
    .into_iter()
    .find(|path| path.is_file())?;
    let raw = std::fs::read_to_string(package_json).ok()?;
    let value: serde_json::Value = serde_json::from_str(&raw).ok()?;
    value
        .get("version")
        .and_then(|version| version.as_str())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use once_cell::sync::Lazy;
    use tokio::sync::Mutex;

    static ENV_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

    #[tokio::test]
    async fn missing_runner_returns_actionable_error() {
        let _guard = ENV_LOCK.lock().await;
        std::env::remove_var(BRIDGE_RUNNER_ENV);
        let err = run_bridge("hydrogen:dev", &[]).await.unwrap_err();
        assert!(err.message.contains("bridge, but it is not installed"));
        let next_steps = err.next_steps.unwrap();
        assert!(next_steps.contains("shopify bridge install"));
        assert!(next_steps.contains(BRIDGE_RUNNER_ENV));
    }

    #[test]
    fn bundled_runner_path_is_beside_executable() {
        let exe = PathBuf::from("/tmp/shopify-release/bin/shopify");
        let runner = bundled_bridge_runner_for_exe(&exe).unwrap();
        let suffix = if cfg!(windows) {
            PathBuf::from("bridge").join("bridge-runner.cmd")
        } else {
            PathBuf::from("bridge").join("bridge-runner")
        };
        assert!(runner.ends_with(suffix));
    }

    #[tokio::test]
    async fn cached_runner_is_discovered() {
        let _guard = ENV_LOCK.lock().await;
        let cache = tempfile::tempdir().unwrap();
        let previous = std::env::var_os("XDG_CACHE_HOME");
        std::env::set_var("XDG_CACHE_HOME", cache.path());

        let runner = cached_bridge_runner().unwrap_or_else(|| {
            let name = if cfg!(windows) {
                "bridge-runner.cmd"
            } else {
                "bridge-runner"
            };
            bridge_cache_dir().join(BUNDLED_BRIDGE_DIR).join(name)
        });
        std::fs::create_dir_all(runner.parent().unwrap()).unwrap();
        std::fs::write(&runner, "bridge").unwrap();
        assert_eq!(cached_bridge_runner().as_deref(), Some(runner.as_path()));

        if let Some(previous) = previous {
            std::env::set_var("XDG_CACHE_HOME", previous);
        } else {
            std::env::remove_var("XDG_CACHE_HOME");
        }
    }

    #[test]
    fn reads_bundled_bridge_version_from_package_json() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("shopify");
        let package_dir = dir.path().join("bridge").join("node-cli");
        std::fs::create_dir_all(&package_dir).unwrap();
        std::fs::write(
            package_dir.join("package.json"),
            r#"{"name":"@shopify/cli","version":"3.99.1"}"#,
        )
        .unwrap();

        assert_eq!(bridge_version_for_exe(&exe).as_deref(), Some("3.99.1"));
    }

    #[test]
    fn reads_full_copy_bridge_version_from_nested_package_json() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("shopify");
        let package_dir = dir
            .path()
            .join("bridge")
            .join("node-cli")
            .join("packages")
            .join("cli");
        std::fs::create_dir_all(&package_dir).unwrap();
        std::fs::write(
            package_dir.join("package.json"),
            r#"{"name":"@shopify/cli","version":"4.2.0"}"#,
        )
        .unwrap();

        assert_eq!(bridge_version_for_exe(&exe).as_deref(), Some("4.2.0"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn env_runner_receives_command_id_args_env_and_cwd() {
        use std::os::unix::fs::PermissionsExt;

        let _guard = ENV_LOCK.lock().await;
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("bridge.log");
        let cwd = dir.path().join("cwd");
        std::fs::create_dir(&cwd).unwrap();
        let runner = dir.path().join("bridge.sh");
        std::fs::write(
            &runner,
            format!(
                "#!/bin/sh\npwd > '{}'\nprintf '%s\\n' \"$SHOPIFY_TEST_BRIDGE_ENV\" \"$@\" >> '{}'\nexit 0\n",
                log.display(),
                log.display()
            ),
        )
        .unwrap();
        let mut perms = std::fs::metadata(&runner).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&runner, perms).unwrap();

        std::env::set_var(BRIDGE_RUNNER_ENV, &runner);
        std::env::set_var("SHOPIFY_TEST_BRIDGE_ENV", "preserved");
        let previous_cwd = std::env::current_dir().unwrap();
        std::env::set_current_dir(&cwd).unwrap();
        run_bridge("hydrogen:build", &["--path".into(), "web".into()])
            .await
            .unwrap();
        std::env::set_current_dir(previous_cwd).unwrap();

        let raw = std::fs::read_to_string(log).unwrap();
        assert_eq!(
            raw,
            format!(
                "{}\npreserved\nhydrogen:build\n--path\nweb\n",
                cwd.display()
            )
        );
        std::env::remove_var(BRIDGE_RUNNER_ENV);
        std::env::remove_var("SHOPIFY_TEST_BRIDGE_ENV");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn non_zero_runner_status_preserves_exit_code() {
        use std::os::unix::fs::PermissionsExt;

        let _guard = ENV_LOCK.lock().await;
        let dir = tempfile::tempdir().unwrap();
        let runner = dir.path().join("bridge.sh");
        std::fs::write(&runner, "#!/bin/sh\nexit 42\n").unwrap();
        let mut perms = std::fs::metadata(&runner).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&runner, perms).unwrap();

        std::env::set_var(BRIDGE_RUNNER_ENV, &runner);
        let err = run_bridge("plugins:install", &[]).await.unwrap_err();

        assert_eq!(err.kind, CliErrorKind::AbortSilent);
        assert_eq!(err.exit_code, 42);
        std::env::remove_var(BRIDGE_RUNNER_ENV);
    }
}
