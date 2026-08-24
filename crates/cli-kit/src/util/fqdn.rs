use crate::constants::{self, service_environment, ServiceEnvironment};
use std::collections::HashMap;
use std::path::Path;

const INFERENCE_MODE_SENTINEL: &str = "/opt/dev/misc/dev-server-inference-mode";
const NON_SHOP_PREFIXES: &[&str] = &["admin", "app", "dev", "shopify"];

pub fn partners_fqdn(env: Option<&HashMap<String, String>>) -> String {
    match service_environment(env) {
        ServiceEnvironment::Local => dev_server_host("partners"),
        ServiceEnvironment::Production => constants::PARTNERS_FQDN.to_string(),
    }
}

pub fn admin_fqdn(env: Option<&HashMap<String, String>>) -> String {
    match service_environment(env) {
        ServiceEnvironment::Local => dev_server_core_host("admin"),
        ServiceEnvironment::Production => constants::ADMIN_FQDN.to_string(),
    }
}

pub fn app_management_fqdn(env: Option<&HashMap<String, String>>) -> String {
    match service_environment(env) {
        ServiceEnvironment::Local => dev_server_core_host("app"),
        ServiceEnvironment::Production => constants::APP_MANAGEMENT_FQDN.to_string(),
    }
}

pub fn app_dev_fqdn(store_fqdn: &str, env: Option<&HashMap<String, String>>) -> String {
    match service_environment(env) {
        ServiceEnvironment::Local => dev_server_core_host("app"),
        ServiceEnvironment::Production => store_fqdn.to_string(),
    }
}

pub fn developer_dashboard_fqdn(env: Option<&HashMap<String, String>>) -> String {
    match service_environment(env) {
        ServiceEnvironment::Local => dev_server_core_host("dev"),
        ServiceEnvironment::Production => constants::DEVELOPER_DASHBOARD_FQDN.to_string(),
    }
}

pub fn business_platform_fqdn(env: Option<&HashMap<String, String>>) -> String {
    match service_environment(env) {
        ServiceEnvironment::Local => dev_server_host("business-platform"),
        ServiceEnvironment::Production => constants::BUSINESS_PLATFORM_FQDN.to_string(),
    }
}

pub fn identity_fqdn(env: Option<&HashMap<String, String>>) -> String {
    match service_environment(env) {
        ServiceEnvironment::Local => dev_server_host("identity"),
        ServiceEnvironment::Production => constants::IDENTITY_FQDN.to_string(),
    }
}

pub fn normalize_store_fqdn(store: &str, env: Option<&HashMap<String, String>>) -> String {
    let store_fqdn = store
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/')
        .trim_end_matches("/admin");

    if contains_shopify_domain(store_fqdn) {
        return store_fqdn.to_string();
    }

    match service_environment(env) {
        ServiceEnvironment::Local => dev_server_core_host(store_fqdn),
        ServiceEnvironment::Production => format!("{store_fqdn}.myshopify.com"),
    }
}

pub fn store_admin_url(store_fqdn: &str, env: Option<&HashMap<String, String>>) -> String {
    if service_environment(env) == ServiceEnvironment::Local && store_fqdn.ends_with(".my.shop.dev")
    {
        let store_name = store_fqdn.trim_end_matches(".my.shop.dev");
        return format!("admin.shop.dev/store/{store_name}");
    }
    store_fqdn.to_string()
}

fn contains_shopify_domain(store_fqdn: &str) -> bool {
    store_fqdn.ends_with(".myshopify.com")
        || store_fqdn.ends_with("shopify.io")
        || store_fqdn.ends_with(".shop.dev")
}

fn dev_server_host(project_name: &str) -> String {
    if use_edition_2016(project_name) {
        format!("{project_name}.myshopify.io")
    } else {
        host_2024(project_name, project_name)
    }
}

fn dev_server_core_host(prefix: &str) -> String {
    if use_edition_2016("shopify") {
        format!("{prefix}.myshopify.io")
    } else {
        host_2024("shopify", prefix)
    }
}

fn host_2024(project_name: &str, prefix: &str) -> String {
    let prefix = prefix.replace('_', "-");
    if project_name == "shopify" {
        if let Some(shop_name) = prefix.strip_suffix("-dev-api") {
            return format!("{shop_name}.dev-api.shop.dev");
        }
        if !NON_SHOP_PREFIXES.contains(&prefix.as_str()) {
            return format!("{prefix}.my.shop.dev");
        }
    }
    format!("{prefix}.shop.dev")
}

fn use_edition_2016(project_name: &str) -> bool {
    Path::new(INFERENCE_MODE_SENTINEL).exists()
        && !Path::new(&format!(
            "/opt/nginx/etc/manifest/{project_name}/current/edition-2024"
        ))
        .exists()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::EnvVars;

    fn local_env() -> HashMap<String, String> {
        HashMap::from([(EnvVars::SERVICE_ENV.to_string(), "local".to_string())])
    }

    #[test]
    fn production_fqdns_match_upstream() {
        assert_eq!(partners_fqdn(None), "partners.shopify.com");
        assert_eq!(admin_fqdn(None), "admin.shopify.com");
        assert_eq!(app_management_fqdn(None), "app.shopify.com");
        assert_eq!(business_platform_fqdn(None), "destinations.shopifysvc.com");
        assert_eq!(identity_fqdn(None), "accounts.shopify.com");
    }

    #[test]
    fn local_fqdns_use_dev_server_2024_shape_by_default() {
        let env = local_env();
        assert_eq!(partners_fqdn(Some(&env)), "partners.shop.dev");
        assert_eq!(admin_fqdn(Some(&env)), "admin.shop.dev");
        assert_eq!(app_management_fqdn(Some(&env)), "app.shop.dev");
        assert_eq!(
            business_platform_fqdn(Some(&env)),
            "business-platform.shop.dev"
        );
        assert_eq!(identity_fqdn(Some(&env)), "identity.shop.dev");
    }

    #[test]
    fn normalize_store_fqdn_matches_environment() {
        let env = local_env();
        assert_eq!(
            normalize_store_fqdn("https://example.myshopify.com/admin/", None),
            "example.myshopify.com"
        );
        assert_eq!(
            normalize_store_fqdn("example", None),
            "example.myshopify.com"
        );
        assert_eq!(
            normalize_store_fqdn("example", Some(&env)),
            "example.my.shop.dev"
        );
    }

    #[test]
    fn store_admin_url_maps_local_store_url() {
        let env = local_env();
        assert_eq!(
            store_admin_url("example.my.shop.dev", Some(&env)),
            "admin.shop.dev/store/example"
        );
        assert_eq!(
            store_admin_url("example.myshopify.com", None),
            "example.myshopify.com"
        );
    }
}
