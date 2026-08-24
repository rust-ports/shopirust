//! Banner body used by `app info` / `app dev` for the active config file.

use crate::models::loader::LoadedApp;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigInfoBody {
    pub lines: Vec<String>,
}

impl ConfigInfoBody {
    pub fn render_text(&self) -> String {
        self.lines.join("\n")
    }
}

pub fn format_config_info_body(app: &LoadedApp) -> ConfigInfoBody {
    let mut lines = Vec::new();
    let file_name = app
        .configuration_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("shopify.app.toml");
    lines.push(format!("Using {file_name}"));
    if let Some(client_id) = app.client_id() {
        lines.push(format!("Client ID: {client_id}"));
    }
    if !app.name.is_empty() {
        lines.push(format!("Name: {}", app.name));
    }
    if let Some(url) = app.configuration.application_url.as_deref() {
        lines.push(format!("Application URL: {url}"));
    }
    let ext_count = app
        .extensions
        .iter()
        .filter(|e| !e.is_app_config_extension())
        .count();
    lines.push(format!("Extensions: {ext_count}"));
    ConfigInfoBody { lines }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::app::AppConfiguration;
    use crate::models::loader::LoadedApp;
    use std::path::PathBuf;

    #[test]
    fn includes_file_and_counts() {
        let app = LoadedApp {
            directory: PathBuf::from("/app"),
            configuration_path: PathBuf::from("/app/shopify.app.staging.toml"),
            configuration: AppConfiguration {
                client_id: Some("gid://app/1".into()),
                name: Some("Demo".into()),
                application_url: Some("https://example.com".into()),
                ..Default::default()
            },
            hidden_config: Default::default(),
            extensions: vec![],
            webs: vec![],
            identifiers: Default::default(),
            name: "Demo".into(),
            errors: vec![],
            dev_application_urls: None,
        };
        let body = format_config_info_body(&app);
        let text = body.render_text();
        assert!(text.contains("shopify.app.staging.toml"));
        assert!(text.contains("gid://app/1"));
        assert!(text.contains("Extensions: 0"));
    }
}
