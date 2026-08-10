use crate::error::AppError;
use crate::models::loader::{load_app, LoadAppOptions, LoadedApp};
use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppInfoFormat {
    Text,
    Json,
}

#[derive(Debug, Clone)]
pub struct AppInfoOptions {
    pub directory: PathBuf,
    pub config_name: Option<String>,
    pub format: AppInfoFormat,
    pub web_env: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct AppInfoJson {
    pub name: String,
    pub directory: String,
    pub configuration_path: String,
    pub client_id: Option<String>,
    pub application_url: Option<String>,
    pub scopes: Vec<String>,
    pub extensions: Vec<ExtensionInfoJson>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExtensionInfoJson {
    pub handle: String,
    pub type_name: String,
    pub directory: String,
    pub bundle_url: String,
}

#[derive(Debug, Clone)]
pub enum AppInfoResult {
    Text(String),
    Json(AppInfoJson),
}

pub fn app_info(options: AppInfoOptions) -> Result<AppInfoResult, AppError> {
    let app = load_app(LoadAppOptions {
        directory: options.directory,
        config_name: options.config_name,
    })?;
    let json = to_json(&app);
    if options.format == AppInfoFormat::Json {
        return Ok(AppInfoResult::Json(json));
    }

    let mut lines = Vec::new();
    lines.push(format!("App name: {}", app.name));
    lines.push(format!(
        "Client ID: {}",
        app.client_id().unwrap_or("(not linked)")
    ));
    lines.push(format!(
        "Configuration: {}",
        app.configuration_path.display()
    ));
    if let Some(url) = &app.configuration.application_url {
        lines.push(format!("Application URL: {url}"));
    }
    let scopes = app.configuration.scopes();
    if !scopes.is_empty() {
        lines.push(format!("Scopes: {}", scopes.join(", ")));
    }
    if let Some(store) = app
        .configuration
        .build
        .as_ref()
        .and_then(|b| b.dev_store_url.as_ref())
        .or(app.hidden_config.dev_store_url.as_ref())
    {
        lines.push(format!("Dev store: {store}"));
    }
    lines.push(format!("Extensions ({})", app.extensions.len()));
    for ext in &app.extensions {
        lines.push(format!(
            "  - {} ({}) → {}",
            ext.handle,
            ext.type_name(),
            ext.bundle_url()
        ));
    }
    if options.web_env {
        lines.push(String::new());
        lines.push("Web environment:".into());
        if let Some(id) = app.client_id() {
            lines.push(format!("  SHOPIFY_API_KEY={id}"));
        }
        if let Some(url) = &app.configuration.application_url {
            lines.push(format!("  HOST={url}"));
        }
    }
    if !app.errors.is_empty() {
        lines.push(String::new());
        lines.push("Errors:".into());
        for e in &app.errors {
            lines.push(format!("  - {e}"));
        }
    }
    Ok(AppInfoResult::Text(lines.join("\n")))
}

fn to_json(app: &LoadedApp) -> AppInfoJson {
    AppInfoJson {
        name: app.name.clone(),
        directory: app.directory.display().to_string(),
        configuration_path: app.configuration_path.display().to_string(),
        client_id: app.configuration.client_id.clone(),
        application_url: app.configuration.application_url.clone(),
        scopes: app.configuration.scopes(),
        extensions: app
            .extensions
            .iter()
            .map(|e| ExtensionInfoJson {
                handle: e.handle.clone(),
                type_name: e.type_name().to_string(),
                directory: e.directory.display().to_string(),
                bundle_url: e.bundle_url(),
            })
            .collect(),
        errors: app.errors.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn info_text_and_json() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("shopify.app.toml"),
            "client_id = \"abc\"\nname = \"Demo\"\napplication_url = \"https://example.com\"\n",
        )
        .unwrap();
        let text = app_info(AppInfoOptions {
            directory: dir.path().to_path_buf(),
            config_name: None,
            format: AppInfoFormat::Text,
            web_env: false,
        })
        .unwrap();
        match text {
            AppInfoResult::Text(s) => assert!(s.contains("Demo")),
            _ => panic!("expected text"),
        }
        let json = app_info(AppInfoOptions {
            directory: dir.path().to_path_buf(),
            config_name: None,
            format: AppInfoFormat::Json,
            web_env: false,
        })
        .unwrap();
        match json {
            AppInfoResult::Json(j) => assert_eq!(j.client_id.as_deref(), Some("abc")),
            _ => panic!("expected json"),
        }
    }
}
