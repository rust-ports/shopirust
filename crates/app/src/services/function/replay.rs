//! Replay a prior function run from `.shopify/logs`.

use crate::error::AppError;
use crate::models::extensions::extension_instance::ExtensionInstance;
use crate::services::function::common::function_logs_dir;
use crate::services::function::runner::{run_function, RunFunctionOptions};
use serde::Deserialize;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

const LOG_SELECTOR_LIMIT: usize = 100;

#[derive(Debug, Clone)]
pub struct ReplayOptions {
    pub app_directory: PathBuf,
    pub json: bool,
    /// When true, re-run whenever `dist/index.wasm` mtime changes.
    pub watch: bool,
    pub log: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FunctionRunPayload {
    pub input: Option<Value>,
    pub export: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FunctionRunData {
    pub payload: FunctionRunPayload,
    #[serde(default)]
    pub identifier: String,
}

#[derive(Debug, Clone)]
struct LogFileMetadata {
    namespace: String,
    function_handle: String,
    identifier: String,
}

/// Replay a stored function run against the current wasm.
pub async fn replay(ext: &ExtensionInstance, options: ReplayOptions) -> Result<(), AppError> {
    let function_runs_dir = function_logs_dir(&options.app_directory);

    let selected = if let Some(ref log_id) = options.log {
        get_run_from_identifier(&function_runs_dir, &ext.handle, log_id)?
    } else {
        get_run_from_selector(&function_runs_dir, &ext.handle)?
    };

    let input = selected
        .payload
        .input
        .clone()
        .ok_or_else(|| AppError::message("Selected log has no input payload to replay."))?;
    let run_export = selected.payload.export.clone();
    let input_json = serde_json::to_string(&input)?;

    run_function(
        ext,
        RunFunctionOptions {
            input: Some(input_json.clone()),
            export: run_export.clone(),
            json: options.json,
            ..Default::default()
        },
    )
    .await?;

    if !options.watch {
        return Ok(());
    }

    let wasm = ext.function_output_path();
    let mut last_mtime = wasm_mtime(&wasm);
    eprintln!("Watching {} for changes (Ctrl+C to stop).", wasm.display());
    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => break,
            _ = tokio::time::sleep(Duration::from_millis(500)) => {
                let current = wasm_mtime(&wasm);
                if wasm_mtime_changed(last_mtime, current) {
                    last_mtime = current;
                    eprintln!("Detected wasm change, replaying…");
                    run_function(
                        ext,
                        RunFunctionOptions {
                            input: Some(input_json.clone()),
                            export: run_export.clone(),
                            json: options.json,
                            ..Default::default()
                        },
                    )
                    .await?;
                }
            }
        }
    }
    Ok(())
}

pub fn wasm_mtime(path: &Path) -> Option<SystemTime> {
    fs::metadata(path).and_then(|m| m.modified()).ok()
}

pub fn wasm_mtime_changed(previous: Option<SystemTime>, current: Option<SystemTime>) -> bool {
    match (previous, current) {
        (Some(prev), Some(now)) => now > prev,
        (None, Some(_)) => true,
        _ => false,
    }
}

fn parse_log_filename(filename: &str) -> Option<LogFileMetadata> {
    // Expected: 20240522_150641_827Z_extensions_my-function_abcdef.json
    let stem = filename.strip_suffix(".json").unwrap_or(filename);
    let parts: Vec<&str> = stem.split('_').collect();
    if parts.len() < 6 {
        return None;
    }
    Some(LogFileMetadata {
        namespace: parts[3].to_string(),
        function_handle: parts[4].to_string(),
        identifier: parts[5].to_string(),
    })
}

fn get_all_function_run_file_names(function_runs_dir: &Path) -> Vec<String> {
    if !function_runs_dir.is_dir() {
        return vec![];
    }
    let mut names: Vec<String> = fs::read_dir(function_runs_dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            if name.ends_with(".json") {
                Some(name)
            } else {
                None
            }
        })
        .collect();
    names.sort();
    names
}

fn get_run_from_identifier(
    function_runs_dir: &Path,
    function_handle: &str,
    identifier: &str,
) -> Result<FunctionRunData, AppError> {
    let file_name = get_all_function_run_file_names(function_runs_dir)
        .into_iter()
        .find(|filename| {
            parse_log_filename(filename).is_some_and(|meta| {
                meta.namespace == "extensions"
                    && meta.function_handle == function_handle
                    && meta.identifier == identifier
            })
        });

    let Some(file_name) = file_name else {
        return Err(AppError::message(format!(
            "No log found for '{identifier}'.\nSearched {} for function {function_handle}.",
            function_runs_dir.display()
        )));
    };

    let path = function_runs_dir.join(&file_name);
    let data = fs::read_to_string(&path)?;
    let mut parsed: FunctionRunData = serde_json::from_str(&data)?;
    parsed.identifier = identifier.to_string();
    Ok(parsed)
}

fn get_identifier_from_filename(file_name: &str) -> String {
    let stem = Path::new(file_name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(file_name);
    stem.split('_')
        .next_back()
        .unwrap_or("")
        .chars()
        .take(6)
        .collect()
}

fn get_run_from_selector(
    function_runs_dir: &Path,
    function_handle: &str,
) -> Result<FunctionRunData, AppError> {
    let runs = get_function_run_data(function_runs_dir, function_handle)?;
    runs.into_iter().next().ok_or_else(|| {
        AppError::message(format!("No logs found in {}", function_runs_dir.display()))
    })
}

fn get_function_run_data(
    function_runs_dir: &Path,
    function_handle: &str,
) -> Result<Vec<FunctionRunData>, AppError> {
    let mut file_names: Vec<String> = get_all_function_run_file_names(function_runs_dir)
        .into_iter()
        .filter(|filename| {
            parse_log_filename(filename).is_some_and(|meta| {
                meta.namespace == "extensions" && meta.function_handle == function_handle
            })
        })
        .collect();
    file_names.reverse();

    let mut function_run_data = Vec::new();
    for chunk in file_names.chunks(LOG_SELECTOR_LIMIT) {
        if function_run_data.len() >= LOG_SELECTOR_LIMIT {
            break;
        }
        for function_run_file in chunk {
            if function_run_data.len() >= LOG_SELECTOR_LIMIT {
                break;
            }
            let path = function_runs_dir.join(function_run_file);
            let file_data = fs::read_to_string(&path)?;
            let mut parsed: FunctionRunData = serde_json::from_str(&file_data)?;
            if parsed.payload.input.is_none() {
                continue;
            }
            parsed.identifier = get_identifier_from_filename(function_run_file);
            function_run_data.push(parsed);
        }
    }
    Ok(function_run_data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn parse_log_filename_ok() {
        let meta =
            parse_log_filename("20240522_150641_827Z_extensions_my-function_abcdef.json").unwrap();
        assert_eq!(meta.namespace, "extensions");
        assert_eq!(meta.function_handle, "my-function");
        assert_eq!(meta.identifier, "abcdef");
    }

    #[test]
    fn parse_log_filename_short() {
        assert!(parse_log_filename("too_short.json").is_none());
    }

    #[test]
    fn get_run_from_identifier_reads_file() {
        let dir = tempdir().unwrap();
        let name = "20240522_150641_827Z_extensions_my-fn_abc123.json";
        fs::write(
            dir.path().join(name),
            r#"{"payload":{"input":{"a":1},"export":"run"},"identifier":"abc123"}"#,
        )
        .unwrap();
        let run = get_run_from_identifier(dir.path(), "my-fn", "abc123").unwrap();
        assert_eq!(run.payload.export.as_deref(), Some("run"));
        assert_eq!(run.payload.input.unwrap()["a"], 1);
    }

    #[test]
    fn get_run_from_identifier_missing() {
        let dir = tempdir().unwrap();
        let err = get_run_from_identifier(dir.path(), "my-fn", "nope").unwrap_err();
        assert!(err.to_string().contains("No log found"));
    }

    #[test]
    fn selector_picks_newest_with_input() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path()
                .join("20240522_150641_827Z_extensions_fn_old001.json"),
            r#"{"payload":{"input":{"v":1}}}"#,
        )
        .unwrap();
        fs::write(
            dir.path()
                .join("20240523_150641_827Z_extensions_fn_new002.json"),
            r#"{"payload":{"input":{"v":2}}}"#,
        )
        .unwrap();
        let run = get_run_from_selector(dir.path(), "fn").unwrap();
        assert_eq!(run.payload.input.unwrap()["v"], 2);
    }

    #[test]
    fn selector_skips_runs_without_input() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path()
                .join("20240522_150641_827Z_extensions_fn_noinp1.json"),
            r#"{"payload":{}}"#,
        )
        .unwrap();
        fs::write(
            dir.path()
                .join("20240523_150641_827Z_extensions_fn_has002.json"),
            r#"{"payload":{"input":{"ok":true}}}"#,
        )
        .unwrap();
        let run = get_run_from_selector(dir.path(), "fn").unwrap();
        assert_eq!(run.payload.input.unwrap()["ok"], true);
    }

    #[test]
    fn selector_errors_when_empty() {
        let dir = tempdir().unwrap();
        let err = get_run_from_selector(dir.path(), "fn").unwrap_err();
        assert!(err.to_string().contains("No logs found"));
    }

    #[test]
    fn identifier_from_filename_takes_last_token() {
        assert_eq!(
            get_identifier_from_filename("20240522_150641_827Z_extensions_fn_abcdef.json"),
            "abcdef"
        );
    }

    #[test]
    fn wasm_mtime_changed_detects_newer() {
        let older = SystemTime::UNIX_EPOCH;
        let newer = older + Duration::from_secs(1);
        assert!(wasm_mtime_changed(Some(older), Some(newer)));
        assert!(!wasm_mtime_changed(Some(newer), Some(older)));
        assert!(!wasm_mtime_changed(Some(newer), Some(newer)));
        assert!(wasm_mtime_changed(None, Some(newer)));
        assert!(!wasm_mtime_changed(None, None));
    }
}
