use crate::error::AppError;
use crate::models::loader::{load_app, LoadAppOptions};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct PullConfigOptions {
    pub directory: PathBuf,
    pub config_name: Option<String>,
    /// Remote fields to merge into the local TOML (from DeveloperPlatformClient).
    pub remote_name: Option<String>,
    pub remote_application_url: Option<String>,
    pub remote_scopes: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct PullConfigResult {
    pub config_path: PathBuf,
    pub updated: bool,
}

/// Pull remote app configuration into the local TOML file.
///
/// When remote fields are provided (by the command layer after an API fetch),
/// they are merged into the existing local configuration.
pub fn pull_config(options: PullConfigOptions) -> Result<PullConfigResult, AppError> {
    let loaded = load_app(LoadAppOptions {
        directory: options.directory.clone(),
        config_name: options.config_name.clone(),
        ignore_unknown_extensions: false,
    })?;

    let mut updated = false;
    let mut cfg = loaded.configuration.clone();

    if let Some(name) = options.remote_name {
        if cfg.name.as_deref() != Some(name.as_str()) {
            cfg.name = Some(name);
            updated = true;
        }
    }
    if let Some(url) = options.remote_application_url {
        if cfg.application_url.as_deref() != Some(url.as_str()) {
            cfg.application_url = Some(url);
            updated = true;
        }
    }
    if let Some(scopes) = options.remote_scopes {
        let joined = scopes.join(",");
        let current = cfg.scopes().join(",");
        if current != joined {
            cfg.extra.insert(
                "access_scopes".into(),
                serde_json::json!({ "scopes": joined }),
            );
            updated = true;
        }
    }

    if updated {
        let toml_body = configuration_to_toml(&cfg)?;
        fs::write(&loaded.configuration_path, toml_body)?;
    }

    Ok(PullConfigResult {
        config_path: loaded.configuration_path,
        updated,
    })
}

fn configuration_to_toml(cfg: &crate::models::AppConfiguration) -> Result<String, AppError> {
    let mut out = String::new();
    if let Some(ref id) = cfg.client_id {
        out.push_str(&format!("client_id = \"{id}\"\n"));
    }
    if let Some(ref name) = cfg.name {
        out.push_str(&format!("name = \"{}\"\n", name.replace('"', "\\\"")));
    }
    if let Some(ref url) = cfg.application_url {
        out.push_str(&format!("application_url = \"{url}\"\n"));
    }
    if let Some(embedded) = cfg.embedded {
        out.push_str(&format!("embedded = {embedded}\n"));
    }
    let scopes = cfg.scopes();
    out.push('\n');
    out.push_str("[access_scopes]\n");
    out.push_str(&format!("scopes = \"{}\"\n", scopes.join(",")));
    if let Some(ref build) = cfg.build {
        out.push('\n');
        out.push_str("[build]\n");
        if let Some(ref store) = build.dev_store_url {
            out.push_str(&format!("dev_store_url = \"{store}\"\n"));
        }
        if let Some(v) = build.automatically_update_urls_on_dev {
            out.push_str(&format!("automatically_update_urls_on_dev = {v}\n"));
        }
        if let Some(v) = build.include_config_on_deploy {
            out.push_str(&format!("include_config_on_deploy = {v}\n"));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn pull_merges_remote_fields() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("shopify.app.toml"),
            "client_id = \"abc\"\nname = \"Old\"\napplication_url = \"https://old.example\"\n",
        )
        .unwrap();
        let result = pull_config(PullConfigOptions {
            directory: dir.path().to_path_buf(),
            config_name: None,
            remote_name: Some("New".into()),
            remote_application_url: Some("https://new.example".into()),
            remote_scopes: Some(vec!["write_products".into()]),
        })
        .unwrap();
        assert!(result.updated);
        let content = fs::read_to_string(result.config_path).unwrap();
        assert!(content.contains("name = \"New\""));
        assert!(content.contains("https://new.example"));
    }
}
