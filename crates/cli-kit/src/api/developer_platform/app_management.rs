use async_trait::async_trait;
use cli_api::types::{
    filter_disabled_flags, AccountInfo, ApiSecretKey, AppLogsFetchResult,
    AppLogsSubscribeVariables, AppModuleVersion, AppVersion, AppVersionIdentifiers,
    AppVersionWithContext, AssetUrlSchema, BundleFormat, ClientName, CreateAppOptions,
    ExtensionTemplate, ExtensionTemplatesResult, MinimalAppIdentifiers, MinimalOrganizationApp,
    Organization, OrganizationApp, OrganizationSource, OrganizationStore, Paginateable,
    RemoteSpecification, UserError,
};
use cli_api::{CliApiError, DeveloperPlatformClient};
use serde_json::Value;
use std::collections::HashMap;

use crate::api::app_management::{
    AppManagementClient, OrganizationApp as KitOrganizationApp, Specification,
};
use crate::api::business_platform::{
    id_from_encoded_gid, number_from_gid, AccessibleShopsPage, BusinessPlatformClient,
    BusinessPlatformShop,
};

/// App Management / Developer Dashboard implementation of [`DeveloperPlatformClient`].
pub struct AppManagementPlatformClient {
    inner: AppManagementClient,
    business_platform: Option<BusinessPlatformClient>,
}

impl AppManagementPlatformClient {
    pub fn new(inner: AppManagementClient) -> Self {
        Self {
            inner,
            business_platform: None,
        }
    }

    pub fn with_business_platform(mut self, client: BusinessPlatformClient) -> Self {
        self.business_platform = Some(client);
        self
    }

    pub fn into_inner(self) -> AppManagementClient {
        self.inner
    }

    pub fn inner(&self) -> &AppManagementClient {
        &self.inner
    }

    fn map_err(e: crate::api::graphql::GraphqlRequestError) -> CliApiError {
        CliApiError::graphql(e.to_string())
    }

    fn map_spec(spec: Specification) -> RemoteSpecification {
        RemoteSpecification {
            identifier: spec.identifier,
            name: spec.name,
            experience: spec.experience.unwrap_or_else(|| "extension".into()),
            options: Some(serde_json::json!({
                "uidStrategy": spec.uid_strategy,
                "features": spec.features,
            })),
            validation_schema: spec.validation_schema.and_then(|v| v.json_schema),
        }
    }

    fn map_org_app(app: KitOrganizationApp) -> OrganizationApp {
        let secrets = app
            .active_root
            .as_ref()
            .and_then(|r| r.client_credentials.as_ref())
            .map(|c| {
                c.secrets
                    .iter()
                    .map(|s| ApiSecretKey {
                        secret: s.key.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let scopes = app
            .active_root
            .as_ref()
            .and_then(|r| r.granted_shopify_approval_scopes.clone())
            .unwrap_or_default();
        let modules = app
            .active_release
            .as_ref()
            .and_then(|r| r.version.as_ref())
            .and_then(|v| v.app_modules.as_ref());
        let (application_url, redirect_url_whitelist) = urls_from_modules(modules);
        OrganizationApp {
            id: app.id,
            title: app
                .active_release
                .as_ref()
                .and_then(|r| r.version.as_ref())
                .and_then(|v| v.name.clone())
                .unwrap_or_else(|| app.key.clone()),
            api_key: app.key,
            organization_id: app.organization_id,
            api_secret_keys: secrets,
            granted_scopes: scopes,
            application_url,
            redirect_url_whitelist,
            flags: filter_disabled_flags(&[]),
        }
    }

    fn map_shops(page: AccessibleShopsPage) -> Paginateable<Vec<OrganizationStore>> {
        let stores = page
            .stores
            .into_iter()
            .filter_map(|s| map_bp_shop(s, page.provisionable))
            .collect();
        Paginateable {
            data: stores,
            has_more_pages: page.has_more_pages,
        }
    }

    fn bp_client(&self) -> Result<&BusinessPlatformClient, CliApiError> {
        self.business_platform.as_ref().ok_or_else(|| {
            CliApiError::unsupported(ClientName::AppManagement.as_str(), "dev_stores_for_org")
        })
    }
}

fn urls_from_modules(
    modules: Option<&Vec<crate::api::app_management::AppModule>>,
) -> (Option<String>, Vec<String>) {
    let Some(modules) = modules else {
        return (None, vec![]);
    };
    let mut application_url = None;
    let mut redirect_url_whitelist = vec![];
    for m in modules {
        let ident = m
            .specification
            .as_ref()
            .map(|s| {
                s.external_identifier
                    .clone()
                    .unwrap_or_else(|| s.identifier.clone())
            })
            .unwrap_or_default();
        let config: Value = m
            .config
            .as_deref()
            .and_then(|c| serde_json::from_str(c).ok())
            .unwrap_or(Value::Null);
        if ident == "app_home" || ident == "app_home_external" {
            application_url = config
                .get("app_url")
                .or_else(|| config.get("application_url"))
                .and_then(|v| v.as_str())
                .map(str::to_string);
        }
        if ident == "app_access" || ident == "app_access_external" {
            if let Some(arr) = config
                .get("redirect_url_allowlist")
                .or_else(|| config.pointer("/auth/redirect_urls"))
                .and_then(|v| v.as_array())
            {
                redirect_url_whitelist = arr
                    .iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect();
            }
        }
    }
    (application_url, redirect_url_whitelist)
}

fn map_bp_shop(store: BusinessPlatformShop, provisionable: bool) -> Option<OrganizationStore> {
    let shop_domain = store.url.or(store.primary_domain)?;
    let shop_domain = shop_domain
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/')
        .to_string();
    let shop_id = store
        .external_id
        .as_deref()
        .map(id_from_encoded_gid)
        .or(store.id.clone())
        .unwrap_or_default();
    Some(OrganizationStore {
        shop_id,
        shop_domain: shop_domain.clone(),
        shop_name: store.name,
        transfer_disabled: true,
        convertable_to_partner_test: true,
        provisionable,
        link: Some(shop_domain),
        store_type: store.store_type,
    })
}

fn modules_to_version(modules: Vec<crate::api::app_management::AppModule>) -> AppVersion {
    AppVersion {
        app_module_versions: modules
            .into_iter()
            .map(|m| AppModuleVersion {
                registration_id: m.uuid.clone(),
                registration_uuid: Some(m.uuid),
                registration_title: m.handle.unwrap_or_default(),
                config: m.config.and_then(|c| serde_json::from_str(&c).ok()),
                target: m.target,
                module_type: m
                    .specification
                    .as_ref()
                    .map(|s| s.identifier.clone())
                    .unwrap_or_default(),
            })
            .collect(),
    }
}

#[async_trait]
impl DeveloperPlatformClient for AppManagementPlatformClient {
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
        let orgs = self.inner.organizations().await.map_err(Self::map_err)?;
        Ok(orgs
            .into_iter()
            .map(|o| Organization {
                id: o.id,
                business_name: o.name,
                source: OrganizationSource::BusinessPlatform,
            })
            .collect())
    }

    async fn org_from_id(&self, org_id: &str) -> Result<Option<Organization>, CliApiError> {
        Ok(self
            .inner
            .org_from_id(org_id)
            .await
            .map_err(Self::map_err)?
            .map(|o| Organization {
                id: o.id,
                business_name: o.name,
                source: OrganizationSource::BusinessPlatform,
            }))
    }

    async fn org_and_apps(
        &self,
        org_id: &str,
    ) -> Result<Paginateable<(Organization, Vec<MinimalOrganizationApp>)>, CliApiError> {
        let detail = self
            .inner
            .org_from_id(org_id)
            .await
            .map_err(Self::map_err)?
            .ok_or_else(|| CliApiError::message(format!("Organization {org_id} not found")))?;
        let apps = detail
            .apps
            .into_iter()
            .map(|a| MinimalOrganizationApp {
                identifiers: MinimalAppIdentifiers {
                    api_key: a.key.unwrap_or_default(),
                    organization_id: org_id.to_string(),
                    id: a.id,
                },
                title: a.title,
            })
            .collect();
        Ok(Paginateable {
            data: (
                Organization {
                    id: detail.id,
                    business_name: detail.name,
                    source: OrganizationSource::BusinessPlatform,
                },
                apps,
            ),
            has_more_pages: detail.apps_page_info,
        })
    }

    async fn apps_for_org(
        &self,
        org_id: &str,
        _term: Option<&str>,
    ) -> Result<Paginateable<Vec<MinimalOrganizationApp>>, CliApiError> {
        let page = self.org_and_apps(org_id).await?;
        Ok(Paginateable {
            data: page.data.1,
            has_more_pages: page.has_more_pages,
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
        let initial_version = serde_json::json!({ "name": options.name });
        let created = self
            .inner
            .create_app(&org.id, initial_version)
            .await
            .map_err(Self::map_err)?;
        if !created.user_errors.is_empty() {
            let msg = created
                .user_errors
                .into_iter()
                .filter_map(|e| e.message)
                .collect::<Vec<_>>()
                .join("; ");
            return Err(CliApiError::message(msg));
        }
        let app = created
            .app
            .ok_or_else(|| CliApiError::message("create_app returned no app"))?;
        Ok(OrganizationApp {
            id: app.id,
            title: options.name,
            api_key: app.key,
            organization_id: Some(org.id.clone()),
            api_secret_keys: app
                .active_root
                .and_then(|r| r.client_credentials)
                .map(|c| {
                    c.secrets
                        .into_iter()
                        .map(|s| ApiSecretKey { secret: s.key })
                        .collect()
                })
                .unwrap_or_default(),
            granted_scopes: options.scopes_array.unwrap_or_default(),
            application_url: None,
            redirect_url_whitelist: vec![],
            flags: vec![],
        })
    }

    async fn specifications(
        &self,
        app: &MinimalAppIdentifiers,
    ) -> Result<Vec<RemoteSpecification>, CliApiError> {
        let specs = self
            .inner
            .specifications(&app.organization_id)
            .await
            .map_err(Self::map_err)?;
        Ok(specs.into_iter().map(Self::map_spec).collect())
    }

    async fn template_specifications(
        &self,
        _app: &MinimalAppIdentifiers,
    ) -> Result<ExtensionTemplatesResult, CliApiError> {
        match fetch_cdn_templates().await {
            Ok(result) => Ok(result),
            Err(cdn_err) => {
                tracing::debug!("CDN template catalog failed ({cdn_err}); falling back to GraphQL");
                let templates = self
                    .inner
                    .template_specifications()
                    .await
                    .map_err(Self::map_err)?;
                Ok(map_graphql_templates(templates))
            }
        }
    }

    async fn app_extension_registrations(
        &self,
        app: &MinimalAppIdentifiers,
    ) -> Result<Value, CliApiError> {
        let regs = self
            .inner
            .app_extension_registrations(&app.api_key)
            .await
            .map_err(Self::map_err)?;
        serde_json::to_value(regs).map_err(|e| CliApiError::message(e.to_string()))
    }

    async fn active_app_version(
        &self,
        app: &MinimalAppIdentifiers,
    ) -> Result<Option<AppVersion>, CliApiError> {
        let release = self
            .inner
            .active_app_version(&app.id)
            .await
            .map_err(Self::map_err)?;
        Ok(release.and_then(|a| {
            a.active_release
                .and_then(|r| r.version)
                .and_then(|v| v.app_modules)
                .map(modules_to_version)
        }))
    }

    async fn app_versions(&self, app: &OrganizationApp) -> Result<Value, CliApiError> {
        let versions = self
            .inner
            .app_versions(&app.id)
            .await
            .map_err(Self::map_err)?;
        serde_json::to_value(versions).map_err(|e| CliApiError::message(e.to_string()))
    }

    async fn app_version_by_tag(
        &self,
        app: &MinimalOrganizationApp,
        tag: &str,
    ) -> Result<AppVersionWithContext, CliApiError> {
        let versions = self
            .inner
            .app_versions(&app.identifiers.id)
            .await
            .map_err(Self::map_err)?;
        let match_v = versions
            .into_iter()
            .find(|v| v.metadata.as_ref().and_then(|m| m.version_tag.as_deref()) == Some(tag))
            .ok_or_else(|| CliApiError::message(format!("No version with tag {tag}")))?;
        let detail = self
            .inner
            .app_version_by_id(&match_v.id)
            .await
            .map_err(Self::map_err)?
            .ok_or_else(|| CliApiError::message("Version detail not found"))?;
        let modules = detail.app_modules.unwrap_or_default();
        let version = modules_to_version(modules);
        Ok(AppVersionWithContext {
            id: 0,
            uuid: detail.id,
            version_tag: Some(tag.to_string()),
            app_module_versions: version.app_module_versions,
        })
    }

    async fn app_versions_diff(
        &self,
        app: &MinimalOrganizationApp,
        version: &AppVersionIdentifiers,
    ) -> Result<Value, CliApiError> {
        let diff = self
            .inner
            .app_versions_diff(&app.identifiers.api_key, &version.version_id)
            .await
            .map_err(Self::map_err)?;
        serde_json::to_value(diff).map_err(|e| CliApiError::message(e.to_string()))
    }

    async fn generate_signed_upload_url(
        &self,
        app: &MinimalAppIdentifiers,
    ) -> Result<AssetUrlSchema, CliApiError> {
        let ext = match self.bundle_format() {
            BundleFormat::Br => "br",
            BundleFormat::Zip => "zip",
        };
        let url = self
            .inner
            .generate_signed_upload_url(ext, &app.organization_id)
            .await
            .map_err(Self::map_err)?;
        Ok(AssetUrlSchema {
            asset_url: url.source_upload_url,
            user_errors: url
                .user_errors
                .into_iter()
                .map(|e| UserError {
                    field: e.field,
                    message: e.message.unwrap_or_default(),
                })
                .collect(),
        })
    }

    async fn create_extension(
        &self,
        input: &cli_api::types::ExtensionCreateInput,
    ) -> Result<cli_api::types::CreatedExtension, CliApiError> {
        // App Management assigns UUIDs on atomic deploy; mint a local uid for identifiers.
        let uuid = uuid::Uuid::new_v4().to_string();
        Ok(cli_api::types::CreatedExtension {
            id: uuid.clone(),
            uuid,
            type_name: input.type_name.clone(),
            title: input.title.clone(),
        })
    }

    async fn update_extension(
        &self,
        _: &cli_api::types::ExtensionUpdateDraftInput,
    ) -> Result<cli_api::types::ExtensionUpdateDraftResult, CliApiError> {
        // App Management uses Dev Sessions, not Partners draft push.
        Ok(cli_api::types::ExtensionUpdateDraftResult {
            user_errors: vec![],
        })
    }

    async fn deploy(&self, input: Value) -> Result<Value, CliApiError> {
        let app_id = input
            .get("appId")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CliApiError::message("deploy requires appId"))?;
        let version = input
            .get("version")
            .cloned()
            .unwrap_or(Value::Object(Default::default()));
        let metadata = input.get("metadata").cloned();
        let result = self
            .inner
            .deploy(app_id, version, metadata)
            .await
            .map_err(Self::map_err)?;
        serde_json::to_value(result).map_err(|e| CliApiError::message(e.to_string()))
    }

    async fn release(
        &self,
        app: &MinimalOrganizationApp,
        version: &AppVersionIdentifiers,
    ) -> Result<Value, CliApiError> {
        let result = self
            .inner
            .release(&app.identifiers.id, &version.version_id)
            .await
            .map_err(Self::map_err)?;
        serde_json::to_value(result).map_err(|e| CliApiError::message(e.to_string()))
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
        if let Some(bp) = self.business_platform.as_ref() {
            let email = bp
                .user_email("0")
                .await
                .map_err(Self::map_err)
                .ok()
                .flatten();
            return Ok(AccountInfo { email, id: None });
        }
        Err(CliApiError::unsupported(
            ClientName::AppManagement.as_str(),
            "current_account_info",
        ))
    }

    async fn dev_stores_for_org(
        &self,
        org_id: &str,
        search_term: Option<&str>,
    ) -> Result<Paginateable<Vec<OrganizationStore>>, CliApiError> {
        let page = self
            .bp_client()?
            .list_app_dev_stores(&number_from_gid(org_id), search_term)
            .await
            .map_err(Self::map_err)?;
        Ok(Self::map_shops(page))
    }

    async fn store_by_domain(
        &self,
        org_id: &str,
        shop_domain: &str,
        store_types: &[&str],
    ) -> Result<Option<OrganizationStore>, CliApiError> {
        let types = if store_types.is_empty() {
            vec!["APP_DEVELOPMENT"]
        } else {
            store_types.to_vec()
        };
        let page = self
            .bp_client()?
            .fetch_store_by_domain(&number_from_gid(org_id), shop_domain, &types)
            .await
            .map_err(Self::map_err)?;
        Ok(Self::map_shops(page).data.into_iter().next())
    }

    fn to_extension_graphql_type(&self, input: &str) -> String {
        input.to_string()
    }

    async fn app_deep_link(&self, app: &MinimalAppIdentifiers) -> Result<String, CliApiError> {
        Ok(format!(
            "https://dev.shopify.com/dashboard/{}/apps/{}",
            app.organization_id, app.id
        ))
    }

    fn app_logs_poll_base_url(&self, organization_id: &str) -> String {
        crate::api::app_management::app_management_app_logs_url(organization_id, None, None)
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
        organization_id: &str,
        jwt_token: &str,
        cursor: Option<&str>,
        filters: Option<&HashMap<String, String>>,
    ) -> Result<AppLogsFetchResult, CliApiError> {
        self.inner
            .fetch_app_logs(organization_id, jwt_token, cursor, filters.cloned())
            .await
            .map_err(CliApiError::message)
    }
}

const TEMPLATE_JSON_URL: &str = "https://cdn.shopify.com/static/cli/extensions/templates.json";

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct CdnExtensionTemplate {
    identifier: String,
    name: String,
    #[serde(default)]
    group: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(rename = "type", default)]
    type_name: Option<String>,
    #[serde(default)]
    types: Option<Vec<Value>>,
    #[serde(default)]
    support_links: Option<Vec<String>>,
}

async fn fetch_cdn_templates() -> Result<ExtensionTemplatesResult, String> {
    let response = crate::http::fetch(TEMPLATE_JSON_URL, None, None, None)
        .await
        .map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(format!("CDN HTTP {}", response.status()));
    }
    let body = response.text().await.map_err(|e| e.to_string())?;
    let parsed: Vec<CdnExtensionTemplate> =
        serde_json::from_str(&body).map_err(|e| e.to_string())?;
    Ok(map_cdn_templates(parsed))
}

fn map_cdn_templates(parsed: Vec<CdnExtensionTemplate>) -> ExtensionTemplatesResult {
    let mut group_order = Vec::new();
    let templates = parsed
        .into_iter()
        .map(|t| {
            if let Some(group) = t.group.as_deref() {
                if !group.is_empty() && !group_order.iter().any(|g| g == group) {
                    group_order.push(group.to_string());
                }
            }
            let mut types = t.types.unwrap_or_default();
            if types.is_empty() && (t.type_name.is_some() || t.url.is_some()) {
                types.push(serde_json::json!({
                    "type": t.type_name,
                    "url": t.url,
                }));
            }
            let url = t.url.or_else(|| {
                t.support_links
                    .as_ref()
                    .and_then(|links| links.first().cloned())
            });
            ExtensionTemplate {
                identifier: t.identifier,
                name: t.name,
                group: t.group,
                url,
                types,
            }
        })
        .collect();
    ExtensionTemplatesResult {
        templates,
        group_order,
    }
}

fn map_graphql_templates(
    templates: Vec<crate::api::app_management::TemplateSpecification>,
) -> ExtensionTemplatesResult {
    let mut group_order = Vec::new();
    let templates = templates
        .into_iter()
        .map(|t| {
            if let Some(group) = t.group.as_deref() {
                if !group.is_empty() && !group_order.iter().any(|g| g == group) {
                    group_order.push(group.to_string());
                }
            }
            ExtensionTemplate {
                identifier: t.identifier,
                name: t.name,
                group: t.group,
                url: t.support_links.and_then(|links| links.into_iter().next()),
                types: t
                    .types
                    .unwrap_or_default()
                    .into_iter()
                    .map(|ty| serde_json::to_value(ty).unwrap_or(Value::Null))
                    .collect(),
            }
        })
        .collect();
    ExtensionTemplatesResult {
        templates,
        group_order,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_cdn_templates_extracts_group_order_and_types() {
        let result = map_cdn_templates(vec![
            CdnExtensionTemplate {
                identifier: "checkout_ui".into(),
                name: "Checkout UI".into(),
                group: Some("Checkout".into()),
                url: Some("https://github.com/Shopify/checkout-ui".into()),
                type_name: Some("checkout_ui_extension".into()),
                types: None,
                support_links: None,
            },
            CdnExtensionTemplate {
                identifier: "theme".into(),
                name: "Theme".into(),
                group: Some("Online store".into()),
                url: None,
                type_name: None,
                types: Some(vec![serde_json::json!({"type": "theme_app_extension"})]),
                support_links: Some(vec!["https://shopify.dev/docs".into()]),
            },
        ]);
        assert_eq!(result.group_order, vec!["Checkout", "Online store"]);
        assert_eq!(result.templates.len(), 2);
        assert_eq!(
            result.templates[0].url.as_deref(),
            Some("https://github.com/Shopify/checkout-ui")
        );
        assert_eq!(
            result.templates[0].types[0].get("type").and_then(|v| v.as_str()),
            Some("checkout_ui_extension")
        );
    }
}
