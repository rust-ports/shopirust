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
