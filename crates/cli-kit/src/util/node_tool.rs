use cli_core::error::{CliError, CliErrorKind};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Output};

pub const THEME_TOOLS_DIR_ENV: &str = "SHOPIFY_CLI_THEME_TOOLS_DIR";
pub const NODE_EXECUTABLE_ENV: &str = "SHOPIFY_CLI_NODE";

#[derive(Debug, Clone)]
pub struct PackagedNodeTool {
    pub package: String,
    pub package_json: PathBuf,
    pub executable: PathBuf,
    pub node: String,
}

impl PackagedNodeTool {
    pub fn resolve(
        package: &str,
        adapter: &str,
        working_directory: &Path,
    ) -> Result<Self, CliError> {
        let package_json = package_json_candidates(package, working_directory)
            .into_iter()
            .find(|path| path.is_file())
            .ok_or_else(|| {
                CliError::abort(format!(
                    "Unable to find the bundled {package} runtime. Reinstall Shopify CLI and retry."
                ))
            })?;
        let raw = std::fs::read_to_string(&package_json).map_err(|error| {
            CliError::abort(format!(
                "Unable to read bundled package metadata {}: {error}",
                package_json.display()
            ))
        })?;
        let metadata: Value = serde_json::from_str(&raw).map_err(|error| {
            CliError::abort(format!(
                "Invalid bundled package metadata {}: {error}",
                package_json.display()
            ))
        })?;
        let _version = metadata
            .get("version")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                CliError::abort(format!("The bundled {package} package has no version."))
            })?;
        let tools_root = package_json
            .ancestors()
            .find(|path| path.file_name().is_some_and(|name| name == "node_modules"))
            .and_then(Path::parent)
            .expect("package is below a node_modules directory");
        let executable = tools_root.join("adapters").join(adapter);
        if !executable.is_file() {
            return Err(CliError::abort(format!(
                "The bundled {package} executable is missing at {}. Reinstall Shopify CLI and retry.",
                executable.display()
            )));
        }

        Ok(Self {
            package: package.to_string(),
            package_json,
            executable,
            node: bundled_node_executable().unwrap_or_else(|| "node".into()),
        })
    }

    pub fn command(&self, working_directory: &Path) -> Command {
        let mut command = Command::new(&self.node);
        command.arg(&self.executable).current_dir(working_directory);
        command
    }

    pub fn status(
        &self,
        working_directory: &Path,
        args: &[String],
    ) -> Result<ExitStatus, CliError> {
        self.command(working_directory)
            .args(args)
            .status()
            .map_err(|error| launch_error(self, error))
    }

    pub fn output(&self, working_directory: &Path, args: &[String]) -> Result<Output, CliError> {
        self.command(working_directory)
            .args(args)
            .output()
            .map_err(|error| launch_error(self, error))
    }

    pub fn version(&self) -> Option<String> {
        let raw = std::fs::read_to_string(&self.package_json).ok()?;
        let metadata: Value = serde_json::from_str(&raw).ok()?;
        metadata.get("version")?.as_str().map(str::to_string)
    }
}

pub fn child_exit_error(status: ExitStatus) -> CliError {
    CliError {
        kind: CliErrorKind::AbortSilent,
        message: String::new(),
        next_steps: None,
        exit_code: status.code().unwrap_or(1),
    }
}

fn launch_error(tool: &PackagedNodeTool, error: std::io::Error) -> CliError {
    CliError::abort(format!(
        "Unable to launch {} with `{}`: {error}",
        tool.package, tool.node
    ))
    .with_next_steps(format!(
        "Install Node.js 22.12.0 or newer, or set {NODE_EXECUTABLE_ENV} to a compatible Node.js executable."
    ))
}

fn package_json_candidates(package: &str, working_directory: &Path) -> Vec<PathBuf> {
    let package_path = PathBuf::from(package);
    let mut roots = Vec::new();
    if let Some(root) = std::env::var_os(THEME_TOOLS_DIR_ENV).filter(|value| !value.is_empty()) {
        roots.push(PathBuf::from(root));
    }
    if let Ok(executable) = std::env::current_exe() {
        if let Some(bin) = executable.parent() {
            roots.push(bin.join("bridge/theme-tools"));
            roots.push(bin.join("bridge/node-cli"));
            roots.push(bin.join("bridge/node-cli/packages/theme"));
        }
    }
    roots.push(crate::commands::compat::bridge_cache_dir().join("bridge/theme-tools"));
    roots.push(working_directory.to_path_buf());

    roots
        .into_iter()
        .map(|root| {
            root.join("node_modules")
                .join(&package_path)
                .join("package.json")
        })
        .collect()
}

fn bundled_node_executable() -> Option<String> {
    if let Some(node) = std::env::var(NODE_EXECUTABLE_ENV)
        .ok()
        .filter(|value| !value.trim().is_empty())
    {
        return Some(node);
    }
    let node_name = if cfg!(windows) {
        "node.exe"
    } else {
        "bin/node"
    };
    let candidates = [
        std::env::current_exe().ok().and_then(|executable| {
            executable
                .parent()
                .map(|bin| bin.join("bridge").join("node").join(node_name))
        }),
        Some(
            crate::commands::compat::bridge_cache_dir()
                .join("bridge")
                .join("node")
                .join(node_name),
        ),
    ];
    candidates
        .into_iter()
        .flatten()
        .find(|path| path.is_file())
        .map(|path| path.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use once_cell::sync::Lazy;
    use std::sync::Mutex;

    static ENV_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

    fn install_tool(root: &Path) -> PathBuf {
        let package = root.join("node_modules/@shopify/example");
        std::fs::create_dir_all(&package).unwrap();
        std::fs::create_dir_all(root.join("adapters")).unwrap();
        std::fs::write(root.join("adapters/example.cjs"), "process.exit(0)").unwrap();
        std::fs::write(
            package.join("package.json"),
            serde_json::json!({"name":"@shopify/example","version":"1.2.3"}).to_string(),
        )
        .unwrap();
        package
    }

    #[test]
    fn resolves_tool_from_explicit_packaged_directory() {
        let _guard = ENV_LOCK.lock().unwrap();
        let root = tempfile::tempdir().unwrap();
        install_tool(root.path());
        std::env::set_var(THEME_TOOLS_DIR_ENV, root.path());

        let tool =
            PackagedNodeTool::resolve("@shopify/example", "example.cjs", Path::new("/unused"))
                .unwrap();

        assert_eq!(tool.executable, root.path().join("adapters/example.cjs"));
        assert_eq!(tool.version().as_deref(), Some("1.2.3"));
        std::env::remove_var(THEME_TOOLS_DIR_ENV);
    }

    #[test]
    fn reports_missing_packaged_tool() {
        let _guard = ENV_LOCK.lock().unwrap();
        let root = tempfile::tempdir().unwrap();
        std::env::set_var(THEME_TOOLS_DIR_ENV, root.path());

        let error =
            PackagedNodeTool::resolve("@shopify/missing", "missing.cjs", root.path()).unwrap_err();

        assert!(error.message.contains("bundled @shopify/missing runtime"));
        std::env::remove_var(THEME_TOOLS_DIR_ENV);
    }
}
