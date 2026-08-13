//! Build Shopify Functions (command / cargo / JS+Javy toolchain).

use crate::error::AppError;
use crate::models::extensions::extension_instance::ExtensionInstance;
use crate::services::function::binaries::{
    derive_javascript_binary_dependencies, download_binary, javy_binary, javy_plugin_binary,
    trampoline_binary, wasm_opt_binary, BinaryDependencies, V1_TRAMPOLINE_VERSION,
    V2_TRAMPOLINE_VERSION,
};
use crate::services::function::schema_version::validate_schema_api_version;
use crate::services::init::hyphenate_name;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const PREFERRED_FUNCTION_NPM_PACKAGE_MAJOR_VERSION: &str = "2";
const JAVY_WORLD: &str = "shopify-function";

#[derive(Debug, Clone, Default)]
pub struct FunctionBuildOptions {
    /// When true, print step progress to stdout.
    pub use_tasks: bool,
}

/// Build a function extension to wasm (primary entry for `app function build` and `app build`).
pub async fn build_function_extension(
    ext: &ExtensionInstance,
    options: FunctionBuildOptions,
) -> Result<PathBuf, AppError> {
    if let Some(api_version) = ext.api_version() {
        validate_schema_api_version(&ext.directory, ext.local_identifier(), api_version)?;
    }

    let dist = ext.directory.join("dist");
    fs::create_dir_all(&dist)?;

    if let Some(cmd) = ext.build_command() {
        if ext.typegen_command().is_some() || ext.is_javascript() {
            if let Some(ref typegen) = ext.typegen_command() {
                run_typegen_command(ext, typegen)?;
            } else if ext.is_javascript() {
                build_graphql_types(ext).await?;
            }
        }
        run_shell_command(&cmd, &ext.directory)?;
        let out = ensure_wasm_output(ext)?;
        post_process_wasm(ext, &out).await?;
        return Ok(out);
    }

    if ext.is_javascript() {
        if options.use_tasks {
            eprintln!("Building function {}...", ext.handle);
        }
        build_js_function(ext, &options).await?;
        let out = ensure_wasm_output(ext)?;
        post_process_wasm(ext, &out).await?;
        return Ok(out);
    }

    // Prefer existing wasm artifacts (already built).
    if let Some(existing) = find_existing_wasm(ext) {
        let dest = ext.function_output_path();
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        if existing != dest {
            fs::copy(&existing, &dest)?;
        }
        post_process_wasm(ext, &dest).await?;
        return Ok(dest);
    }

    // Cargo wasm build (WASI first, then wasm32-unknown-unknown).
    if ext.directory.join("Cargo.toml").exists() {
        if let Some(ref typegen) = ext.typegen_command() {
            run_typegen_command(ext, typegen)?;
        }
        if let Some(wasm) = cargo_build_wasm(ext) {
            let dest = ext.function_output_path();
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&wasm, &dest)?;
            post_process_wasm(ext, &dest).await?;
            return Ok(dest);
        }
    }

    if let Some(ref typegen) = ext.typegen_command() {
        // Non-JS without build.command still needs an explicit build command after typegen.
        let _ = typegen;
        return Err(AppError::message(format!(
            "The function extension {} doesn't have a build command or it's empty.\n\
             Edit the shopify.function.extension.toml configuration file and set how to build the extension.\n\n\
             [build]\ncommand = \"{{COMMAND}}\"\n\n\
             Note that the command must output a dist/index.wasm file.",
            ext.local_identifier()
        )));
    }

    Err(AppError::message(format!(
        "Function extension '{}' has no wasm artifact. Build it first or place index.wasm in dist/.",
        ext.handle
    )))
}

const CARGO_WASM_TARGETS: &[&str] = &["wasm32-wasip1", "wasm32-unknown-unknown"];

fn cargo_build_wasm(ext: &ExtensionInstance) -> Option<PathBuf> {
    for target in CARGO_WASM_TARGETS {
        let status = Command::new("cargo")
            .args(["build", "--release", "--target", target])
            .current_dir(&ext.directory)
            .status();
        if let Ok(s) = status {
            if s.success() {
                if let Some(wasm) = find_cargo_wasm(ext, target) {
                    return Some(wasm);
                }
            }
        }
    }
    None
}

fn find_existing_wasm(ext: &ExtensionInstance) -> Option<PathBuf> {
    let crate_name = ext.handle.replace('-', "_");
    let mut candidates = vec![
        ext.directory.join("dist/index.wasm"),
        ext.directory.join("index.wasm"),
    ];
    for target in CARGO_WASM_TARGETS {
        candidates.push(
            ext.directory
                .join("target")
                .join(target)
                .join("release")
                .join(format!("{crate_name}.wasm")),
        );
    }
    candidates.into_iter().find(|p| p.exists())
}

fn find_cargo_wasm(ext: &ExtensionInstance, target: &str) -> Option<PathBuf> {
    let dir = ext.directory.join("target").join(target).join("release");
    let entries = fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        if entry.path().extension().and_then(|e| e.to_str()) == Some("wasm") {
            return Some(entry.path());
        }
    }
    None
}

fn ensure_wasm_output(ext: &ExtensionInstance) -> Result<PathBuf, AppError> {
    let dest = ext.function_output_path();
    if dest.is_file() {
        return Ok(dest);
    }
    if let Some(existing) = find_existing_wasm(ext) {
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        if existing != dest {
            fs::copy(&existing, &dest)?;
        }
        return Ok(dest);
    }
    Err(AppError::message(format!(
        "Function build for '{}' did not produce {}",
        ext.handle,
        dest.display()
    )))
}

fn run_shell_command(command: &str, cwd: &Path) -> Result<(), AppError> {
    #[cfg(unix)]
    let status = Command::new("sh")
        .args(["-c", command])
        .current_dir(cwd)
        .status();
    #[cfg(not(unix))]
    let status = Command::new("cmd")
        .args(["/C", command])
        .current_dir(cwd)
        .status();

    let status =
        status.map_err(|e| AppError::message(format!("Failed to run build command: {e}")))?;
    if !status.success() {
        return Err(AppError::message(format!(
            "Build command failed with {:?}",
            status.code()
        )));
    }
    Ok(())
}

fn run_typegen_command(ext: &ExtensionInstance, typegen: &str) -> Result<(), AppError> {
    run_shell_command(typegen, &ext.directory)
}

/// Generate GraphQL types via `typegen_command` or `graphql-code-generator` for JS functions.
pub async fn build_graphql_types(ext: &ExtensionInstance) -> Result<(), AppError> {
    if let Some(typegen) = ext.typegen_command() {
        return run_typegen_command(ext, &typegen);
    }
    if !ext.is_javascript() {
        return Err(AppError::message(
            "No typegen_command specified. Set build.typegen_command in your function extension TOML \
             to generate GraphQL types for non-JavaScript functions.",
        ));
    }

    let pm = detect_package_manager(&ext.directory);
    let status = Command::new(&pm)
        .args([
            "exec",
            "--",
            "graphql-code-generator",
            "--config",
            "package.json",
        ])
        .current_dir(&ext.directory)
        .status()
        .map_err(|e| AppError::message(format!("Failed to run graphql-code-generator: {e}")))?;
    if !status.success() {
        // Try without the `--` separator (pnpm style)
        let status = Command::new(&pm)
            .args(["exec", "graphql-code-generator", "--config", "package.json"])
            .current_dir(&ext.directory)
            .status()
            .map_err(|e| AppError::message(format!("Failed to run graphql-code-generator: {e}")))?;
        if !status.success() {
            return Err(AppError::message(
                "graphql-code-generator failed. Ensure dependencies are installed.",
            ));
        }
    }
    Ok(())
}

fn detect_package_manager(dir: &Path) -> String {
    if dir.join("pnpm-lock.yaml").exists() {
        "pnpm".into()
    } else if dir.join("yarn.lock").exists() {
        "yarn".into()
    } else {
        "npm".into()
    }
}

pub fn validate_shopify_function_package_version(
    ext: &ExtensionInstance,
) -> Result<BinaryDependencies, AppError> {
    let package_json_path = ext.shopify_function_package_json().ok_or_else(|| {
        AppError::message(format!(
            "Could not find the Shopify Functions JavaScript library.\n\
             Make sure you have a compatible version of the @shopify/shopify_function library installed.\n\
             Add \"@shopify/shopify_function\": \"~{PREFERRED_FUNCTION_NPM_PACKAGE_MAJOR_VERSION}.0.0\" \
             to the dependencies section of the package.json file in your function's directory."
        ))
    })?;

    let package_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(package_json_path)?)?;
    let version = package_json
        .get("version")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::message("Invalid @shopify/shopify_function package.json"))?;
    let major = version.split('.').next().unwrap_or("");
    derive_javascript_binary_dependencies(major).ok_or_else(|| {
        AppError::message(
            "The installed version of the Shopify Functions JavaScript library is not compatible \
             with this version of Shopify CLI.",
        )
    })
}

async fn build_js_function(
    ext: &ExtensionInstance,
    options: &FunctionBuildOptions,
) -> Result<(), AppError> {
    let deps = validate_shopify_function_package_version(ext)?;
    let exports = js_exports(ext)?;

    if options.use_tasks {
        eprintln!("Building GraphQL types...");
    }
    build_graphql_types(ext).await?;

    if options.use_tasks {
        eprintln!("Bundling JS function...");
    }
    if exports.is_empty() {
        bundle_default(ext)?;
    } else {
        bundle_with_exports(ext, &exports)?;
    }

    if options.use_tasks {
        eprintln!("Running javy...");
    }
    if exports.is_empty() {
        run_javy(ext, &deps, &[]).await?;
    } else {
        run_javy_with_wit(ext, &deps, &exports).await?;
    }

    if options.use_tasks {
        eprintln!("Done!");
    }
    Ok(())
}

fn find_shopify_function_file(ext: &ExtensionInstance, file: &str) -> Option<PathBuf> {
    let relative = Path::new("node_modules/@shopify/shopify_function").join(file);
    let mut dir = Some(ext.directory.as_path());
    while let Some(current) = dir {
        let candidate = current.join(&relative);
        if candidate.is_file() {
            return Some(candidate);
        }
        dir = current.parent();
    }
    None
}

fn bundle_default(ext: &ExtensionInstance) -> Result<(), AppError> {
    let entry_point = find_shopify_function_file(ext, "index.ts")
        .or_else(|| find_shopify_function_file(ext, "index.js"))
        .ok_or_else(|| {
            AppError::message("Could not find the Shopify Functions JavaScript library.")
        })?;
    let _run = find_shopify_function_file(ext, "run.ts")
        .or_else(|| find_shopify_function_file(ext, "run.js"))
        .ok_or_else(|| {
            AppError::message("Could not find the Shopify Functions JavaScript library.")
        })?;
    let user_function = ext.entry_path.clone().ok_or_else(|| {
        AppError::message(
            "Could not find your function entry point. It must be in src/index.js or src/index.ts",
        )
    })?;

    run_esbuild(&ext.directory, &entry_point, &user_function, None)
}

fn bundle_with_exports(ext: &ExtensionInstance, exports: &[String]) -> Result<(), AppError> {
    let _entry = find_shopify_function_file(ext, "index.ts")
        .or_else(|| find_shopify_function_file(ext, "index.js"))
        .ok_or_else(|| {
            AppError::message("Could not find the Shopify Functions JavaScript library.")
        })?;
    let user_function = ext.entry_path.clone().ok_or_else(|| {
        AppError::message(
            "Could not find your function entry point. It must be in src/index.js or src/index.ts",
        )
    })?;

    let contents = export_entrypoint_contents(exports);
    run_esbuild(
        &ext.directory,
        Path::new("stdin.ts"),
        &user_function,
        Some(&contents),
    )
}

fn export_entrypoint_contents(exports: &[String]) -> String {
    let prelude = "\nimport __runFunction from \"@shopify/shopify_function/run\"";
    let bodies: Vec<String> = exports
        .iter()
        .map(|name| {
            let identifier = camelize(name);
            let alias = camelize(&format!("run-{name}"));
            format!(
                "\nimport {{ {identifier} as {alias} }} from \"user-function\"\n\
                 export function {identifier}() {{ return __runFunction({alias}) }}"
            )
        })
        .collect();
    format!("{prelude}\n{}", bodies.join("\n"))
}

fn wit_for_exports(exports: &[String]) -> String {
    let wit_exports: Vec<String> = exports
        .iter()
        .map(|name| format!("export %{}: func();", hyphenate_name(name)))
        .collect();
    format!(
        "package function:impl;\n\nworld {JAVY_WORLD} {{\n  {}\n}}",
        wit_exports.join("\n  ")
    )
}

fn run_esbuild(
    directory: &Path,
    entry_point: &Path,
    user_function: &Path,
    stdin_contents: Option<&str>,
) -> Result<(), AppError> {
    fs::create_dir_all(directory.join("dist"))?;
    let outfile = directory.join("dist/function.js");
    let alias = format!("user-function:{}", user_function.display());

    // Prefer local esbuild, then npx.
    let mut attempts: Vec<(String, Vec<String>)> = vec![];
    let local = directory.join("node_modules/.bin/esbuild");
    if local.is_file() {
        if let Some(contents) = stdin_contents {
            let stdin_path = directory.join("dist/.javy-entrypoint.ts");
            fs::write(&stdin_path, contents)?;
            attempts.push((
                local.display().to_string(),
                vec![
                    stdin_path.display().to_string(),
                    "--bundle".into(),
                    "--format=esm".into(),
                    "--target=es2022".into(),
                    format!("--alias:{alias}"),
                    format!("--outfile={}", outfile.display()),
                    "--log-level=silent".into(),
                ],
            ));
        } else {
            attempts.push((
                local.display().to_string(),
                vec![
                    entry_point.display().to_string(),
                    "--bundle".into(),
                    "--format=esm".into(),
                    "--target=es2022".into(),
                    format!("--alias:{alias}"),
                    format!("--outfile={}", outfile.display()),
                    "--log-level=silent".into(),
                ],
            ));
        }
    }

    let npx_args = if let Some(contents) = stdin_contents {
        let stdin_path = directory.join("dist/.javy-entrypoint.ts");
        fs::write(&stdin_path, contents)?;
        vec![
            "--yes".into(),
            "esbuild".into(),
            stdin_path.display().to_string(),
            "--bundle".into(),
            "--format=esm".into(),
            "--target=es2022".into(),
            format!("--alias:{alias}"),
            format!("--outfile={}", outfile.display()),
            "--log-level=silent".into(),
        ]
    } else {
        vec![
            "--yes".into(),
            "esbuild".into(),
            entry_point.display().to_string(),
            "--bundle".into(),
            "--format=esm".into(),
            "--target=es2022".into(),
            format!("--alias:{alias}"),
            format!("--outfile={}", outfile.display()),
            "--log-level=silent".into(),
        ]
    };
    attempts.push(("npx".into(), npx_args));

    let mut last_err = None;
    for (cmd, args) in attempts {
        let status = Command::new(&cmd)
            .args(&args)
            .current_dir(directory)
            .status();
        match status {
            Ok(s) if s.success() => return Ok(()),
            Ok(s) => {
                last_err = Some(AppError::message(format!(
                    "esbuild exited with {:?}",
                    s.code()
                )));
            }
            Err(e) => {
                last_err = Some(AppError::message(format!("Failed to run esbuild: {e}")));
            }
        }
    }
    Err(last_err.unwrap_or_else(|| AppError::message("Failed to bundle JS function")))
}

async fn run_javy(
    ext: &ExtensionInstance,
    deps: &BinaryDependencies,
    extra: &[String],
) -> Result<(), AppError> {
    let javy = javy_binary(&deps.javy)?;
    let plugin = javy_plugin_binary(&deps.javy_plugin);
    download_binary(&javy).await?;
    download_binary(&plugin).await?;

    let out = ext.function_output_path();
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut args = vec![
        "build".into(),
        "-C".into(),
        "dynamic".into(),
        "-C".into(),
        format!("plugin={}", plugin.path.display()),
    ];
    args.extend(extra.iter().cloned());
    args.push("-o".into());
    args.push(out.display().to_string());
    args.push("dist/function.js".into());

    let status = Command::new(&javy.path)
        .args(&args)
        .current_dir(&ext.directory)
        .status()
        .map_err(|e| AppError::message(format!("Failed to run javy: {e}")))?;
    if !status.success() {
        return Err(AppError::message(format!(
            "javy exited with {:?}",
            status.code()
        )));
    }
    Ok(())
}

async fn run_javy_with_wit(
    ext: &ExtensionInstance,
    deps: &BinaryDependencies,
    exports: &[String],
) -> Result<(), AppError> {
    let tmp = tempfile::tempdir().map_err(|e| AppError::message(e.to_string()))?;
    let wit_path = tmp.path().join("javy-world.wit");
    fs::write(&wit_path, wit_for_exports(exports))?;
    let extra = vec![
        "-C".into(),
        format!("wit={}", wit_path.display()),
        "-C".into(),
        format!("wit-world={JAVY_WORLD}"),
    ];
    run_javy(ext, deps, &extra).await
}

async fn post_process_wasm(ext: &ExtensionInstance, module_path: &Path) -> Result<(), AppError> {
    run_trampoline(module_path).await?;
    if ext.wasm_opt_enabled() {
        run_wasm_opt(module_path).await?;
    }
    Ok(())
}

pub async fn run_wasm_opt(module_path: &Path) -> Result<(), AppError> {
    let wasm_opt = wasm_opt_binary();
    download_binary(&wasm_opt).await?;
    let status = Command::new("node")
        .args([
            wasm_opt.name.as_str(),
            &module_path.display().to_string(),
            "-Oz",
            "--enable-bulk-memory",
            "--enable-multimemory",
            "--enable-nontrapping-float-to-int",
            "--strip-debug",
            "-o",
            &module_path.display().to_string(),
        ])
        .current_dir(wasm_opt.path.parent().unwrap_or_else(|| Path::new(".")))
        .status()
        .map_err(|e| AppError::message(format!("Failed to run wasm-opt: {e}")))?;
    if !status.success() {
        return Err(AppError::message(format!(
            "wasm-opt exited with {:?}",
            status.code()
        )));
    }
    Ok(())
}

pub async fn run_trampoline(module_path: &Path) -> Result<(), AppError> {
    let imported = imported_wasm_modules(module_path)?;
    let trampoline_version = if imported.iter().any(|m| m == "shopify_function_v1") {
        Some(V1_TRAMPOLINE_VERSION)
    } else if imported.iter().any(|m| m == "shopify_function_v2") {
        Some(V2_TRAMPOLINE_VERSION)
    } else {
        None
    };
    let Some(version) = trampoline_version else {
        return Ok(());
    };
    let trampoline = trampoline_binary(version)?;
    download_binary(&trampoline).await?;
    let status = Command::new(&trampoline.path)
        .args([
            "-i",
            &module_path.display().to_string(),
            "-o",
            &module_path.display().to_string(),
        ])
        .status()
        .map_err(|e| AppError::message(format!("Failed to run trampoline: {e}")))?;
    if !status.success() {
        return Err(AppError::message(format!(
            "trampoline exited with {:?}",
            status.code()
        )));
    }
    Ok(())
}

/// Extract unique import module names from a wasm binary (deterministic order).
pub fn imported_wasm_modules(module_path: &Path) -> Result<Vec<String>, AppError> {
    let bytes = fs::read(module_path)?;
    if bytes.len() < 8 || &bytes[0..4] != b"\0asm" {
        return Ok(vec![]);
    }
    let mut modules = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut i = 8usize;
    while i + 1 < bytes.len() {
        let section_id = bytes[i];
        i += 1;
        let Some((size, read)) = read_u32_leb128(&bytes[i..]) else {
            break;
        };
        i += read;
        let end = (i + size as usize).min(bytes.len());
        if section_id == 2 {
            // import section
            let mut p = i;
            let Some((count, c_read)) = read_u32_leb128(&bytes[p..end]) else {
                break;
            };
            p += c_read;
            for _ in 0..count {
                let Some((mod_len, m_read)) = read_u32_leb128(&bytes[p..end]) else {
                    break;
                };
                p += m_read;
                if p + mod_len as usize > end {
                    break;
                }
                let name = String::from_utf8_lossy(&bytes[p..p + mod_len as usize]).to_string();
                p += mod_len as usize;
                if seen.insert(name.clone()) {
                    modules.push(name);
                }
                let Some((field_len, f_read)) = read_u32_leb128(&bytes[p..end]) else {
                    break;
                };
                p += f_read + field_len as usize;
                if p >= end {
                    break;
                }
                let kind = bytes[p];
                p += 1;
                // skip import descriptor
                match kind {
                    0 => {
                        // func
                        let Some((_, r)) = read_u32_leb128(&bytes[p..end]) else {
                            break;
                        };
                        p += r;
                    }
                    1 => {
                        // table
                        if p < end {
                            p += 1;
                        } // reftype
                        p = skip_limits(&bytes, p, end);
                    }
                    2 => {
                        // mem
                        p = skip_limits(&bytes, p, end);
                    }
                    3 => {
                        // global
                        if p + 1 < end {
                            p += 2;
                        }
                    }
                    _ => break,
                }
            }
        }
        i = end;
    }
    Ok(modules)
}

fn read_u32_leb128(bytes: &[u8]) -> Option<(u32, usize)> {
    let mut result = 0u32;
    let mut shift = 0;
    for (idx, b) in bytes.iter().enumerate() {
        result |= u32::from(b & 0x7f) << shift;
        if b & 0x80 == 0 {
            return Some((result, idx + 1));
        }
        shift += 7;
        if shift > 35 {
            return None;
        }
    }
    None
}

fn skip_limits(bytes: &[u8], mut p: usize, end: usize) -> usize {
    if p >= end {
        return p;
    }
    let flags = bytes[p];
    p += 1;
    if let Some((_, r)) = read_u32_leb128(&bytes[p..end]) {
        p += r;
    }
    if flags & 1 != 0 {
        if let Some((_, r)) = read_u32_leb128(&bytes[p..end]) {
            p += r;
        }
    }
    p
}

/// Resolve export names from targeting configuration.
pub fn js_exports(ext: &ExtensionInstance) -> Result<Vec<String>, AppError> {
    let targets = ext.targeting();
    let without_export: Vec<_> = targets.iter().filter(|t| t.export.is_none()).collect();
    let with_export: Vec<_> = targets.iter().filter(|t| t.export.is_some()).collect();

    if targets.len() > 1 && !without_export.is_empty() {
        let list = without_export
            .iter()
            .map(|t| format!("- '{}'", t.target))
            .collect::<Vec<_>>()
            .join("\n");
        return Err(AppError::message(format!(
            "Can't infer export name for targets:\n{list}\n\
             All targets must have an export when multiple targets are present."
        )));
    }

    let invalid: Vec<_> = with_export
        .iter()
        .filter(|t| {
            !t.export.as_ref().is_some_and(|e| {
                e.chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
            })
        })
        .collect();
    if !invalid.is_empty() {
        let names: Vec<_> = invalid
            .iter()
            .filter_map(|t| t.export.as_deref())
            .map(|e| format!("'{e}'"))
            .collect();
        return Err(AppError::message(format!(
            "Invalid export names: {}.\n\n\
             The TOML's exports must be kebab-case (lowercase, hyphen or numbers) to comply with WebAssembly's Component Model.",
            names.join(", ")
        )));
    }

    Ok(with_export
        .into_iter()
        .filter_map(|t| t.export.clone())
        .collect())
}

fn camelize(input: &str) -> String {
    let mut result = String::new();
    let mut capitalize = false;
    for (i, c) in input.chars().enumerate() {
        if c == '-' || c == '_' || c == ' ' {
            capitalize = true;
            continue;
        }
        if i == 0 {
            result.extend(c.to_lowercase());
        } else if capitalize {
            result.extend(c.to_uppercase());
            capitalize = false;
        } else {
            result.push(c);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::extensions::create_extension_specification;
    use serde_json::json;
    use std::collections::HashMap;
    use tempfile::tempdir;

    fn make_ext(dir: &Path, config: serde_json::Value) -> ExtensionInstance {
        let mut map = HashMap::new();
        if let Some(obj) = config.as_object() {
            for (k, v) in obj {
                map.insert(k.clone(), v.clone());
            }
        }
        let spec = create_extension_specification("function").unwrap();
        let mut ext = ExtensionInstance::new(
            "my-fn",
            dir.to_path_buf(),
            dir.join("shopify.extension.toml"),
            map,
            spec,
        );
        if dir.join("src/index.js").exists() {
            ext.entry_path = Some(dir.join("src/index.js"));
        }
        if dir.join("src/index.ts").exists() {
            ext.entry_path = Some(dir.join("src/index.ts"));
        }
        ext
    }

    fn create_wasm_module(import_module_name: &str) -> Vec<u8> {
        let name = import_module_name.as_bytes();
        let mut import_content = Vec::new();
        import_content.push(0x01); // count
        import_content.push(name.len() as u8);
        import_content.extend_from_slice(name);
        import_content.extend_from_slice(&[0x03, b'f', b'o', b'o', 0x00, 0x00]); // field "foo", func type 0

        let mut module = vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];
        // type section
        module.extend_from_slice(&[0x01, 0x04, 0x01, 0x60, 0x00, 0x00]);
        // import section
        module.push(0x02);
        module.push(import_content.len() as u8);
        module.extend_from_slice(&import_content);
        module
    }

    #[test]
    fn imported_modules_from_wasm() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("mod.wasm");
        fs::write(&path, create_wasm_module("shopify_function_v2")).unwrap();
        let mods = imported_wasm_modules(&path).unwrap();
        assert_eq!(mods, vec!["shopify_function_v2".to_string()]);
    }

    #[test]
    fn js_exports_single_without_export_ok() {
        let dir = tempdir().unwrap();
        let ext = make_ext(
            dir.path(),
            json!({
                "targeting": [{ "target": "cart.transform.run" }]
            }),
        );
        assert!(js_exports(&ext).unwrap().is_empty());
    }

    #[test]
    fn js_exports_multiple_requires_export() {
        let dir = tempdir().unwrap();
        let ext = make_ext(
            dir.path(),
            json!({
                "targeting": [
                    { "target": "a" },
                    { "target": "b", "export": "run-b" }
                ]
            }),
        );
        assert!(js_exports(&ext)
            .unwrap_err()
            .to_string()
            .contains("Can't infer"));
    }

    #[test]
    fn js_exports_rejects_invalid_names() {
        let dir = tempdir().unwrap();
        let ext = make_ext(
            dir.path(),
            json!({
                "targeting": [{ "target": "a", "export": "runExport" }]
            }),
        );
        assert!(js_exports(&ext)
            .unwrap_err()
            .to_string()
            .contains("Invalid export names"));
    }

    #[test]
    fn js_exports_collects_kebab() {
        let dir = tempdir().unwrap();
        let ext = make_ext(
            dir.path(),
            json!({
                "targeting": [
                    { "target": "a", "export": "run-a" },
                    { "target": "b", "export": "run-b" }
                ]
            }),
        );
        assert_eq!(js_exports(&ext).unwrap(), vec!["run-a", "run-b"]);
    }

    #[tokio::test]
    async fn typegen_errors_for_non_js_without_command() {
        let dir = tempdir().unwrap();
        let ext = make_ext(dir.path(), json!({ "type": "function" }));
        let err = build_graphql_types(&ext).await.unwrap_err();
        assert!(err.to_string().contains("No typegen_command"));
    }

    #[tokio::test]
    async fn typegen_runs_custom_command() {
        let dir = tempdir().unwrap();
        let marker = dir.path().join("types.generated");
        let ext = make_ext(
            dir.path(),
            json!({
                "build": {
                    "typegen_command": format!("touch {}", marker.display())
                }
            }),
        );
        build_graphql_types(&ext).await.unwrap();
        assert!(marker.exists());
    }

    #[tokio::test]
    async fn build_copies_existing_wasm() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("dist")).unwrap();
        // Minimal valid-looking bytes; trampoline/wasm-opt skipped for tiny invalid modules
        // when imports are empty / validate fails — post_process no-ops trampoline.
        fs::write(dir.path().join("dist/index.wasm"), b"\0asm\x01\0\0\0").unwrap();
        let ext = make_ext(
            dir.path(),
            json!({
                "type": "function",
                "api_version": "2024-01",
                "build": { "wasm_opt": false }
            }),
        );
        // Avoid network for wasm-opt in unit test (wasm_opt: false).
        let out = build_function_extension(&ext, FunctionBuildOptions { use_tasks: false })
            .await
            .unwrap();
        assert!(out.exists());
    }

    #[test]
    fn camelize_basic() {
        assert_eq!(camelize("run-discount"), "runDiscount");
        assert_eq!(camelize("export"), "export");
    }

    #[test]
    fn wit_world_generation() {
        let wit = wit_for_exports(&["run-a".into(), "run-b".into()]);
        assert!(wit.contains("export %run-a: func();"));
        assert!(wit.contains("world shopify-function"));
    }

    #[test]
    fn find_cargo_wasm_unknown_unknown_fallback() {
        let dir = tempdir().unwrap();
        let release = dir
            .path()
            .join("target/wasm32-unknown-unknown/release");
        fs::create_dir_all(&release).unwrap();
        fs::write(release.join("my_fn.wasm"), b"\0asm").unwrap();
        let ext = make_ext(dir.path(), json!({}));
        let found = find_cargo_wasm(&ext, "wasm32-unknown-unknown").unwrap();
        assert!(found.ends_with("my_fn.wasm"));
        assert!(find_existing_wasm(&ext).is_some());
    }
}
