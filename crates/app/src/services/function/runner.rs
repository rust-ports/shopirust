//! Invoke `function-runner` against a local wasm artifact.

use crate::error::AppError;
use crate::models::extensions::extension_instance::ExtensionInstance;
use crate::services::function::binaries::{
    download_binary, function_runner_binary, PREFERRED_FUNCTION_RUNNER_VERSION,
};
use crate::services::function::build::validate_shopify_function_package_version;
use std::path::PathBuf;
use std::process::{Command, Stdio};

#[derive(Debug, Clone, Default)]
pub struct RunFunctionOptions {
    pub input: Option<String>,
    pub input_path: Option<PathBuf>,
    pub export: Option<String>,
    pub json: bool,
    pub schema_path: Option<PathBuf>,
    pub query_path: Option<PathBuf>,
}

async fn resolve_runner_binary(
    ext: &ExtensionInstance,
) -> Result<crate::services::function::binaries::DownloadableBinary, AppError> {
    if ext.is_function_extension() && ext.is_javascript() {
        if let Ok(deps) = validate_shopify_function_package_version(ext) {
            return function_runner_binary(&deps.function_runner);
        }
    }
    function_runner_binary(PREFERRED_FUNCTION_RUNNER_VERSION)
}

/// Run the function locally via the downloaded `function-runner` binary.
pub async fn run_function(
    ext: &ExtensionInstance,
    options: RunFunctionOptions,
) -> Result<(), AppError> {
    let runner = resolve_runner_binary(ext).await?;
    download_binary(&runner).await?;

    let mut args: Vec<String> = vec![
        "-f".into(),
        ext.function_output_path().display().to_string(),
    ];
    if let Some(ref input_path) = options.input_path {
        args.push("--input".into());
        args.push(input_path.display().to_string());
    }
    if let Some(ref export) = options.export {
        args.push("--export".into());
        args.push(export.clone());
    }
    if options.json {
        args.push("--json".into());
    }
    if let (Some(schema), Some(query)) = (&options.schema_path, &options.query_path) {
        args.push("--schema-path".into());
        args.push(schema.display().to_string());
        args.push("--query-path".into());
        args.push(query.display().to_string());
    }

    let mut cmd = Command::new(&runner.path);
    cmd.args(&args).current_dir(&ext.directory);
    if options.input_path.is_none() {
        cmd.stdin(Stdio::inherit());
    }
    cmd.stdout(Stdio::inherit()).stderr(Stdio::inherit());

    if let Some(ref input) = options.input {
        use std::io::Write;
        cmd.stdin(Stdio::piped());
        let mut child = cmd
            .spawn()
            .map_err(|e| AppError::message(format!("Failed to start function-runner: {e}")))?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(input.as_bytes())?;
        }
        let status = child
            .wait()
            .map_err(|e| AppError::message(format!("function-runner failed: {e}")))?;
        if !status.success() {
            return Err(AppError::message(format!(
                "function-runner exited with {:?}",
                status.code()
            )));
        }
        return Ok(());
    }

    let status = cmd
        .status()
        .map_err(|e| AppError::message(format!("Failed to start function-runner: {e}")))?;
    if !status.success() {
        return Err(AppError::message(format!(
            "function-runner exited with {:?}",
            status.code()
        )));
    }
    Ok(())
}

/// Build CLI args for `function-runner` (unit-tested independently of spawning).
pub fn runner_args(ext: &ExtensionInstance, options: &RunFunctionOptions) -> Vec<String> {
    let mut args: Vec<String> = vec![
        "-f".into(),
        ext.function_output_path().display().to_string(),
    ];
    if let Some(ref input_path) = options.input_path {
        args.push("--input".into());
        args.push(input_path.display().to_string());
    }
    if let Some(ref export) = options.export {
        args.push("--export".into());
        args.push(export.clone());
    }
    if options.json {
        args.push("--json".into());
    }
    if let (Some(schema), Some(query)) = (&options.schema_path, &options.query_path) {
        args.push("--schema-path".into());
        args.push(schema.display().to_string());
        args.push("--query-path".into());
        args.push(query.display().to_string());
    }
    args
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::extensions::create_extension_specification;
    use std::collections::HashMap;

    fn fn_ext() -> ExtensionInstance {
        let spec = create_extension_specification("function").unwrap();
        ExtensionInstance::new(
            "discount",
            PathBuf::from("/app/extensions/discount"),
            PathBuf::from("/app/extensions/discount/shopify.extension.toml"),
            HashMap::new(),
            spec,
        )
    }

    #[test]
    fn args_include_wasm_path() {
        let args = runner_args(&fn_ext(), &RunFunctionOptions::default());
        assert_eq!(args[0], "-f");
        assert!(args[1].ends_with("dist/index.wasm"));
    }

    #[test]
    fn args_include_export_json_and_schema() {
        let opts = RunFunctionOptions {
            export: Some("run".into()),
            json: true,
            schema_path: Some(PathBuf::from("schema.graphql")),
            query_path: Some(PathBuf::from("query.graphql")),
            ..Default::default()
        };
        let args = runner_args(&fn_ext(), &opts);
        assert!(args.contains(&"--export".into()));
        assert!(args.contains(&"run".into()));
        assert!(args.contains(&"--json".into()));
        assert!(args.contains(&"--schema-path".into()));
        assert!(args.contains(&"--query-path".into()));
    }

    #[test]
    fn input_path_adds_flag() {
        let opts = RunFunctionOptions {
            input_path: Some(PathBuf::from("input.json")),
            ..Default::default()
        };
        let args = runner_args(&fn_ext(), &opts);
        assert!(args.contains(&"--input".into()));
        assert!(args.iter().any(|a| a.ends_with("input.json")));
    }
}
