use app::services::{
    import_extensions, ExtensionRegistration, ExtensionVersion, ImportExtensionsOptions,
};
use app::{load_app, LoadAppOptions};
use cli_api::MinimalAppIdentifiers;
use cli_core::command::BaseCommand;
use cli_core::error::CliError;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

use super::auth_helpers::authenticated_developer_platform;

#[derive(Debug)]
pub struct ImportExtensions {
    path: String,
    config: Option<String>,
    /// JSON file of extension registrations (offline / fixture-friendly).
    registrations_file: Option<String>,
    extension_type: Option<String>,
    all: bool,
    overwrite: bool,
    client_id: Option<String>,
    reset: bool,
}

impl ImportExtensions {
    pub fn new(
        path: String,
        config: Option<String>,
        registrations_file: Option<String>,
        extension_type: Option<String>,
        all: bool,
        overwrite: bool,
        client_id: Option<String>,
        reset: bool,
    ) -> Self {
        Self {
            path,
            config,
            registrations_file,
            extension_type,
            all,
            overwrite,
            client_id,
            reset,
        }
    }
}

#[async_trait::async_trait]
impl BaseCommand for ImportExtensions {
    fn name() -> &'static str {
        "import-extensions"
    }
    fn topic() -> &'static str {
        "app"
    }
    fn description() -> &'static str {
        "Import dashboard extensions into local TOML files"
    }

    async fn run(&self) -> Result<(), CliError> {
        let _ = (&self.client_id, self.reset);
        let app = load_app(LoadAppOptions {
            directory: PathBuf::from(&self.path),
            config_name: self.config.clone(),
            ignore_unknown_extensions: false,
        })
        .map_err(|e| CliError::abort(e.to_string()))?;

        let extensions = if let Some(file) = &self.registrations_file {
            let raw = fs::read_to_string(file).map_err(|e| CliError::abort(e.to_string()))?;
            serde_json::from_str(&raw).map_err(|e| CliError::abort(e.to_string()))?
        } else {
            fetch_remote_registrations(&app).await?
        };

        let types = self
            .extension_type
            .as_ref()
            .map(|t| vec![t.clone()])
            .unwrap_or_default();

        let imported = import_extensions(
            &app,
            ImportExtensionsOptions {
                extensions,
                extension_types: types,
                all: self.all,
                overwrite_existing: self.overwrite,
                app_embedded: app.configuration.embedded.unwrap_or(false),
            },
        )
        .map_err(|e| CliError::abort(e.to_string()))?;

        println!("Imported {} extension(s):", imported.len());
        for ext in imported {
            println!("  • \"{}\" at {}", ext.title, ext.directory.display());
        }
        Ok(())
    }
}

async fn fetch_remote_registrations(
    app: &app::LoadedApp,
) -> Result<Vec<ExtensionRegistration>, CliError> {
    let api_key = app
        .configuration
        .client_id
        .clone()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            CliError::abort(
                "No client_id found. Run `shopify app config link` or pass --registrations-file.",
            )
        })?;

    let client = authenticated_developer_platform().await?;
    let identifiers = MinimalAppIdentifiers {
        api_key: api_key.clone(),
        organization_id: String::new(),
        id: String::new(),
    };
    let raw = client
        .app_extension_registrations(&identifiers)
        .await
        .map_err(|e| CliError::abort(e.to_string()))?;

    Ok(parse_extension_registrations(&raw))
}

/// Parse Partners / App Management registration payloads into import shapes.
pub fn parse_extension_registrations(raw: &Value) -> Vec<ExtensionRegistration> {
    let mut out = Vec::new();
    let arrays = [
        raw.pointer("/app/extensionRegistrations"),
        raw.pointer("/app/extension_registrations"),
        raw.pointer("/app/dashboardManagedExtensionRegistrations"),
        raw.pointer("/app/dashboard_managed_extension_registrations"),
        raw.pointer("/extensionRegistrations"),
        raw.pointer("/dashboardManagedExtensionRegistrations"),
        raw.as_array().map(|_| raw),
    ];
    for arr in arrays.into_iter().flatten() {
        if let Some(items) = arr.as_array() {
            for item in items {
                if let Some(reg) = registration_from_value(item) {
                    out.push(reg);
                }
            }
        }
    }
    out
}

fn registration_from_value(item: &Value) -> Option<ExtensionRegistration> {
    let uuid = item
        .get("uuid")
        .or_else(|| item.get("id"))
        .and_then(|v| v.as_str())?
        .to_string();
    let title = item
        .get("title")
        .or_else(|| item.get("name"))
        .and_then(|v| v.as_str())
        .unwrap_or("extension")
        .to_string();
    let type_name = item
        .get("type")
        .or_else(|| item.get("typeName"))
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    let draft_version = version_from(
        item.get("draftVersion")
            .or_else(|| item.get("draft_version")),
    );
    let active_version = version_from(
        item.get("activeVersion")
            .or_else(|| item.get("active_version")),
    );

    Some(ExtensionRegistration {
        uuid,
        title,
        type_name,
        draft_version,
        active_version,
    })
}

fn version_from(value: Option<&Value>) -> Option<ExtensionVersion> {
    let v = value?;
    let config = v.get("config").and_then(|c| {
        if c.is_string() {
            c.as_str().map(|s| s.to_string())
        } else {
            Some(c.to_string())
        }
    });
    let context = v
        .get("context")
        .and_then(|c| c.as_str())
        .map(|s| s.to_string());
    Some(ExtensionVersion { config, context })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_partners_shape() {
        let raw = json!({
            "app": {
                "dashboardManagedExtensionRegistrations": [
                    {
                        "uuid": "u-1",
                        "title": "My Theme",
                        "type": "theme_app_extension",
                        "draftVersion": { "config": "{\"api_version\":\"2024-10\"}" }
                    }
                ]
            }
        });
        let regs = parse_extension_registrations(&raw);
        assert_eq!(regs.len(), 1);
        assert_eq!(regs[0].uuid, "u-1");
        assert_eq!(regs[0].type_name, "theme_app_extension");
    }
}
