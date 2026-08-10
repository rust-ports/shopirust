//! Adapters implementing [`cli_api::DeveloperPlatformClient`] over cli-kit GraphQL clients.

mod app_management;
mod partners;

pub use app_management::AppManagementPlatformClient;
pub use partners::PartnersPlatformClient;

use cli_api::{
    all_developer_platform_clients, select_developer_platform_client, DeveloperPlatformClient,
    SelectDeveloperPlatformClientOptions,
};

pub use cli_api::{
    all_developer_platform_clients as all_clients, select_developer_platform_client as select_client,
    SelectDeveloperPlatformClientOptions as SelectOptions,
};

/// Convenience: build both platform clients from tokens and select one.
pub fn developer_platform(
    partners_token: Option<String>,
    app_management_token: String,
    options: SelectDeveloperPlatformClientOptions,
) -> Box<dyn DeveloperPlatformClient> {
    let partners = partners_token.map(|tok| {
        Box::new(PartnersPlatformClient::new(
            crate::api::partners::PartnersClient::new_with_token(tok, None),
        )) as Box<dyn DeveloperPlatformClient>
    });
    let app_management = Box::new(AppManagementPlatformClient::new(
        crate::api::app_management::AppManagementClient::new(app_management_token, None),
    )) as Box<dyn DeveloperPlatformClient>;
    select_developer_platform_client(options, partners, app_management)
}

/// List available clients (App Management always; Partners when token present).
pub fn list_developer_platform_clients(
    partners_token: Option<String>,
    app_management_token: String,
    block_partners_access: bool,
) -> Vec<Box<dyn DeveloperPlatformClient>> {
    let partners = partners_token.map(|tok| {
        Box::new(PartnersPlatformClient::new(
            crate::api::partners::PartnersClient::new_with_token(tok, None),
        )) as Box<dyn DeveloperPlatformClient>
    });
    let app_management = Box::new(AppManagementPlatformClient::new(
        crate::api::app_management::AppManagementClient::new(app_management_token, None),
    )) as Box<dyn DeveloperPlatformClient>;
    all_developer_platform_clients(partners, app_management, block_partners_access)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cli_api::{BundleFormat, ClientName, Organization, OrganizationSource};

    #[test]
    fn developer_platform_defaults_to_app_management() {
        let client = developer_platform(Some("p".into()), "am".into(), Default::default());
        assert_eq!(client.client_name(), ClientName::AppManagement);
        assert_eq!(client.bundle_format(), BundleFormat::Br);
        assert!(client.supports_atomic_deployments());
        assert_eq!(client.web_ui_name(), "Developer Dashboard");
    }

    #[test]
    fn developer_platform_first_party_prefers_partners() {
        let client = developer_platform(
            Some("p".into()),
            "am".into(),
            SelectDeveloperPlatformClientOptions {
                first_party_dev: true,
                ..Default::default()
            },
        );
        assert_eq!(client.client_name(), ClientName::Partners);
        assert_eq!(client.bundle_format(), BundleFormat::Zip);
        assert!(!client.supports_atomic_deployments());
    }

    #[test]
    fn developer_platform_uses_org_source() {
        let client = developer_platform(
            Some("p".into()),
            "am".into(),
            SelectDeveloperPlatformClientOptions {
                organization: Some(Organization {
                    id: "1".into(),
                    business_name: "Acme".into(),
                    source: OrganizationSource::Partners,
                }),
                ..Default::default()
            },
        );
        assert_eq!(client.client_name(), ClientName::Partners);
    }

    #[test]
    fn list_clients_can_block_partners() {
        let clients = list_developer_platform_clients(Some("p".into()), "am".into(), true);
        assert_eq!(clients.len(), 1);
        assert_eq!(clients[0].client_name(), ClientName::AppManagement);
    }
}
