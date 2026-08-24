//! `shopify app dev clean` — stop Next-Gen Dev Session preview on a store.

use crate::error::AppError;
use crate::services::context::LinkedAppContext;
use crate::services::dev::processes::DevSessionClient;
use cli_api::{DeveloperPlatformClient, OrganizationStore};

#[derive(Debug, Clone)]
pub struct DevCleanOptions {
    pub app_dev_token: String,
    pub app_dev_graphql_url: String,
}

/// Delete the active dev session and restore the app's active version on the store.
pub async fn dev_clean(
    ctx: &LinkedAppContext,
    client: &dyn DeveloperPlatformClient,
    store: &OrganizationStore,
    options: DevCleanOptions,
) -> Result<(), AppError> {
    if !client.supports_dev_sessions() {
        return Err(AppError::message(
            "Dev preview is not supported for this app. It's valid only for apps created on the Next-Gen Dev Platform.",
        ));
    }

    let session = DevSessionClient::new(
        store.shop_domain.clone(),
        options.app_dev_token,
        options.app_dev_graphql_url,
    );

    session.delete(&ctx.remote_app.id).await?;

    println!("Dev preview stopped.");
    println!(
        "The dev preview has been stopped on {} and the app's active version has been restored.",
        store.shop_domain
    );
    println!("You can start it again with `shopify app dev`.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::loader::LoadedApp;
    use crate::services::context::LinkedAppContext;
    use crate::test_support::{sample_org_app, sample_store, MockClient};
    use cli_api::Organization;
    use std::path::PathBuf;

    fn ctx() -> LinkedAppContext {
        LinkedAppContext {
            app: LoadedApp {
                directory: PathBuf::from("/tmp/app"),
                configuration_path: PathBuf::from("/tmp/app/shopify.app.toml"),
                configuration: Default::default(),
                hidden_config: Default::default(),
                extensions: vec![],
                webs: vec![],
                identifiers: crate::models::identifiers::Identifiers::new(),
                name: "t".into(),
                errors: vec![],
                dev_application_urls: None,
            },
            remote_app: sample_org_app("key"),
            organization: Organization {
                id: "org".into(),
                business_name: "Acme".into(),
                source: cli_api::OrganizationSource::BusinessPlatform,
            },
        }
    }

    #[tokio::test]
    async fn rejects_without_dev_sessions() {
        let client = MockClient {
            atomic: true,
            dev_sessions: Some(false),
            ..Default::default()
        };
        let err = dev_clean(
            &ctx(),
            &client,
            &sample_store("1", "shop.myshopify.com"),
            DevCleanOptions {
                app_dev_token: String::new(),
                app_dev_graphql_url: "https://example/graphql".into(),
            },
        )
        .await
        .unwrap_err();
        assert!(err.to_string().contains("not supported"));
    }
}
