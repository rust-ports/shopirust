use async_trait::async_trait;
use serde_json::Value;

use crate::error::CliApiError;
use crate::types::{
    AccountInfo, AppLogsFetchResult, AppLogsSubscribeVariables, AppVersion, AppVersionIdentifiers,
    AppVersionWithContext, AssetUrlSchema, BundleFormat, ClientName, CreateAppOptions,
    ExtensionTemplatesResult, MinimalAppIdentifiers, MinimalOrganizationApp, Organization,
    OrganizationApp, OrganizationSource, OrganizationStore, Paginateable, RemoteSpecification,
};
use std::collections::HashMap;

/// Unified interface over Partners and App Management developer-platform APIs.
#[async_trait]
pub trait DeveloperPlatformClient: Send + Sync {
    fn client_name(&self) -> ClientName;
    fn web_ui_name(&self) -> &'static str;
    fn supports_atomic_deployments(&self) -> bool;
    fn supports_dev_sessions(&self) -> bool;
    fn supports_store_search(&self) -> bool;
    fn organization_source(&self) -> OrganizationSource;
    fn bundle_format(&self) -> BundleFormat;
    fn supports_dashboard_managed_extensions(&self) -> bool;

    async fn organizations(&self) -> Result<Vec<Organization>, CliApiError>;
    async fn org_from_id(&self, org_id: &str) -> Result<Option<Organization>, CliApiError>;
    async fn org_and_apps(
        &self,
        org_id: &str,
    ) -> Result<Paginateable<(Organization, Vec<MinimalOrganizationApp>)>, CliApiError>;
    async fn apps_for_org(
        &self,
        org_id: &str,
        term: Option<&str>,
    ) -> Result<Paginateable<Vec<MinimalOrganizationApp>>, CliApiError>;
    async fn app_from_identifiers(
        &self,
        api_key: &str,
    ) -> Result<Option<OrganizationApp>, CliApiError>;
    async fn create_app(
        &self,
        org: &Organization,
        options: CreateAppOptions,
    ) -> Result<OrganizationApp, CliApiError>;
    async fn specifications(
        &self,
        app: &MinimalAppIdentifiers,
    ) -> Result<Vec<RemoteSpecification>, CliApiError>;
    async fn template_specifications(
        &self,
        app: &MinimalAppIdentifiers,
    ) -> Result<ExtensionTemplatesResult, CliApiError>;
    async fn app_extension_registrations(
        &self,
        app: &MinimalAppIdentifiers,
    ) -> Result<Value, CliApiError>;
    async fn active_app_version(
        &self,
        app: &MinimalAppIdentifiers,
    ) -> Result<Option<AppVersion>, CliApiError>;
    async fn app_versions(&self, app: &OrganizationApp) -> Result<Value, CliApiError>;
    async fn app_version_by_tag(
        &self,
        app: &MinimalOrganizationApp,
        tag: &str,
    ) -> Result<AppVersionWithContext, CliApiError>;
    async fn app_versions_diff(
        &self,
        app: &MinimalOrganizationApp,
        version: &AppVersionIdentifiers,
    ) -> Result<Value, CliApiError>;
    async fn generate_signed_upload_url(
        &self,
        app: &MinimalAppIdentifiers,
    ) -> Result<AssetUrlSchema, CliApiError>;
    async fn deploy(&self, input: Value) -> Result<Value, CliApiError>;
    async fn release(
        &self,
        app: &MinimalOrganizationApp,
        version: &AppVersionIdentifiers,
    ) -> Result<Value, CliApiError>;
    async fn update_urls(&self, input: Value) -> Result<Value, CliApiError>;
    async fn current_account_info(&self) -> Result<AccountInfo, CliApiError>;
    async fn dev_stores_for_org(
        &self,
        org_id: &str,
        search_term: Option<&str>,
    ) -> Result<Paginateable<Vec<OrganizationStore>>, CliApiError>;
    fn to_extension_graphql_type(&self, input: &str) -> String;
    async fn app_deep_link(&self, app: &MinimalAppIdentifiers) -> Result<String, CliApiError>;

    /// Base URL for the app-logs poll HTTP endpoint (no query string).
    fn app_logs_poll_base_url(&self, organization_id: &str) -> String;

    /// Subscribe to app logs; returns a JWT used to poll the HTTP endpoint.
    async fn subscribe_to_app_logs(
        &self,
        variables: &AppLogsSubscribeVariables,
        organization_id: &str,
    ) -> Result<String, CliApiError>;

    /// Poll the app-logs HTTP endpoint.
    async fn fetch_app_logs(
        &self,
        organization_id: &str,
        jwt_token: &str,
        cursor: Option<&str>,
        filters: Option<&HashMap<String, String>>,
    ) -> Result<AppLogsFetchResult, CliApiError>;
}

/// Options for selecting which developer-platform client to use.
#[derive(Debug, Clone, Default)]
pub struct SelectDeveloperPlatformClientOptions {
    pub organization: Option<Organization>,
    pub first_party_dev: bool,
    pub block_partners_access: bool,
}

/// Select among already-constructed clients based on organization / defaults.
pub fn select_developer_platform_client(
    options: SelectDeveloperPlatformClientOptions,
    partners: Option<Box<dyn DeveloperPlatformClient>>,
    app_management: Box<dyn DeveloperPlatformClient>,
) -> Box<dyn DeveloperPlatformClient> {
    if let Some(org) = options.organization {
        return match org.source {
            OrganizationSource::BusinessPlatform => app_management,
            OrganizationSource::Partners => {
                if options.block_partners_access {
                    app_management
                } else {
                    partners.unwrap_or(app_management)
                }
            }
        };
    }

    if options.first_party_dev && !options.block_partners_access {
        if let Some(partners) = partners {
            return partners;
        }
    }
    app_management
}

/// Return every available developer-platform client instance.
pub fn all_developer_platform_clients(
    partners: Option<Box<dyn DeveloperPlatformClient>>,
    app_management: Box<dyn DeveloperPlatformClient>,
    block_partners_access: bool,
) -> Vec<Box<dyn DeveloperPlatformClient>> {
    let mut clients = vec![app_management];
    if !block_partners_access {
        if let Some(partners) = partners {
            clients.push(partners);
        }
    }
    clients
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::*;

    struct StubClient {
        name: ClientName,
        source: OrganizationSource,
        atomic: bool,
        bundle: BundleFormat,
    }

    #[async_trait]
    impl DeveloperPlatformClient for StubClient {
        fn client_name(&self) -> ClientName {
            self.name
        }
        fn web_ui_name(&self) -> &'static str {
            "stub"
        }
        fn supports_atomic_deployments(&self) -> bool {
            self.atomic
        }
        fn supports_dev_sessions(&self) -> bool {
            self.atomic
        }
        fn supports_store_search(&self) -> bool {
            self.atomic
        }
        fn organization_source(&self) -> OrganizationSource {
            self.source
        }
        fn bundle_format(&self) -> BundleFormat {
            self.bundle
        }
        fn supports_dashboard_managed_extensions(&self) -> bool {
            !self.atomic
        }

        async fn organizations(&self) -> Result<Vec<Organization>, CliApiError> {
            Ok(vec![])
        }
        async fn org_from_id(&self, _: &str) -> Result<Option<Organization>, CliApiError> {
            Ok(None)
        }
        async fn org_and_apps(
            &self,
            _: &str,
        ) -> Result<Paginateable<(Organization, Vec<MinimalOrganizationApp>)>, CliApiError>
        {
            Err(CliApiError::message("unimplemented"))
        }
        async fn apps_for_org(
            &self,
            _: &str,
            _: Option<&str>,
        ) -> Result<Paginateable<Vec<MinimalOrganizationApp>>, CliApiError> {
            Err(CliApiError::message("unimplemented"))
        }
        async fn app_from_identifiers(
            &self,
            _: &str,
        ) -> Result<Option<OrganizationApp>, CliApiError> {
            Ok(None)
        }
        async fn create_app(
            &self,
            _: &Organization,
            _: CreateAppOptions,
        ) -> Result<OrganizationApp, CliApiError> {
            Err(CliApiError::message("unimplemented"))
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
            Ok(Value::Null)
        }
        async fn app_version_by_tag(
            &self,
            _: &MinimalOrganizationApp,
            _: &str,
        ) -> Result<AppVersionWithContext, CliApiError> {
            Err(CliApiError::message("unimplemented"))
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
            input.to_string()
        }
        async fn app_deep_link(&self, app: &MinimalAppIdentifiers) -> Result<String, CliApiError> {
            Ok(format!("https://example.com/{}", app.id))
        }
        fn app_logs_poll_base_url(&self, organization_id: &str) -> String {
            format!("https://example.com/orgs/{organization_id}/app_logs/poll")
        }
        async fn subscribe_to_app_logs(
            &self,
            _: &AppLogsSubscribeVariables,
            _: &str,
        ) -> Result<String, CliApiError> {
            Ok("stub-jwt".into())
        }
        async fn fetch_app_logs(
            &self,
            _: &str,
            _: &str,
            _: Option<&str>,
            _: Option<&HashMap<String, String>>,
        ) -> Result<AppLogsFetchResult, CliApiError> {
            Ok(AppLogsFetchResult {
                status: 200,
                app_logs: vec![],
                cursor: None,
                errors: vec![],
            })
        }
    }

    fn partners() -> Box<dyn DeveloperPlatformClient> {
        Box::new(StubClient {
            name: ClientName::Partners,
            source: OrganizationSource::Partners,
            atomic: false,
            bundle: BundleFormat::Zip,
        })
    }

    fn app_mgmt() -> Box<dyn DeveloperPlatformClient> {
        Box::new(StubClient {
            name: ClientName::AppManagement,
            source: OrganizationSource::BusinessPlatform,
            atomic: true,
            bundle: BundleFormat::Br,
        })
    }

    #[test]
    fn selects_app_management_by_default() {
        let client = select_developer_platform_client(
            SelectDeveloperPlatformClientOptions::default(),
            Some(partners()),
            app_mgmt(),
        );
        assert_eq!(client.client_name(), ClientName::AppManagement);
        assert!(client.supports_atomic_deployments());
        assert_eq!(client.bundle_format(), BundleFormat::Br);
    }

    #[test]
    fn selects_partners_for_first_party_dev() {
        let client = select_developer_platform_client(
            SelectDeveloperPlatformClientOptions {
                first_party_dev: true,
                ..Default::default()
            },
            Some(partners()),
            app_mgmt(),
        );
        assert_eq!(client.client_name(), ClientName::Partners);
        assert_eq!(client.bundle_format(), BundleFormat::Zip);
    }

    #[test]
    fn selects_by_organization_source() {
        let bp = select_developer_platform_client(
            SelectDeveloperPlatformClientOptions {
                organization: Some(Organization {
                    id: "1".into(),
                    business_name: "BP".into(),
                    source: OrganizationSource::BusinessPlatform,
                }),
                ..Default::default()
            },
            Some(partners()),
            app_mgmt(),
        );
        assert_eq!(bp.client_name(), ClientName::AppManagement);

        let p = select_developer_platform_client(
            SelectDeveloperPlatformClientOptions {
                organization: Some(Organization {
                    id: "2".into(),
                    business_name: "P".into(),
                    source: OrganizationSource::Partners,
                }),
                ..Default::default()
            },
            Some(partners()),
            app_mgmt(),
        );
        assert_eq!(p.client_name(), ClientName::Partners);
    }

    #[test]
    fn all_clients_respects_block_partners() {
        let clients = all_developer_platform_clients(Some(partners()), app_mgmt(), true);
        assert_eq!(clients.len(), 1);
        assert_eq!(clients[0].client_name(), ClientName::AppManagement);
    }
}
