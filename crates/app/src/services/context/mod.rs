pub mod breakdown_extensions;
pub mod id_matching;
pub mod identifiers;

pub use id_matching::{
    automatic_matchmaking, LocalSource, MatchResult, RemoteSource as MatchRemoteSource,
};

use crate::error::AppError;
use crate::models::loader::{load_app, LoadAppOptions, LoadedApp};
use cli_api::{DeveloperPlatformClient, Organization, OrganizationApp, OrganizationSource};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct LinkedAppContextOptions {
    pub directory: PathBuf,
    pub config_name: Option<String>,
    pub client_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct LinkedAppContext {
    pub app: LoadedApp,
    pub remote_app: OrganizationApp,
    pub organization: Organization,
}

/// Load the local app and resolve the linked remote app via the platform client.
pub async fn linked_app_context(
    options: LinkedAppContextOptions,
    client: &dyn DeveloperPlatformClient,
) -> Result<LinkedAppContext, AppError> {
    let app = load_app(LoadAppOptions {
        directory: options.directory,
        config_name: options.config_name,
                ignore_unknown_extensions: false,
        })?;

    let api_key = options
        .client_id
        .or_else(|| app.configuration.client_id.clone())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            AppError::message(
                "No client_id found. Run `shopify app config link` or pass --client-id.",
            )
        })?;

    let remote_app = client
        .app_from_identifiers(&api_key)
        .await
        .map_err(|e| AppError::message(e.to_string()))?
        .ok_or_else(|| AppError::message(format!("Invalid API Key: {api_key}")))?;

    let org_id = remote_app.organization_id.clone().unwrap_or_default();

    let organization = if org_id.is_empty() {
        Organization {
            id: String::new(),
            business_name: "Unknown".into(),
            source: client.organization_source(),
        }
    } else {
        client
            .org_from_id(&org_id)
            .await
            .map_err(|e| AppError::message(e.to_string()))?
            .unwrap_or(Organization {
                id: org_id,
                business_name: "Unknown".into(),
                source: client.organization_source(),
            })
    };

    // Silence unused import warning for OrganizationSource in some builds
    let _ = OrganizationSource::BusinessPlatform;

    Ok(LinkedAppContext {
        app,
        remote_app,
        organization,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use cli_api::*;
    use serde_json::Value;
    use std::fs;
    use tempfile::tempdir;

    struct MockClient {
        app: OrganizationApp,
    }

    #[async_trait]
    impl DeveloperPlatformClient for MockClient {
        fn client_name(&self) -> ClientName {
            ClientName::AppManagement
        }
        fn web_ui_name(&self) -> &'static str {
            "Developer Dashboard"
        }
        fn supports_atomic_deployments(&self) -> bool {
            true
        }
        fn supports_dev_sessions(&self) -> bool {
            true
        }
        fn supports_store_search(&self) -> bool {
            true
        }
        fn organization_source(&self) -> OrganizationSource {
            OrganizationSource::BusinessPlatform
        }
        fn bundle_format(&self) -> BundleFormat {
            BundleFormat::Br
        }
        fn supports_dashboard_managed_extensions(&self) -> bool {
            false
        }
        async fn organizations(&self) -> Result<Vec<Organization>, CliApiError> {
            Ok(vec![])
        }
        async fn org_from_id(&self, id: &str) -> Result<Option<Organization>, CliApiError> {
            Ok(Some(Organization {
                id: id.into(),
                business_name: "Acme".into(),
                source: OrganizationSource::BusinessPlatform,
            }))
        }
        async fn org_and_apps(
            &self,
            _: &str,
        ) -> Result<Paginateable<(Organization, Vec<MinimalOrganizationApp>)>, CliApiError>
        {
            Err(CliApiError::message("n/a"))
        }
        async fn apps_for_org(
            &self,
            _: &str,
            _: Option<&str>,
        ) -> Result<Paginateable<Vec<MinimalOrganizationApp>>, CliApiError> {
            Err(CliApiError::message("n/a"))
        }
        async fn app_from_identifiers(
            &self,
            api_key: &str,
        ) -> Result<Option<OrganizationApp>, CliApiError> {
            if api_key == self.app.api_key {
                Ok(Some(self.app.clone()))
            } else {
                Ok(None)
            }
        }
        async fn create_app(
            &self,
            _: &Organization,
            _: CreateAppOptions,
        ) -> Result<OrganizationApp, CliApiError> {
            Err(CliApiError::message("n/a"))
        }
        async fn specifications(
            &self,
            _: &MinimalAppIdentifiers,
        ) -> Result<Vec<RemoteSpecification>, CliApiError> {
            Ok(vec![])
        }
        async fn template_specifications(
            &self,
            _: &MinimalAppIdentifiers,
        ) -> Result<ExtensionTemplatesResult, CliApiError> {
            Ok(ExtensionTemplatesResult { templates: vec![] })
        }
        async fn app_extension_registrations(
            &self,
            _: &MinimalAppIdentifiers,
        ) -> Result<Value, CliApiError> {
            Ok(Value::Null)
        }
        async fn active_app_version(
            &self,
            _: &MinimalAppIdentifiers,
        ) -> Result<Option<AppVersion>, CliApiError> {
            Ok(None)
        }
        async fn app_versions(&self, _: &OrganizationApp) -> Result<Value, CliApiError> {
            Ok(serde_json::json!([]))
        }
        async fn app_version_by_tag(
            &self,
            _: &MinimalOrganizationApp,
            _: &str,
        ) -> Result<AppVersionWithContext, CliApiError> {
            Err(CliApiError::message("n/a"))
        }
        async fn app_versions_diff(
            &self,
            _: &MinimalOrganizationApp,
            _: &AppVersionIdentifiers,
        ) -> Result<Value, CliApiError> {
            Ok(Value::Null)
        }
        async fn generate_signed_upload_url(
            &self,
            _: &MinimalAppIdentifiers,
        ) -> Result<AssetUrlSchema, CliApiError> {
            Ok(AssetUrlSchema {
                asset_url: None,
                user_errors: vec![],
            })
        }
        async fn deploy(&self, _: Value) -> Result<Value, CliApiError> {
            Ok(Value::Null)
        }
        async fn release(
            &self,
            _: &MinimalOrganizationApp,
            _: &AppVersionIdentifiers,
        ) -> Result<Value, CliApiError> {
            Ok(Value::Null)
        }
        async fn update_urls(&self, _: Value) -> Result<Value, CliApiError> {
            Ok(Value::Null)
        }
        async fn current_account_info(&self) -> Result<AccountInfo, CliApiError> {
            Ok(AccountInfo {
                email: None,
                id: None,
            })
        }
        async fn dev_stores_for_org(
            &self,
            _: &str,
            _: Option<&str>,
        ) -> Result<Paginateable<Vec<OrganizationStore>>, CliApiError> {
            Ok(Paginateable {
                data: vec![],
                has_more_pages: false,
            })
        }
        fn to_extension_graphql_type(&self, input: &str) -> String {
            input.into()
        }
        async fn app_deep_link(&self, app: &MinimalAppIdentifiers) -> Result<String, CliApiError> {
            Ok(format!("https://example.com/{}", app.id))
        }
        fn app_logs_poll_base_url(&self, organization_id: &str) -> String {
            format!("https://example.com/orgs/{organization_id}/app_logs/poll")
        }
        async fn subscribe_to_app_logs(
            &self,
            _: &cli_api::AppLogsSubscribeVariables,
            _: &str,
        ) -> Result<String, CliApiError> {
            Ok("test-jwt".into())
        }
        async fn fetch_app_logs(
            &self,
            _: &str,
            _: &str,
            _: Option<&str>,
            _: Option<&std::collections::HashMap<String, String>>,
        ) -> Result<cli_api::AppLogsFetchResult, CliApiError> {
            Ok(cli_api::AppLogsFetchResult {
                status: 200,
                app_logs: vec![],
                cursor: None,
                errors: vec![],
            })
        }
    }

    #[tokio::test]
    async fn linked_context_resolves_remote_app() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("shopify.app.toml"),
            "client_id = \"key-1\"\nname = \"Demo\"\napplication_url = \"https://example.com\"\n",
        )
        .unwrap();
        let client = MockClient {
            app: OrganizationApp {
                id: "app-1".into(),
                title: "Demo".into(),
                api_key: "key-1".into(),
                organization_id: Some("org-1".into()),
                api_secret_keys: vec![],
                granted_scopes: vec![],
                application_url: None,
                redirect_url_whitelist: vec![],
                flags: vec![],
            },
        };
        let ctx = linked_app_context(
            LinkedAppContextOptions {
                directory: dir.path().to_path_buf(),
                config_name: None,
                client_id: None,
            },
            &client,
        )
        .await
        .unwrap();
        assert_eq!(ctx.remote_app.api_key, "key-1");
        assert_eq!(ctx.organization.business_name, "Acme");
    }
}
