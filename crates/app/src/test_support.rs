//! Shared [`DeveloperPlatformClient`] mock for unit tests.

use async_trait::async_trait;
use cli_api::*;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Default)]
pub struct MockClient {
    pub app: Option<OrganizationApp>,
    pub organization: Option<Organization>,
    pub organizations: Vec<Organization>,
    pub apps: Vec<MinimalOrganizationApp>,
    pub stores: Vec<OrganizationStore>,
    pub created: Mutex<Vec<CreateAppOptions>>,
    pub create_result: Option<OrganizationApp>,
    pub active_version: Option<AppVersion>,
    pub specifications: Vec<RemoteSpecification>,
    pub registrations: Value,
    pub templates: Vec<ExtensionTemplate>,
    /// When false, mimics Partners (`create_extension` instead of atomic deploy).
    pub atomic: bool,
    /// Override `supports_dev_sessions` (defaults to `atomic`).
    pub dev_sessions: Option<bool>,
    pub update_urls_calls: Mutex<Vec<Value>>,
    pub update_urls_user_errors: Vec<String>,
    pub created_extensions: Mutex<Vec<cli_api::ExtensionCreateInput>>,
    pub updated_extensions: Mutex<Vec<cli_api::ExtensionUpdateDraftInput>>,
    pub update_errors: Vec<String>,
    pub signed_upload_url: Option<String>,
    pub deploy_calls: Mutex<Vec<Value>>,
}

impl MockClient {
    pub fn with_app(app: OrganizationApp) -> Self {
        let org = Organization {
            id: app.organization_id.clone().unwrap_or_else(|| "org-1".into()),
            business_name: "Acme".into(),
            source: OrganizationSource::BusinessPlatform,
        };
        Self {
            organization: Some(org.clone()),
            organizations: vec![org],
            apps: vec![MinimalOrganizationApp {
                identifiers: MinimalAppIdentifiers {
                    api_key: app.api_key.clone(),
                    organization_id: app.organization_id.clone().unwrap_or_default(),
                    id: app.id.clone(),
                },
                title: app.title.clone(),
            }],
            app: Some(app),
            atomic: true,
            ..Default::default()
        }
    }
}

pub fn sample_org_app(api_key: &str) -> OrganizationApp {
    OrganizationApp {
        id: "app-1".into(),
        title: "Demo".into(),
        api_key: api_key.into(),
        organization_id: Some("org-1".into()),
        api_secret_keys: vec![],
        granted_scopes: vec!["write_products".into()],
        application_url: Some("https://example.com".into()),
        redirect_url_whitelist: vec!["https://example.com/callback".into()],
        flags: vec![],
    }
}

pub fn sample_store(id: &str, domain: &str) -> OrganizationStore {
    OrganizationStore {
        shop_id: id.into(),
        shop_domain: domain.into(),
        shop_name: domain.split('.').next().unwrap_or(domain).into(),
        transfer_disabled: true,
        convertable_to_partner_test: true,
        provisionable: false,
        link: Some(domain.into()),
        store_type: Some("app_development".into()),
    }
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
        self.atomic
    }
    fn supports_dev_sessions(&self) -> bool {
        self.dev_sessions.unwrap_or(self.atomic)
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
        Ok(self.organizations.clone())
    }
    async fn org_from_id(&self, id: &str) -> Result<Option<Organization>, CliApiError> {
        if let Some(ref org) = self.organization {
            if org.id == id {
                return Ok(Some(org.clone()));
            }
        }
        Ok(self.organizations.iter().find(|o| o.id == id).cloned())
    }
    async fn org_and_apps(
        &self,
        org_id: &str,
    ) -> Result<Paginateable<(Organization, Vec<MinimalOrganizationApp>)>, CliApiError> {
        let org = self
            .org_from_id(org_id)
            .await?
            .unwrap_or(Organization {
                id: org_id.into(),
                business_name: "Unknown".into(),
                source: OrganizationSource::BusinessPlatform,
            });
        Ok(Paginateable {
            data: (org, self.apps.clone()),
            has_more_pages: false,
        })
    }
    async fn apps_for_org(
        &self,
        _: &str,
        _: Option<&str>,
    ) -> Result<Paginateable<Vec<MinimalOrganizationApp>>, CliApiError> {
        Ok(Paginateable {
            data: self.apps.clone(),
            has_more_pages: false,
        })
    }
    async fn app_from_identifiers(
        &self,
        api_key: &str,
    ) -> Result<Option<OrganizationApp>, CliApiError> {
        Ok(self
            .app
            .as_ref()
            .filter(|a| a.api_key == api_key)
            .cloned())
    }
    async fn create_app(
        &self,
        org: &Organization,
        options: CreateAppOptions,
    ) -> Result<OrganizationApp, CliApiError> {
        self.created.lock().unwrap().push(options.clone());
        if let Some(ref created) = self.create_result {
            return Ok(created.clone());
        }
        Ok(OrganizationApp {
            id: "created-app".into(),
            title: options.name,
            api_key: "created-key".into(),
            organization_id: Some(org.id.clone()),
            api_secret_keys: vec![],
            granted_scopes: options.scopes_array.unwrap_or_default(),
            application_url: None,
            redirect_url_whitelist: vec![],
            flags: vec![],
        })
    }
    async fn specifications(
        &self,
        _: &MinimalAppIdentifiers,
    ) -> Result<Vec<RemoteSpecification>, CliApiError> {
        Ok(self.specifications.clone())
    }
    async fn template_specifications(
        &self,
        _: &MinimalAppIdentifiers,
    ) -> Result<ExtensionTemplatesResult, CliApiError> {
        Ok(ExtensionTemplatesResult {
            templates: self.templates.clone(),
        })
    }
    async fn app_extension_registrations(
        &self,
        _: &MinimalAppIdentifiers,
    ) -> Result<Value, CliApiError> {
        Ok(self.registrations.clone())
    }
    async fn active_app_version(
        &self,
        _: &MinimalAppIdentifiers,
    ) -> Result<Option<AppVersion>, CliApiError> {
        Ok(self.active_version.clone())
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
            asset_url: self
                .signed_upload_url
                .clone()
                .or_else(|| Some("https://upload.example/signed".into())),
            user_errors: vec![],
        })
    }
    async fn create_extension(
        &self,
        input: &cli_api::ExtensionCreateInput,
    ) -> Result<cli_api::CreatedExtension, CliApiError> {
        self.created_extensions.lock().unwrap().push(input.clone());
        Ok(cli_api::CreatedExtension {
            id: format!("id:{}", input.handle),
            uuid: format!("created:{}", input.handle),
            type_name: input.type_name.clone(),
            title: input.title.clone(),
        })
    }
    async fn update_extension(
        &self,
        input: &cli_api::ExtensionUpdateDraftInput,
    ) -> Result<cli_api::ExtensionUpdateDraftResult, CliApiError> {
        self.updated_extensions.lock().unwrap().push(input.clone());
        Ok(cli_api::ExtensionUpdateDraftResult {
            user_errors: self
                .update_errors
                .iter()
                .map(|m| cli_api::UserError {
                    field: None,
                    message: m.clone(),
                })
                .collect(),
        })
    }
    async fn deploy(&self, input: Value) -> Result<Value, CliApiError> {
        self.deploy_calls.lock().unwrap().push(input);
        Ok(Value::Null)
    }
    async fn release(
        &self,
        _: &MinimalOrganizationApp,
        _: &AppVersionIdentifiers,
    ) -> Result<Value, CliApiError> {
        Ok(Value::Null)
    }
    async fn update_urls(&self, input: Value) -> Result<Value, CliApiError> {
        self.update_urls_calls.lock().unwrap().push(input);
        if !self.update_urls_user_errors.is_empty() {
            let errors: Vec<Value> = self
                .update_urls_user_errors
                .iter()
                .map(|m| serde_json::json!({ "field": [], "message": m }))
                .collect();
            return Ok(serde_json::json!({ "appUpdate": { "userErrors": errors } }));
        }
        Ok(serde_json::json!({ "appUpdate": { "userErrors": [] } }))
    }
    async fn current_account_info(&self) -> Result<AccountInfo, CliApiError> {
        Ok(AccountInfo {
            email: Some("dev@example.com".into()),
            id: None,
        })
    }
    async fn dev_stores_for_org(
        &self,
        _: &str,
        search_term: Option<&str>,
    ) -> Result<Paginateable<Vec<OrganizationStore>>, CliApiError> {
        let data = if let Some(term) = search_term {
            self.stores
                .iter()
                .filter(|s| s.shop_domain.contains(term) || s.shop_name.contains(term))
                .cloned()
                .collect()
        } else {
            self.stores.clone()
        };
        Ok(Paginateable {
            data,
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
        _: &AppLogsSubscribeVariables,
        _: &str,
    ) -> Result<String, CliApiError> {
        Ok("test-jwt".into())
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
    async fn migrate_app_module(
        &self,
        _: &str,
        _: &str,
        _: &str,
    ) -> Result<bool, CliApiError> {
        Ok(true)
    }
    async fn migrate_flow_extension(&self, _: &str, _: &str) -> Result<bool, CliApiError> {
        Ok(true)
    }
    async fn migrate_to_ui_extension(&self, _: &str, _: &str) -> Result<bool, CliApiError> {
        Ok(true)
    }
    async fn convert_to_transfer_disabled_store(
        &self,
        _: &str,
        _: &str,
    ) -> Result<bool, CliApiError> {
        Ok(true)
    }
}
