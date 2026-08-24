use clap::Args;
use cli_core::error::{CliError, CliErrorKind};
use std::path::{Path, PathBuf};
use std::process::Command;

pub const BRIDGE_RUNNER_ENV: &str = "SHOPIFY_CLI_BRIDGE_RUNNER";
const BUNDLED_BRIDGE_DIR: &str = "bridge";
const BUNDLED_BRIDGE_RUNNER: &str = "bridge-runner";
const BUNDLED_BRIDGE_NODE_CLI: &str = "node-cli";

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
            "`shopify {}` is registered through the Node compatibility bridge, but no bridge runner was found.",
            command_id.replace(':', " ")
        ))
        .with_next_steps(
            "Install a release artifact that includes `bridge/bridge-runner`, or set SHOPIFY_CLI_BRIDGE_RUNNER to an executable that accepts `<command-id> [...args]`.",
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
        .map(|path| path.to_string_lossy().into_owned())
        .ok_or(())
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
        assert!(err.message.contains("no bridge runner was found"));
        let next_steps = err.next_steps.unwrap();
        assert!(next_steps.contains("bridge/bridge-runner"));
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
