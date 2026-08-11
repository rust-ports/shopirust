//! Sync wrapper kept for any remaining call sites outside async contexts.

use crate::error::AppError;
use crate::models::extensions::ExtensionInstance;
use crate::services::function::{build_function_extension as build_fn, FunctionBuildOptions};
use std::path::PathBuf;

/// Build or copy a function wasm artifact (blocking). Prefer the async API from
/// `services::function` when already on a tokio runtime.
pub fn build_function_extension(ext: &ExtensionInstance) -> Result<PathBuf, AppError> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| AppError::message(e.to_string()))?;
    rt.block_on(build_fn(
        ext,
        FunctionBuildOptions {
            use_tasks: false,
        },
    ))
}
