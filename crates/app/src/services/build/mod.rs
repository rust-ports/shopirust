pub mod function;
pub mod include_assets;
pub mod theme;
pub mod ui;

use crate::error::AppError;
use crate::models::extensions::ExtensionFeature;
use crate::models::loader::{load_app, LoadAppOptions, LoadedApp};
use crate::services::build::include_assets::include_assets_step;
use crate::services::build::theme::build_theme_extension;
use crate::services::build::ui::build_ui_extension;
use crate::services::function::{build_function_extension, FunctionBuildOptions};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone)]
pub struct BuildOptions {
    pub directory: PathBuf,
    pub config_name: Option<String>,
    pub skip_dependencies_installation: bool,
}

#[derive(Debug, Clone)]
pub struct BuildResult {
    pub built: Vec<String>,
    pub skipped: Vec<String>,
    pub errors: Vec<String>,
}

pub async fn build_app(options: BuildOptions) -> Result<BuildResult, AppError> {
    let app = load_app(LoadAppOptions {
        directory: options.directory.clone(),
        config_name: options.config_name,
                ignore_unknown_extensions: false,
        })?;

    let mut result = BuildResult {
        built: vec![],
        skipped: vec![],
        errors: vec![],
    };

    if !options.skip_dependencies_installation {
        install_web_dependencies(&options.directory, &mut result);
    }
    build_webs(&options.directory, &mut result);

    for ext in &app.extensions {
        match build_extension(ext, &app).await {
            Ok(label) => result.built.push(label),
            Err(e) => result.errors.push(format!("{}: {e}", ext.handle)),
        }
    }

    if !result.errors.is_empty() {
        return Err(AppError::message(format!(
            "Build failed:\n{}",
            result.errors.join("\n")
        )));
    }
    Ok(result)
}

async fn build_extension(
    ext: &crate::models::extensions::ExtensionInstance,
    _app: &LoadedApp,
) -> Result<String, AppError> {
    include_assets_step(ext)?;
    if ext.is_theme_extension() {
        build_theme_extension(ext)?;
        return Ok(format!("theme:{}", ext.handle));
    }
    if ext.is_function_extension()
        || ext
            .specification
            .features
            .contains(&ExtensionFeature::Function)
    {
        build_function_extension(
            ext,
            FunctionBuildOptions {
                use_tasks: false,
            },
        )
        .await?;
        return Ok(format!("function:{}", ext.handle));
    }
    if ext.is_ui_extension() {
        build_ui_extension(ext)?;
        return Ok(format!("ui:{}", ext.handle));
    }
    // Generic: copy assets only
    Ok(format!("assets:{}", ext.handle))
}

fn install_web_dependencies(directory: &Path, result: &mut BuildResult) {
    let web = directory.join("web");
    if !web.is_dir() {
        return;
    }
    let pm = detect_package_manager(&web);
    let status = Command::new(&pm).arg("install").current_dir(&web).status();
    match status {
        Ok(s) if s.success() => result.built.push(format!("web-deps:{pm}")),
        Ok(_) => result.skipped.push("web-deps (install failed)".into()),
        Err(_) => result.skipped.push(format!("web-deps ({pm} not found)")),
    }
}

fn build_webs(directory: &Path, result: &mut BuildResult) {
    let web = directory.join("web");
    if !web.is_dir() {
        return;
    }
    let pm = detect_package_manager(&web);
    let status = Command::new(&pm)
        .args(["run", "build"])
        .current_dir(&web)
        .status();
    match status {
        Ok(s) if s.success() => result.built.push("web:build".into()),
        Ok(_) => result
            .skipped
            .push("web:build (script failed or missing)".into()),
        Err(_) => result
            .skipped
            .push("web:build (package manager missing)".into()),
    }
}

pub(crate) fn detect_package_manager(web: &Path) -> String {
    if web.join("pnpm-lock.yaml").exists() {
        "pnpm".into()
    } else if web.join("yarn.lock").exists() {
        "yarn".into()
    } else {
        "npm".into()
    }
}

/// Build extensions into their output paths for deploy (used when `--no-build` is absent).
pub async fn bundle_and_build_extensions(app: &mut LoadedApp) -> Result<(), AppError> {
    for ext in &mut app.extensions {
        include_assets_step(ext)?;
        if ext.is_theme_extension() {
            let out = build_theme_extension(ext)?;
            ext.output_path = Some(out);
        } else if ext.is_function_extension() {
            let out = build_function_extension(
                ext,
                FunctionBuildOptions {
                    use_tasks: false,
                },
            )
            .await?;
            ext.output_path = Some(out);
        } else if ext.is_ui_extension() {
            let out = build_ui_extension(ext)?;
            ext.output_path = Some(out);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[tokio::test]
    async fn build_theme_extension_copies_files() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("shopify.app.toml"),
            "client_id = \"abc\"\nname = \"Demo\"\napplication_url = \"https://e.com\"\n",
        )
        .unwrap();
        let ext = dir.path().join("extensions/my-theme");
        fs::create_dir_all(ext.join("blocks")).unwrap();
        fs::write(
            ext.join("shopify.extension.toml"),
            "type = \"theme\"\nhandle = \"my-theme\"\n",
        )
        .unwrap();
        fs::write(
            ext.join("blocks/star.liquid"),
            "{% schema %}{}{% endschema %}",
        )
        .unwrap();

        let result = build_app(BuildOptions {
            directory: dir.path().to_path_buf(),
            config_name: None,
            skip_dependencies_installation: true,
        })
        .await
        .unwrap();
        assert!(result.built.iter().any(|b| b.contains("my-theme")));
        assert!(ext.join("dist").exists() || ext.join("blocks/star.liquid").exists());
    }

    #[test]
    fn detect_package_manager_prefers_pnpm_then_yarn() {
        let dir = tempdir().unwrap();
        assert_eq!(detect_package_manager(dir.path()), "npm");
        fs::write(dir.path().join("yarn.lock"), "").unwrap();
        assert_eq!(detect_package_manager(dir.path()), "yarn");
        fs::write(dir.path().join("pnpm-lock.yaml"), "").unwrap();
        assert_eq!(detect_package_manager(dir.path()), "pnpm");
    }
}
