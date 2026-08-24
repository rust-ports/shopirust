use async_trait::async_trait;
use cli_api::types::{
    filter_disabled_flags, AccountInfo, ApiSecretKey, AppLogsFetchResult,
    AppLogsSubscribeVariables, AppVersion, AppVersionIdentifiers, AppVersionWithContext,
    AssetUrlSchema, BundleFormat, ClientName, CreateAppOptions, ExtensionTemplatesResult,
    MinimalAppIdentifiers, MinimalOrganizationApp, Organization, OrganizationApp,
    OrganizationSource, OrganizationStore, Paginateable, RemoteSpecification,
};
use cli_api::{CliApiError, DeveloperPlatformClient};
use serde_json::Value;
use std::collections::HashMap;

use crate::api::partners::{OrganizationApp as KitOrganizationApp, PartnersClient};

/// Partners Dashboard implementation of [`DeveloperPlatformClient`].
pub struct PartnersPlatformClient {
    inner: PartnersClient,
}

impl PartnersPlatformClient {
    pub fn new(inner: PartnersClient) -> Self {
        Self { inner }
    }

    pub fn into_inner(self) -> PartnersClient {
        self.inner
    }

    pub fn inner(&self) -> &PartnersClient {
        &self.inner
    }

    fn map_org_app(app: KitOrganizationApp) -> OrganizationApp {
        OrganizationApp {
            id: app.id,
            title: app.title,
            api_key: app.api_key,
            organization_id: app.organization_id,
            api_secret_keys: app
                .api_secret_keys
                .into_iter()
                .map(|k| ApiSecretKey { secret: k.secret })
                .collect(),
            granted_scopes: app.granted_scopes,
            application_url: app.application_url,
            redirect_url_whitelist: app.redirect_url_whitelist,
            flags: filter_disabled_flags(&app.disabled_flags),
        }
    }

    fn map_err(e: crate::api::graphql::GraphqlRequestError) -> CliApiError {
        CliApiError::graphql(e.to_string())
    }
}

#[async_trait]
impl DeveloperPlatformClient for PartnersPlatformClient {
    fn client_name(&self) -> ClientName {
        ClientName::Partners
    }

    fn web_ui_name(&self) -> &'static str {
        "Partner Dashboard"
    }

    fn supports_atomic_deployments(&self) -> bool {
        false
    }

    fn supports_dev_sessions(&self) -> bool {
        false
    }

    fn supports_store_search(&self) -> bool {
        false
    }

    fn organization_source(&self) -> OrganizationSource {
        OrganizationSource::Partners
    }

    fn bundle_format(&self) -> BundleFormat {
        BundleFormat::Zip
    }

    fn supports_dashboard_managed_extensions(&self) -> bool {
        true
    }

    async fn organizations(&self) -> Result<Vec<Organization>, CliApiError> {
        let orgs = self.inner.organizations().await.map_err(Self::map_err)?;
        Ok(orgs
            .into_iter()
            .map(|o| Organization {
                id: o.id,
                business_name: o.business_name,
                source: OrganizationSource::Partners,
            })
            .collect())
    }

    async fn org_from_id(&self, org_id: &str) -> Result<Option<Organization>, CliApiError> {
        Ok(self
            .inner
            .org_from_id_basic(org_id)
            .await
            .map_err(Self::map_err)?
            .map(|o| Organization {
                id: o.id,
                business_name: o.business_name,
                source: OrganizationSource::Partners,
            }))
    }

    async fn org_and_apps(
        &self,
        org_id: &str,
    ) -> Result<Paginateable<(Organization, Vec<MinimalOrganizationApp>)>, CliApiError> {
        let info = self
            .inner
            .org_from_id(org_id, None)
            .await
            .map_err(Self::map_err)?
            .ok_or_else(|| CliApiError::message(format!("Organization {org_id} not found")))?;
        let apps = info
            .apps
            .into_iter()
            .map(|a| MinimalOrganizationApp {
                identifiers: MinimalAppIdentifiers {
                    api_key: a.api_key,
                    organization_id: org_id.to_string(),
                    id: a.id,
                },
                title: a.title,
            })
            .collect();
        Ok(Paginateable {
            data: (
                Organization {
                    id: info.id,
                    business_name: info.business_name,
                    source: OrganizationSource::Partners,
                },
                apps,
            ),
            has_more_pages: info.apps_page_info,
        })
    }

    async fn apps_for_org(
        &self,
        org_id: &str,
        term: Option<&str>,
    ) -> Result<Paginateable<Vec<MinimalOrganizationApp>>, CliApiError> {
        let info = self
            .inner
            .org_from_id(org_id, term)
            .await
            .map_err(Self::map_err)?
            .ok_or_else(|| CliApiError::message(format!("Organization {org_id} not found")))?;
        let apps = info
            .apps
            .into_iter()
            .map(|a| MinimalOrganizationApp {
                identifiers: MinimalAppIdentifiers {
                    api_key: a.api_key,
                    organization_id: org_id.to_string(),
                    id: a.id,
                },
                title: a.title,
            })
            .collect();
        Ok(Paginateable {
            data: apps,
            has_more_pages: info.apps_page_info,
        })
    }

    async fn app_from_identifiers(
        &self,
        api_key: &str,
    ) -> Result<Option<OrganizationApp>, CliApiError> {
        Ok(self
            .inner
            .app_from_id(api_key)
            .await
            .map_err(Self::map_err)?
            .map(Self::map_org_app))
    }

    async fn create_app(
        &self,
        org: &Organization,
        options: CreateAppOptions,
    ) -> Result<OrganizationApp, CliApiError> {
        let org_id: i64 = org
            .id
            .parse()
            .map_err(|_| CliApiError::message(format!("invalid org id {}", org.id)))?;
        let is_launchable = options.is_launchable.unwrap_or(true);
        let (app_url, redirect) = if is_launchable {
            ("https://example.com", "https://example.com/api/auth")
        } else {
            (
                "https://shopify.dev/apps/default-app-home",
                "https://shopify.dev/apps/default-app-home/api/auth",
            )
        };
        let scopes = options.scopes_array.clone().unwrap_or_default();
        let result = self
            .inner
            .create_app(org_id, &options.name, app_url, vec![redirect], &scopes)
            .await
            .map_err(Self::map_err)?;
        if !result.user_errors.is_empty() {
            let msg = result
                .user_errors
                .into_iter()
                .map(|e| e.message)
                .collect::<Vec<_>>()
                .join("; ");
            return Err(CliApiError::message(msg));
        }
        let app = result
            .app
            .ok_or_else(|| CliApiError::message("create_app returned no app"))?;
        Ok(Self::map_org_app(app))
    }

    async fn specifications(
        &self,
        app: &MinimalAppIdentifiers,
    ) -> Result<Vec<RemoteSpecification>, CliApiError> {
        self.inner
            .extension_specifications(&app.api_key)
            .await
            .map_err(Self::map_err)
    }

    async fn template_specifications(
        &self,
        app: &MinimalAppIdentifiers,
    ) -> Result<ExtensionTemplatesResult, CliApiError> {
        self.inner
            .template_specifications(&app.api_key)
            .await
            .map_err(Self::map_err)
    }

    async fn app_extension_registrations(
        &self,
        app: &MinimalAppIdentifiers,
    ) -> Result<Value, CliApiError> {
        let regs = self
            .inner
            .extension_registrations(&app.api_key)
            .await
            .map_err(Self::map_err)?;
        serde_json::to_value(regs).map_err(|e| CliApiError::message(e.to_string()))
    }

    async fn active_app_version(
        &self,
        app: &MinimalAppIdentifiers,
    ) -> Result<Option<AppVersion>, CliApiError> {
        self.inner
            .active_app_version(&app.api_key)
            .await
            .map_err(Self::map_err)
    }

    async fn app_versions(&self, app: &OrganizationApp) -> Result<Value, CliApiError> {
        self.inner
            .app_versions_list(&app.api_key)
            .await
            .map_err(Self::map_err)
    }

    async fn app_version_by_tag(
        &self,
        app: &MinimalOrganizationApp,
        tag: &str,
    ) -> Result<AppVersionWithContext, CliApiError> {
        self.inner
            .app_version_by_tag(&app.identifiers.api_key, tag)
            .await
            .map_err(Self::map_err)
    }

    async fn app_versions_diff(
        &self,
        app: &MinimalOrganizationApp,
        version: &AppVersionIdentifiers,
    ) -> Result<Value, CliApiError> {
        self.inner
            .app_versions_diff(&app.identifiers.api_key, version.app_version_id)
            .await
            .map_err(Self::map_err)
    }

    async fn generate_signed_upload_url(
        &self,
        app: &MinimalAppIdentifiers,
    ) -> Result<AssetUrlSchema, CliApiError> {
        let result = self
            .inner
            .generate_signed_upload_url(&app.api_key, 1)
            .await
            .map_err(Self::map_err)?;
        Ok(AssetUrlSchema {
            asset_url: result.signed_upload_url,
            user_errors: result
                .user_errors
                .into_iter()
                .map(|e| cli_api::types::UserError {
                    field: if e.field.is_empty() {
                        None
                    } else {
                        Some(e.field)
                    },
                    message: e.message,
                })
                .collect(),
        })
    }

    async fn create_extension(
        &self,
        input: &cli_api::types::ExtensionCreateInput,
    ) -> Result<cli_api::types::CreatedExtension, CliApiError> {
        let result = self
            .inner
            .create_extension(input)
            .await
            .map_err(Self::map_err)?;
        if !result.user_errors.is_empty() {
            let msg = result
                .user_errors
                .into_iter()
                .map(|e| e.message)
                .collect::<Vec<_>>()
                .join(", ");
            return Err(CliApiError::message(msg));
        }
        let reg = result
            .extension_registration
            .ok_or_else(|| CliApiError::message("extensionCreate returned no registration"))?;
        Ok(cli_api::types::CreatedExtension {
            id: reg.id,
            uuid: reg.uuid,
            type_name: reg.type_name,
            title: reg.title,
        })
    }

    async fn update_extension(
        &self,
        input: &cli_api::types::ExtensionUpdateDraftInput,
    ) -> Result<cli_api::types::ExtensionUpdateDraftResult, CliApiError> {
        let result = self
            .inner
            .update_extension_draft(input)
            .await
            .map_err(Self::map_err)?;
        Ok(result)
    }

    async fn deploy(&self, input: Value) -> Result<Value, CliApiError> {
        let result = self
            .inner
            .deploy_app_input(input)
            .await
            .map_err(Self::map_err)?;
        serde_json::to_value(result).map_err(|e| CliApiError::message(e.to_string()))
    }

    async fn release(
        &self,
        app: &MinimalOrganizationApp,
        version: &AppVersionIdentifiers,
    ) -> Result<Value, CliApiError> {
        self.inner
            .release_app_version(&app.identifiers.api_key, None, Some(version.app_version_id))
            .await
            .map_err(Self::map_err)
    }

    async fn update_urls(&self, input: Value) -> Result<Value, CliApiError> {
        let api_key = input
            .get("apiKey")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CliApiError::message("update_urls requires apiKey"))?;
        let app_url = input
            .get("appUrl")
            .or_else(|| input.get("applicationUrl"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let redirect_owned: Vec<String> = input
            .get("redirectUrlWhitelist")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        let redirect_refs: Vec<&str> = redirect_owned.iter().map(String::as_str).collect();
        let errors = self
            .inner
            .update_urls(api_key, app_url, redirect_refs)
            .await
            .map_err(Self::map_err)?;
        Ok(serde_json::json!({ "userErrors": errors }))
    }

    async fn current_account_info(&self) -> Result<AccountInfo, CliApiError> {
        let info = self
            .inner
            .current_account_info()
            .await
            .map_err(Self::map_err)?;
        Ok(AccountInfo {
            email: info.email,
            id: info.org_name,
        })
    }

    async fn dev_stores_for_org(
        &self,
        org_id: &str,
        _search_term: Option<&str>,
    ) -> Result<Paginateable<Vec<OrganizationStore>>, CliApiError> {
        let stores = self
            .inner
            .dev_stores_by_org(org_id)
            .await
            .map_err(Self::map_err)?;
        let mapped = stores
            .into_iter()
            .map(|s| OrganizationStore {
                shop_id: s.shop_id,
                shop_domain: s.shop_domain,
                shop_name: s.shop_name,
                transfer_disabled: s.transfer_disabled,
                convertable_to_partner_test: s.convertable_to_partner_test,
                provisionable: false,
                link: Some(s.link),
                store_type: None,
            })
            .collect();
        Ok(Paginateable {
            data: mapped,
            has_more_pages: false,
        })
    }

    fn to_extension_graphql_type(&self, input: &str) -> String {
        input.to_uppercase()
    }

    async fn app_deep_link(&self, app: &MinimalAppIdentifiers) -> Result<String, CliApiError> {
        Ok(format!(
            "https://partners.shopify.com/{}/apps/{}",
            app.organization_id, app.id
        ))
    }

    fn app_logs_poll_base_url(&self, _organization_id: &str) -> String {
        crate::api::partners::PartnersClient::app_logs_poll_base_url()
    }

    async fn subscribe_to_app_logs(
        &self,
        variables: &AppLogsSubscribeVariables,
        _organization_id: &str,
    ) -> Result<String, CliApiError> {
        self.inner
            .subscribe_to_app_logs(&variables.shop_ids, &variables.api_key)
            .await
            .map_err(Self::map_err)
    }

    async fn fetch_app_logs(
        &self,
        _organization_id: &str,
        jwt_token: &str,
        cursor: Option<&str>,
        filters: Option<&HashMap<String, String>>,
    ) -> Result<AppLogsFetchResult, CliApiError> {
        self.inner
            .fetch_app_logs(jwt_token, cursor, filters.cloned())
            .await
            .map_err(CliApiError::message)
    }

    async fn migrate_app_module(
        &self,
        api_key: &str,
        registration_id: &str,
        type_name: &str,
    ) -> Result<bool, CliApiError> {
        self.inner
            .migrate_app_module(api_key, registration_id, type_name)
            .await
            .map_err(Self::map_err)
    }

    async fn migrate_flow_extension(
        &self,
        api_key: &str,
        registration_id: &str,
    ) -> Result<bool, CliApiError> {
        self.inner
            .migrate_flow_extension(api_key, registration_id)
            .await
            .map_err(Self::map_err)
    }

    async fn migrate_to_ui_extension(
        &self,
        api_key: &str,
        registration_id: &str,
    ) -> Result<bool, CliApiError> {
        self.inner
            .migrate_to_ui_extension(api_key, registration_id)
            .await
            .map_err(Self::map_err)
    }

    async fn convert_to_transfer_disabled_store(
        &self,
        organization_id: &str,
        shop_id: &str,
    ) -> Result<bool, CliApiError> {
        let org_num = organization_id.parse::<i64>().unwrap_or(0);
        self.inner
            .convert_dev_to_test_store(org_num, shop_id)
            .await
            .map_err(Self::map_err)
    }

    async fn update_developer_preview(
        &self,
        api_key: &str,
        enabled: bool,
    ) -> Result<bool, CliApiError> {
        self.inner
            .update_developer_preview(api_key, enabled)
            .await
            .map_err(Self::map_err)
    }

    async fn app_preview_mode(&self, api_key: &str) -> Result<Option<bool>, CliApiError> {
        self.inner
            .app_preview_mode(api_key)
            .await
            .map_err(Self::map_err)
    }

    async fn store_by_domain(
        &self,
        org_id: &str,
        shop_domain: &str,
        _store_types: &[&str],
    ) -> Result<Option<OrganizationStore>, CliApiError> {
        let store = self
            .inner
            .find_store_by_domain(org_id, shop_domain)
            .await
            .map_err(Self::map_err)?;
        Ok(store.map(|s| OrganizationStore {
            shop_id: s.shop_id,
            shop_domain: s.shop_domain,
            shop_name: s.shop_name,
            transfer_disabled: s.transfer_disabled,
            convertable_to_partner_test: s.convertable_to_partner_test,
            provisionable: false,
            link: Some(s.link),
            store_type: None,
        }))
    }
}
