//! Resolve a development store for linked-app commands (upstream `store-context.ts`).

use crate::constants::{HIDDEN_CONFIG_DIR, HIDDEN_PROJECT_FILE};
use crate::error::AppError;
use crate::prompts::store::select_store_prompt;
use crate::prompts::Prompter;
use crate::services::config::patch_app_hidden_config_file;
use crate::services::context::LinkedAppContext;
use cli_api::{DeveloperPlatformClient, OrganizationStore};
use serde_json::json;

#[derive(Debug, Clone)]
pub struct StoreContextOptions {
    pub store_fqdn: Option<String>,
    pub force_reselect_store: bool,
    /// Store types to filter by. Defaults to app-development stores.
    pub store_types: Vec<String>,
}

impl Default for StoreContextOptions {
    fn default() -> Self {
        Self {
            store_fqdn: None,
            force_reselect_store: false,
            store_types: vec!["APP_DEVELOPMENT".into()],
        }
    }
}

fn normalize_shop_domain(raw: &str) -> String {
    raw.trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/')
        .trim_end_matches("/admin")
        .to_string()
}

/// Flag → cached `dev_store_url` → prompt `dev_stores_for_org`.
pub async fn store_context(
    ctx: &LinkedAppContext,
    client: &dyn DeveloperPlatformClient,
    options: StoreContextOptions,
    prompter: Option<&dyn Prompter>,
) -> Result<OrganizationStore, AppError> {
    let store_types: Vec<&str> = options.store_types.iter().map(String::as_str).collect();
    let types = if store_types.is_empty() {
        vec!["APP_DEVELOPMENT"]
    } else {
        store_types
    };

    let cached_from_toml = ctx
        .app
        .configuration
        .build
        .as_ref()
        .and_then(|b| b.dev_store_url.clone());
    let cached_from_hidden = ctx.app.hidden_config.dev_store_url.clone();
    let cached = cached_from_toml.clone().or(cached_from_hidden.clone());
    let cached = if options.force_reselect_store {
        None
    } else {
        cached
    };

    let store_fqdn_to_use = options.store_fqdn.clone().or(cached);

    let selected = if let Some(fqdn) = store_fqdn_to_use {
        fetch_store(ctx, client, &fqdn, &types).await?
    } else {
        let listed = client
            .dev_stores_for_org(&ctx.organization.id, None)
            .await
            .map_err(|e| AppError::message(e.to_string()))?;
        select_store(listed.data, client, &ctx.organization.id, prompter).await?
    };

    let mut selected = selected;
    selected.shop_domain = normalize_shop_domain(&selected.shop_domain);

    let cached_url = cached_from_toml.or(cached_from_hidden);
    if cached_url.as_deref() != Some(selected.shop_domain.as_str()) {
        if let Some(ref client_id) = ctx.app.configuration.client_id {
            let path = ctx
                .app
                .directory
                .join(HIDDEN_CONFIG_DIR)
                .join(HIDDEN_PROJECT_FILE);
            patch_app_hidden_config_file(
                &path,
                client_id,
                &json!({ "dev_store_url": selected.shop_domain }),
            )?;
        }
    }

    client
        .ensure_user_access_to_store(&ctx.organization.id, &selected)
        .await
        .map_err(|e| AppError::message(e.to_string()))?;

    Ok(selected)
}

async fn fetch_store(
    ctx: &LinkedAppContext,
    client: &dyn DeveloperPlatformClient,
    fqdn: &str,
    store_types: &[&str],
) -> Result<OrganizationStore, AppError> {
    let normalized = normalize_shop_domain(fqdn);
    client
        .store_by_domain(&ctx.organization.id, &normalized, store_types)
        .await
        .map_err(|e| AppError::message(e.to_string()))?
        .ok_or_else(|| {
            AppError::message(format!(
                "Could not find store `{normalized}`. Pass --store with a development store FQDN."
            ))
        })
}

async fn select_store(
    stores: Vec<OrganizationStore>,
    client: &dyn DeveloperPlatformClient,
    org_id: &str,
    prompter: Option<&dyn Prompter>,
) -> Result<OrganizationStore, AppError> {
    let show_domain = client.client_name() == cli_api::ClientName::AppManagement;
    if let Some(prompter) = prompter {
        select_store_prompt(prompter, &stores, show_domain)?.ok_or_else(|| {
            AppError::message(
                "No development stores found. Pass --store with a development store FQDN.",
            )
        })
    } else if stores.len() == 1 {
        Ok(stores.into_iter().next().unwrap())
    } else if stores.is_empty() {
        let _ = org_id;
        Err(AppError::message(
            "No development stores found. Pass --store with a development store FQDN.",
        ))
    } else {
        Err(AppError::message(
            "Multiple development stores found. Pass --store with a development store FQDN.",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prompts::InjectedPrompter;
    use crate::services::context::{linked_app_context, LinkedAppContextOptions};
    use crate::test_support::{sample_org_app, sample_store, MockClient};
    use std::fs;
    use tempfile::tempdir;

    async fn ctx_in(dir: &std::path::Path, client: &MockClient) -> LinkedAppContext {
        fs::write(
            dir.join("shopify.app.toml"),
            "client_id = \"key-1\"\nname = \"Demo\"\napplication_url = \"https://example.com\"\n[build]\ndev_store_url = \"cached-store.myshopify.com\"\n",
        )
        .unwrap();
        linked_app_context(
            LinkedAppContextOptions {
                directory: dir.to_path_buf(),
                config_name: None,
                client_id: None,
                force_relink: false,
            },
            client,
            None,
        )
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn uses_explicit_store_fqdn() {
        let dir = tempdir().unwrap();
        let mut client = MockClient::with_app(sample_org_app("key-1"));
        client.stores = vec![
            sample_store("1", "cached-store.myshopify.com"),
            sample_store("2", "explicit-store.myshopify.com"),
        ];
        let ctx = ctx_in(dir.path(), &client).await;
        let store = store_context(
            &ctx,
            &client,
            StoreContextOptions {
                store_fqdn: Some("explicit-store.myshopify.com".into()),
                force_reselect_store: false,
                store_types: vec!["APP_DEVELOPMENT".into()],
            },
            None,
        )
        .await
        .unwrap();
        assert_eq!(store.shop_domain, "explicit-store.myshopify.com");
    }

    #[tokio::test]
    async fn uses_cached_dev_store_url() {
        let dir = tempdir().unwrap();
        let mut client = MockClient::with_app(sample_org_app("key-1"));
        client.stores = vec![sample_store("1", "cached-store.myshopify.com")];
        let ctx = ctx_in(dir.path(), &client).await;
        let store = store_context(&ctx, &client, StoreContextOptions::default(), None)
            .await
            .unwrap();
        assert_eq!(store.shop_domain, "cached-store.myshopify.com");
    }

    #[tokio::test]
    async fn force_reselect_ignores_cache() {
        let dir = tempdir().unwrap();
        let mut client = MockClient::with_app(sample_org_app("key-1"));
        client.stores = vec![
            sample_store("1", "cached-store.myshopify.com"),
            sample_store("2", "other.myshopify.com"),
        ];
        let ctx = ctx_in(dir.path(), &client).await;
        let p = InjectedPrompter::new();
        p.push_select("2");
        let store = store_context(
            &ctx,
            &client,
            StoreContextOptions {
                store_fqdn: None,
                force_reselect_store: true,
                store_types: vec!["APP_DEVELOPMENT".into()],
            },
            Some(&p),
        )
        .await
        .unwrap();
        assert_eq!(store.shop_id, "2");
    }

    #[tokio::test]
    async fn missing_store_errors() {
        let dir = tempdir().unwrap();
        let client = MockClient::with_app(sample_org_app("key-1"));
        let ctx = ctx_in(dir.path(), &client).await;
        let err = store_context(
            &ctx,
            &client,
            StoreContextOptions {
                store_fqdn: Some("missing.myshopify.com".into()),
                ..Default::default()
            },
            None,
        )
        .await
        .unwrap_err();
        assert!(err.to_string().contains("Could not find store"));
    }

    #[tokio::test]
    async fn single_store_auto_selected() {
        let dir = tempdir().unwrap();
        let mut client = MockClient::with_app(sample_org_app("key-1"));
        client.stores = vec![sample_store("9", "only.myshopify.com")];
        fs::write(
            dir.path().join("shopify.app.toml"),
            "client_id = \"key-1\"\nname = \"Demo\"\napplication_url = \"https://example.com\"\n",
        )
        .unwrap();
        let ctx = linked_app_context(
            LinkedAppContextOptions {
                directory: dir.path().to_path_buf(),
                config_name: None,
                client_id: None,
                force_relink: false,
            },
            &client,
            None,
        )
        .await
        .unwrap();
        let store = store_context(&ctx, &client, StoreContextOptions::default(), None)
            .await
            .unwrap();
        assert_eq!(store.shop_domain, "only.myshopify.com");
    }

    #[tokio::test]
    async fn multiple_stores_require_flag_or_prompt() {
        let dir = tempdir().unwrap();
        let mut client = MockClient::with_app(sample_org_app("key-1"));
        client.stores = vec![
            sample_store("1", "a.myshopify.com"),
            sample_store("2", "b.myshopify.com"),
        ];
        fs::write(
            dir.path().join("shopify.app.toml"),
            "client_id = \"key-1\"\nname = \"Demo\"\napplication_url = \"https://example.com\"\n",
        )
        .unwrap();
        let ctx = linked_app_context(
            LinkedAppContextOptions {
                directory: dir.path().to_path_buf(),
                config_name: None,
                client_id: None,
                force_relink: false,
            },
            &client,
            None,
        )
        .await
        .unwrap();
        let err = store_context(&ctx, &client, StoreContextOptions::default(), None)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("Multiple development stores"));
    }

    #[tokio::test]
    async fn normalizes_https_prefix() {
        let dir = tempdir().unwrap();
        let mut client = MockClient::with_app(sample_org_app("key-1"));
        client.stores = vec![sample_store("1", "cached-store.myshopify.com")];
        let ctx = ctx_in(dir.path(), &client).await;
        let store = store_context(
            &ctx,
            &client,
            StoreContextOptions {
                store_fqdn: Some("https://cached-store.myshopify.com/admin".into()),
                ..Default::default()
            },
            None,
        )
        .await
        .unwrap();
        assert_eq!(store.shop_domain, "cached-store.myshopify.com");
    }

    #[tokio::test]
    async fn empty_org_errors_without_flag() {
        let dir = tempdir().unwrap();
        let client = MockClient::with_app(sample_org_app("key-1"));
        fs::write(
            dir.path().join("shopify.app.toml"),
            "client_id = \"key-1\"\nname = \"Demo\"\napplication_url = \"https://example.com\"\n",
        )
        .unwrap();
        let ctx = linked_app_context(
            LinkedAppContextOptions {
                directory: dir.path().to_path_buf(),
                config_name: None,
                client_id: None,
                force_relink: false,
            },
            &client,
            None,
        )
        .await
        .unwrap();
        let err = store_context(&ctx, &client, StoreContextOptions::default(), None)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("No development stores"));
    }
}
