//! Link a local app configuration file to a remote Shopify app
//! (upstream `services/app/config/link.ts`).

use crate::error::AppError;
use crate::local_storage::{set_cached_app_info, CachedAppInfo};
use crate::models::config_file_naming::get_app_configuration_file_name;
use crate::models::loader::{load_app, LoadAppOptions};
use crate::models::AppConfiguration;
use crate::prompts::config::select_config_name;
use crate::prompts::org_app::{prompt_app_name, select_app, select_organization};
use crate::prompts::Prompter;
use crate::services::config::select_app::{
    deep_merge, fetch_app_remote_configuration, local_configuration_specifications,
};
use crate::services::config::use_config::set_current_config_preference;
use crate::services::config::write_app_configuration_file;
use cli_api::{CreateAppOptions, DeveloperPlatformClient, MinimalAppIdentifiers, OrganizationApp};
use serde_json::{Map, Value};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct LinkConfigOptions {
    pub directory: PathBuf,
    pub client_id: Option<String>,
    pub config_name: Option<String>,
    pub app_name: Option<String>,
    pub organization_id: Option<String>,
    pub is_new_app: bool,
}

#[derive(Debug, Clone)]
pub struct LinkConfigResult {
    pub config_file: String,
    pub path: PathBuf,
    pub remote_app: OrganizationApp,
    pub configuration: AppConfiguration,
    pub newly_created: bool,
}

/// Link a local `shopify.app*.toml` to a remote app: select/create, fetch remote config, merge, write.
pub async fn link_config(
    options: LinkConfigOptions,
    client: &dyn DeveloperPlatformClient,
    prompter: Option<&dyn Prompter>,
) -> Result<LinkConfigResult, AppError> {
    let (remote_app, newly_created) =
        select_or_create_remote_app(&options, client, prompter).await?;

    let local = load_local_options(&options, &remote_app.api_key);
    let config_file = resolve_config_file_name(&remote_app, &options, &local, prompter)?;
    let config_path = options.directory.join(&config_file);

    let identifiers = MinimalAppIdentifiers {
        api_key: remote_app.api_key.clone(),
        organization_id: remote_app.organization_id.clone().unwrap_or_default(),
        id: remote_app.id.clone(),
    };
    let specs = local_configuration_specifications();
    let remote_config = fetch_app_remote_configuration(&identifiers, client, &specs, None)
        .await?
        .unwrap_or_else(|| {
            build_app_configuration_from_remote_app_properties(&remote_app, &local.scopes)
        });

    let mut merged = deep_merge(
        local.existing_config.clone(),
        serde_json::json!({ "client_id": remote_app.api_key }),
    );
    merged = deep_merge(merged, remote_config);
    if let Some(obj) = merged.as_object_mut() {
        obj.remove("scopes");
    }

    let build = build_options_for_generated_config_file(
        local.existing_build.as_ref(),
        local.local_app_id_matched_remote,
        newly_created || options.is_new_app,
        client.supports_dev_sessions(),
    );
    if let Some(build) = build {
        if let Some(obj) = merged.as_object_mut() {
            obj.insert("build".into(), build);
        }
    }

    let configuration: AppConfiguration = serde_json::from_value(merged.clone())
        .unwrap_or_else(|_| configuration_from_value(&merged, &remote_app));

    write_app_configuration_file(&config_path, &configuration)?;
    set_current_config_preference(
        &options.directory,
        &config_file,
        configuration.client_id.clone(),
    )?;

    set_cached_app_info(&CachedAppInfo {
        directory: options.directory.display().to_string(),
        config_file: Some(config_file.clone()),
        app_id: Some(remote_app.api_key.clone()),
        title: Some(remote_app.title.clone()),
        org_id: remote_app.organization_id.clone(),
        store_fqdn: None,
        ..Default::default()
    })?;

    Ok(LinkConfigResult {
        config_file,
        path: config_path,
        remote_app,
        configuration,
        newly_created,
    })
}

struct LocalLinkOptions {
    scopes: String,
    local_app_id_matched_remote: bool,
    existing_build: Option<Value>,
    existing_config: Value,
}

fn load_local_options(options: &LinkConfigOptions, remote_api_key: &str) -> LocalLinkOptions {
    match load_app(LoadAppOptions {
        directory: options.directory.clone(),
        config_name: options.config_name.clone(),
        ignore_unknown_extensions: true,
    }) {
        Ok(app) => {
            let matched = app.configuration.client_id.as_deref() == Some(remote_api_key)
                || options.is_new_app;
            let existing =
                serde_json::to_value(&app.configuration).unwrap_or(Value::Object(Map::new()));
            LocalLinkOptions {
                scopes: app.configuration.scopes().join(","),
                local_app_id_matched_remote: matched,
                existing_build: existing.get("build").cloned(),
                existing_config: if matched {
                    existing
                } else {
                    Value::Object(Map::new())
                },
            }
        }
        Err(_) => LocalLinkOptions {
            scopes: String::new(),
            local_app_id_matched_remote: false,
            existing_build: None,
            existing_config: Value::Object(Map::new()),
        },
    }
}

fn resolve_config_file_name(
    remote_app: &OrganizationApp,
    options: &LinkConfigOptions,
    _local: &LocalLinkOptions,
    prompter: Option<&dyn Prompter>,
) -> Result<String, AppError> {
    if let Some(ref name) = options.config_name {
        return Ok(get_app_configuration_file_name(Some(name)));
    }

    // Reuse an existing TOML already linked to this client_id.
    if let Ok(project) = crate::models::project::Project::load(&options.directory) {
        for path in &project.config_files {
            if let Ok(raw) = std::fs::read_to_string(path) {
                if raw.contains(&format!("client_id = \"{}\"", remote_app.api_key)) {
                    if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                        return Ok(name.to_string());
                    }
                }
            }
        }
        if project.config_files.is_empty() {
            return Ok(get_app_configuration_file_name(None));
        }
    } else {
        return Ok(get_app_configuration_file_name(None));
    }

    if let Some(prompter) = prompter {
        select_config_name(prompter, &options.directory, &remote_app.title)
    } else {
        Ok(get_app_configuration_file_name(None))
    }
}

async fn select_or_create_remote_app(
    options: &LinkConfigOptions,
    client: &dyn DeveloperPlatformClient,
    prompter: Option<&dyn Prompter>,
) -> Result<(OrganizationApp, bool), AppError> {
    if let Some(ref api_key) = options.client_id {
        let remote = client
            .app_from_identifiers(api_key)
            .await
            .map_err(|e| AppError::message(e.to_string()))?
            .ok_or_else(|| {
                AppError::message(format!(
                    "Invalid Client ID: {api_key}. You can find the Client ID in the app settings in the Developer Dashboard."
                ))
            })?;
        return Ok((remote, options.is_new_app));
    }

    let orgs = client
        .organizations()
        .await
        .map_err(|e| AppError::message(e.to_string()))?;
    let org = if let Some(ref id) = options.organization_id {
        orgs.into_iter()
            .find(|o| o.id == *id)
            .ok_or_else(|| AppError::message(format!("Organization {id} not found")))?
    } else {
        let p = prompter.ok_or_else(|| {
            AppError::message("No client_id provided. Pass --client-id or run interactively.")
        })?;
        select_organization(p, &orgs)?
    };

    let apps = client
        .apps_for_org(&org.id, None)
        .await
        .map_err(|e| AppError::message(e.to_string()))?
        .data;

    let selected = if let Some(p) = prompter {
        select_app(p, &apps, true)?
    } else {
        apps.first().cloned()
    };

    if let Some(app) = selected {
        let remote = client
            .app_from_identifiers(&app.identifiers.api_key)
            .await
            .map_err(|e| AppError::message(e.to_string()))?
            .ok_or_else(|| AppError::message("Selected app could not be loaded"))?;
        return Ok((remote, false));
    }

    let name = if let Some(ref n) = options.app_name {
        n.clone()
    } else if let Some(p) = prompter {
        prompt_app_name(p, None)?
    } else {
        return Err(AppError::message(
            "App name is required to create a new app. Pass --name.",
        ));
    };

    let created = client
        .create_app(
            &org,
            CreateAppOptions {
                name,
                is_launchable: Some(false),
                scopes_array: Some(vec![]),
                directory: Some(options.directory.display().to_string()),
                is_embedded: Some(false),
            },
        )
        .await
        .map_err(|e| AppError::message(e.to_string()))?;
    Ok((created, true))
}

fn build_options_for_generated_config_file(
    existing_build: Option<&Value>,
    linked_app_and_client_id_from_file_are_in_sync: bool,
    linked_app_was_newly_created: bool,
    default_to_update_urls_on_dev: bool,
) -> Option<Value> {
    let mut build = Map::new();
    if linked_app_was_newly_created {
        build.insert("include_config_on_deploy".into(), Value::Bool(true));
        if default_to_update_urls_on_dev {
            build.insert("automatically_update_urls_on_dev".into(), Value::Bool(true));
        }
    }
    if linked_app_and_client_id_from_file_are_in_sync {
        if let Some(Value::Object(existing)) = existing_build {
            for (k, v) in existing {
                build.insert(k.clone(), v.clone());
            }
        }
    }
    if build.is_empty() {
        None
    } else {
        Some(Value::Object(build))
    }
}

fn build_app_configuration_from_remote_app_properties(
    remote_app: &OrganizationApp,
    locally_provided_scopes: &str,
) -> Value {
    let application_url = remote_app
        .application_url
        .as_deref()
        .unwrap_or("")
        .trim_end_matches('/')
        .to_string();
    let access_scopes = if locally_provided_scopes.is_empty() {
        serde_json::json!({ "use_legacy_install_flow": true })
    } else {
        serde_json::json!({
            "scopes": locally_provided_scopes,
            "use_legacy_install_flow": true,
        })
    };
    serde_json::json!({
        "name": remote_app.title,
        "application_url": application_url,
        "embedded": true,
        "auth": { "redirect_urls": remote_app.redirect_url_whitelist },
        "access_scopes": access_scopes,
        "webhooks": { "api_version": "2023-07" },
        "pos": { "embedded": false },
    })
}

fn configuration_from_value(value: &Value, remote: &OrganizationApp) -> AppConfiguration {
    let mut cfg = AppConfiguration {
        client_id: Some(remote.api_key.clone()),
        name: value
            .get("name")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .or_else(|| Some(remote.title.clone())),
        application_url: value
            .get("application_url")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .or_else(|| remote.application_url.clone()),
        embedded: value.get("embedded").and_then(|v| v.as_bool()),
        ..Default::default()
    };
    if let Some(obj) = value.as_object() {
        for (k, v) in obj {
            if !matches!(
                k.as_str(),
                "client_id" | "name" | "application_url" | "embedded" | "build"
            ) {
                cfg.extra.insert(k.clone(), v.clone());
            }
        }
        if let Some(build) = obj.get("build") {
            cfg.build = serde_json::from_value(build.clone()).ok();
        }
    }
    cfg
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prompts::InjectedPrompter;
    use crate::test_support::{sample_org_app, MockClient};
    use cli_api::{AppModuleVersion, AppVersion};
    use std::fs;
    use tempfile::tempdir;

    fn options(dir: &std::path::Path, client_id: Option<&str>) -> LinkConfigOptions {
        LinkConfigOptions {
            directory: dir.to_path_buf(),
            client_id: client_id.map(str::to_string),
            config_name: None,
            app_name: None,
            organization_id: None,
            is_new_app: false,
        }
    }

    #[tokio::test]
    async fn link_with_client_id_writes_toml() {
        let dir = tempdir().unwrap();
        let client = MockClient::with_app(sample_org_app("client-123"));
        let result = link_config(options(dir.path(), Some("client-123")), &client, None)
            .await
            .unwrap();
        assert_eq!(result.config_file, "shopify.app.toml");
        let content = fs::read_to_string(&result.path).unwrap();
        assert!(content.contains("client_id = \"client-123\""));
        assert!(content.contains("Demo") || content.contains("name"));
    }

    #[tokio::test]
    async fn link_uses_config_name_flag() {
        let dir = tempdir().unwrap();
        let client = MockClient::with_app(sample_org_app("client-123"));
        let mut opts = options(dir.path(), Some("client-123"));
        opts.config_name = Some("staging".into());
        let result = link_config(opts, &client, None).await.unwrap();
        assert_eq!(result.config_file, "shopify.app.staging.toml");
        assert!(result.path.ends_with("shopify.app.staging.toml"));
    }

    #[tokio::test]
    async fn link_invalid_client_id_errors() {
        let dir = tempdir().unwrap();
        let client = MockClient::with_app(sample_org_app("client-123"));
        let err = link_config(options(dir.path(), Some("missing")), &client, None)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("Invalid Client ID"));
    }

    #[tokio::test]
    async fn link_merges_remote_modules() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("shopify.app.toml"),
            "client_id = \"client-123\"\nname = \"Local\"\n[build]\ndev_store_url = \"cached.myshopify.com\"\n",
        )
        .unwrap();
        let mut client = MockClient::with_app(sample_org_app("client-123"));
        client.active_version = Some(AppVersion {
            app_module_versions: vec![AppModuleVersion {
                registration_id: "1".into(),
                registration_uuid: Some("u1".into()),
                registration_title: "branding".into(),
                config: Some(serde_json::json!({"name": "Remote Name"})),
                target: None,
                module_type: "branding".into(),
            }],
        });
        let result = link_config(options(dir.path(), Some("client-123")), &client, None)
            .await
            .unwrap();
        assert_eq!(result.configuration.name.as_deref(), Some("Remote Name"));
        assert_eq!(
            result
                .configuration
                .build
                .as_ref()
                .and_then(|b| b.dev_store_url.as_deref()),
            Some("cached.myshopify.com")
        );
    }

    #[tokio::test]
    async fn link_creates_app_when_prompted() {
        let dir = tempdir().unwrap();
        let mut client = MockClient::with_app(sample_org_app("client-123"));
        client.apps.clear();
        let p = InjectedPrompter::new();
        p.push_text("Brand New");
        let mut opts = options(dir.path(), None);
        opts.organization_id = Some("org-1".into());
        let result = link_config(opts, &client, Some(&p)).await.unwrap();
        assert!(result.newly_created);
        assert_eq!(result.remote_app.api_key, "created-key");
        assert!(result
            .configuration
            .build
            .as_ref()
            .and_then(|b| b.include_config_on_deploy)
            .unwrap_or(false));
    }

    #[tokio::test]
    async fn link_selects_existing_app() {
        let dir = tempdir().unwrap();
        let client = MockClient::with_app(sample_org_app("client-123"));
        let p = InjectedPrompter::new();
        p.push_select("client-123");
        let mut opts = options(dir.path(), None);
        opts.organization_id = Some("org-1".into());
        let result = link_config(opts, &client, Some(&p)).await.unwrap();
        assert!(!result.newly_created);
        assert_eq!(result.remote_app.api_key, "client-123");
    }

    #[tokio::test]
    async fn link_sets_current_config_preference() {
        let dir = tempdir().unwrap();
        let client = MockClient::with_app(sample_org_app("client-123"));
        let mut opts = options(dir.path(), Some("client-123"));
        opts.config_name = Some("prod".into());
        link_config(opts, &client, None).await.unwrap();
        let cached = crate::local_storage::get_cached_app_info(dir.path()).unwrap();
        assert_eq!(cached.config_file.as_deref(), Some("shopify.app.prod.toml"));
        assert_eq!(cached.app_id.as_deref(), Some("client-123"));
    }

    #[tokio::test]
    async fn link_does_not_reuse_local_when_client_id_differs() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("shopify.app.toml"),
            "client_id = \"other-key\"\nname = \"Other\"\n[build]\ndev_store_url = \"other.myshopify.com\"\n",
        )
        .unwrap();
        let client = MockClient::with_app(sample_org_app("client-123"));
        let result = link_config(options(dir.path(), Some("client-123")), &client, None)
            .await
            .unwrap();
        assert!(result
            .configuration
            .build
            .as_ref()
            .and_then(|b| b.dev_store_url.as_ref())
            .is_none());
    }

    #[tokio::test]
    async fn link_selects_org_when_multiple() {
        let dir = tempdir().unwrap();
        let mut client = MockClient::with_app(sample_org_app("client-123"));
        client.organizations.push(cli_api::Organization {
            id: "org-2".into(),
            business_name: "Other Co".into(),
            source: cli_api::OrganizationSource::BusinessPlatform,
        });
        let p = InjectedPrompter::new();
        p.push_select("org-1");
        p.push_select("client-123");
        let result = link_config(options(dir.path(), None), &client, Some(&p))
            .await
            .unwrap();
        assert_eq!(result.remote_app.api_key, "client-123");
        assert!(!result.newly_created);
    }

    #[tokio::test]
    async fn link_falls_back_when_no_remote_modules() {
        let dir = tempdir().unwrap();
        let mut app = sample_org_app("client-123");
        app.application_url = Some("https://fallback.example/".into());
        app.redirect_url_whitelist = vec!["https://fallback.example/cb".into()];
        let client = MockClient::with_app(app);
        let result = link_config(options(dir.path(), Some("client-123")), &client, None)
            .await
            .unwrap();
        assert_eq!(
            result.configuration.application_url.as_deref(),
            Some("https://fallback.example")
        );
        assert!(result.configuration.extra.contains_key("auth"));
    }
}
