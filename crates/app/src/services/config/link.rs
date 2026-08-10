use crate::error::AppError;
use crate::local_storage::{set_cached_app_info, CachedAppInfo};
use crate::models::config_file_naming::get_app_configuration_file_name;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct LinkConfigOptions {
    pub directory: PathBuf,
    pub client_id: String,
    pub config_name: Option<String>,
    pub app_name: Option<String>,
    pub application_url: Option<String>,
    pub scopes: Option<String>,
    pub org_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct LinkConfigResult {
    pub config_file: String,
    pub path: PathBuf,
}

/// Write / update a local `shopify.app*.toml` linked to a remote client_id.
pub fn link_config(options: LinkConfigOptions) -> Result<LinkConfigResult, AppError> {
    if options.client_id.trim().is_empty() {
        return Err(AppError::message("client_id is required to link an app"));
    }

    let config_file = get_app_configuration_file_name(options.config_name.as_deref());
    let path = options.directory.join(&config_file);

    let mut doc = String::new();
    doc.push_str(&format!("client_id = \"{}\"\n", options.client_id));
    if let Some(name) = options.app_name {
        doc.push_str(&format!("name = \"{}\"\n", escape_toml_string(&name)));
    }
    if let Some(url) = options.application_url {
        doc.push_str(&format!("application_url = \"{}\"\n", escape_toml_string(&url)));
    } else {
        doc.push_str("application_url = \"https://example.com\"\n");
    }
    doc.push_str("embedded = true\n");
    doc.push('\n');
    doc.push_str("[access_scopes]\n");
    doc.push_str(&format!(
        "scopes = \"{}\"\n",
        options.scopes.unwrap_or_default()
    ));

    fs::create_dir_all(&options.directory)?;
    fs::write(&path, doc)?;

    set_cached_app_info(&CachedAppInfo {
        directory: options.directory.display().to_string(),
        config_file: Some(config_file.clone()),
        app_id: Some(options.client_id),
        title: None,
        org_id: options.org_id,
        store_fqdn: None,
    })?;

    Ok(LinkConfigResult { config_file, path })
}

fn escape_toml_string(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn link_writes_toml() {
        let dir = tempdir().unwrap();
        let result = link_config(LinkConfigOptions {
            directory: dir.path().to_path_buf(),
            client_id: "client-123".into(),
            config_name: Some("staging".into()),
            app_name: Some("Staging App".into()),
            application_url: None,
            scopes: Some("write_products".into()),
            org_id: Some("org-1".into()),
        })
        .unwrap();
        assert_eq!(result.config_file, "shopify.app.staging.toml");
        let content = fs::read_to_string(result.path).unwrap();
        assert!(content.contains("client_id = \"client-123\""));
        assert!(content.contains("scopes = \"write_products\""));
    }
}
