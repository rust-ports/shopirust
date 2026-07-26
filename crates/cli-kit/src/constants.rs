/// Registry of all Shopify CLI environment variable names.
///
/// Centralizes env var lookups so they're greppable and typos are impossible.
/// Every constant maps to the upstream `environmentVariables` object.
pub struct EnvVars;
impl EnvVars {
    pub const ALWAYS_LOG_ANALYTICS: &'static str = "SHOPIFY_CLI_ALWAYS_LOG_ANALYTICS";
    pub const ALWAYS_LOG_METRICS: &'static str = "SHOPIFY_CLI_ALWAYS_LOG_METRICS";
    pub const DEVICE_AUTH: &'static str = "SHOPIFY_CLI_DEVICE_AUTH";
    pub const DOCTOR: &'static str = "SHOPIFY_CLI_DOCTOR";
    pub const ENABLE_CLI_REDIRECT: &'static str = "SHOPIFY_CLI_ENABLE_CLI_REDIRECT";
    pub const ENV: &'static str = "SHOPIFY_CLI_ENV";
    pub const NO_ANALYTICS: &'static str = "SHOPIFY_CLI_NO_ANALYTICS";
    pub const OPT_OUT_INSTRUMENTATION: &'static str = "OPT_OUT_INSTRUMENTATION";
    pub const APP_AUTOMATION_TOKEN: &'static str = "SHOPIFY_APP_AUTOMATION_TOKEN";
    pub const PARTNERS_TOKEN: &'static str = "SHOPIFY_CLI_PARTNERS_TOKEN";
    pub const RUN_AS_USER: &'static str = "SHOPIFY_RUN_AS_USER";
    pub const SERVICE_ENV: &'static str = "SHOPIFY_SERVICE_ENV";
    pub const SKIP_CLI_REDIRECT: &'static str = "SHOPIFY_CLI_SKIP_CLI_REDIRECT";
    pub const SPIN_INSTANCE: &'static str = "SPIN_INSTANCE";
    pub const THEME_TOKEN: &'static str = "SHOPIFY_CLI_THEME_TOKEN";
    pub const UNIT_TEST: &'static str = "SHOPIFY_UNIT_TEST";
    pub const VERBOSE: &'static str = "SHOPIFY_FLAG_VERBOSE";
    pub const CODESPACES: &'static str = "CODESPACES";
    pub const CODESPACE_NAME: &'static str = "CODESPACE_NAME";
    pub const CODESPACE_PORT_FORWARDING_DOMAIN: &'static str =
        "GITHUB_CODESPACES_PORT_FORWARDING_DOMAIN";
    pub const GITPOD: &'static str = "GITPOD_WORKSPACE_URL";
    pub const CLOUD_SHELL: &'static str = "CLOUD_SHELL";
    pub const SPIN_APP_PORT: &'static str = "SERVER_PORT";
    pub const SPIN_APP_HOST: &'static str = "SPIN_APP_HOST";
    pub const ORGANIZATION: &'static str = "SHOPIFY_CLI_ORGANIZATION";
    pub const IDENTITY_TOKEN: &'static str = "SHOPIFY_CLI_IDENTITY_TOKEN";
    pub const REFRESH_TOKEN: &'static str = "SHOPIFY_CLI_REFRESH_TOKEN";
    pub const OTEL_URL: &'static str = "SHOPIFY_CLI_OTEL_EXPORTER_OTLP_ENDPOINT";
    pub const THEME_KIT_ACCESS_DOMAIN: &'static str = "SHOPIFY_CLI_THEME_KIT_ACCESS_DOMAIN";
    pub const JSON: &'static str = "SHOPIFY_FLAG_JSON";
    pub const SKIP_NETWORK_RETRY: &'static str = "SHOPIFY_CLI_SKIP_NETWORK_LEVEL_RETRY";
    pub const MAX_REQUEST_TIME_MS: &'static str = "SHOPIFY_CLI_MAX_REQUEST_TIME_FOR_NETWORK_CALLS";
    pub const DISABLE_IMPORT_SCANNING: &'static str = "SHOPIFY_CLI_DISABLE_IMPORT_SCANNING";
    pub const RUN_AS_SHOPIFY: &'static str = "SHOPIFY_RUN_AS_SHOPIFY";
    pub const FIRST_PARTY: &'static str = "SHOPIFY_CLI_1P";
    pub const ENABLE_DOCTOR_RELEASE: &'static str = "SHOPIFY_CLI_ENABLE_DOCTOR_RELEASE";
    pub const NEVER_USE_PARTNERS_API: &'static str = "SHOPIFY_CLI_NEVER_USE_PARTNERS_API";
    pub const BACKEND_PORT: &'static str = "BACKEND_PORT";
}

/// Read an env var, falling back to process env, returning empty string on miss.
pub fn get_env(env: Option<&std::collections::HashMap<String, String>>, key: &str) -> String {
    env.and_then(|e| e.get(key).cloned())
        .or_else(|| std::env::var(key).ok())
        .unwrap_or_default()
}

/// Read an env var, falling back to process env, returning `None` on miss.
pub fn get_env_opt(
    env: Option<&std::collections::HashMap<String, String>>,
    key: &str,
) -> Option<String> {
    env.and_then(|e| e.get(key).cloned())
        .or_else(|| std::env::var(key).ok())
}

/// Check whether an env var is set to a truthy value (1/true/TRUE/yes/YES).
pub fn is_env_truthy(env: Option<&std::collections::HashMap<String, String>>, key: &str) -> bool {
    let val = get_env(env, key);
    matches!(val.as_str(), "1" | "true" | "TRUE" | "yes" | "YES")
}

/// Shopify service deployment environment.
///
/// - `Local` → dev server FQDNs (not yet supported, resolves to production)
/// - `Production` → production FQDNs
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ServiceEnvironment {
    Local,
    Production,
}

/// Determine the current service environment from `SHOPIFY_SERVICE_ENV`.
pub fn service_environment(
    env: Option<&std::collections::HashMap<String, String>>,
) -> ServiceEnvironment {
    let value = get_env(env, EnvVars::SERVICE_ENV);
    if value == "local" {
        ServiceEnvironment::Local
    } else {
        ServiceEnvironment::Production
    }
}

/// Shortcut — is the service running in local dev mode?
pub fn is_local_environment(env: Option<&std::collections::HashMap<String, String>>) -> bool {
    matches!(service_environment(env), ServiceEnvironment::Local)
}

/// Production Identity FQDN: `accounts.shopify.com`
pub const IDENTITY_FQDN: &str = "accounts.shopify.com";

/// Production Partners API FQDN: `partners.shopify.com`
pub const PARTNERS_FQDN: &str = "partners.shopify.com";

/// Production App Management FQDN: `app.shopify.com`
pub const APP_MANAGEMENT_FQDN: &str = "app.shopify.com";

/// Production Business Platform FQDN: `destinations.shopifysvc.com`
pub const BUSINESS_PLATFORM_FQDN: &str = "destinations.shopifysvc.com";

/// Production Admin FQDN: `admin.shopify.com`
pub const ADMIN_FQDN: &str = "admin.shopify.com";

/// Production Developer Dashboard FQDN: `dev.shopify.com`
pub const DEVELOPER_DASHBOARD_FQDN: &str = "dev.shopify.com";

/// Default theme kit access proxy domain.
pub const DEFAULT_THEME_KIT_ACCESS_DOMAIN: &str = "theme-kit-access.shopifyapps.com";

/// Bugsnag API key for crash/error reporting.
pub const BUGSNAG_API_KEY: &str = "9e1e6889176fd0c795d5c659225e0fae";

/// Maximum analytics events per report interval.
pub const REPORTING_RATE_LIMIT_MAX: u32 = 300;

/// Report interval in days for analytics rate limiting.
pub const REPORTING_RATE_LIMIT_TIMEOUT_DAYS: u64 = 1;

/// How many minutes before a session identity token actually expires we treat it as expired.
pub const SESSION_EXPIRATION_MARGIN_MINUTES: u64 = 4;

/// OAuth client ID for Shopify Identity device-authorization flow.
pub fn identity_client_id() -> &'static str {
    "fbdb2649-e327-4907-8f67-908d24cfd7e3"
}

/// Application ID per API surface, used when exchanging identity tokens
/// for API-specific session tokens.
///
/// ## Panics
/// Panics on unknown API names.
pub fn identity_application_id(api: &str) -> &'static str {
    match api {
        "admin" => "7ee65a63608843c577db8b23c4d7316ea0a01bd2f7594f8a9c06ea668c1b775c",
        "partners" => "271e16d403dfa18082ffb3d197bd2b5f4479c3fc32736d69296829cbb28d41a6",
        "storefront-renderer" => "ee139b3d-5861-4d45-b387-1bc3ada7811c",
        "business-platform" => "32ff8ee5-82b8-4d93-9f8a-c6997cefb7dc",
        "app-management" => "7ee65a63608843c577db8b23c4d7316ea0a01bd2f7594f8a9c06ea668c1b775c",
        _ => panic!("Unknown API: {api}"),
    }
}

/// Resolve an FQDN, respecting the service environment.
///
/// Currently always returns `production_fqdn` regardless of environment;
/// local-dev overrides will be wired once the dev server integration exists.
fn resolve_fqdn(
    production_fqdn: &str,
    env: Option<&std::collections::HashMap<String, String>>,
) -> String {
    match service_environment(env) {
        ServiceEnvironment::Local => match production_fqdn {
            PARTNERS_FQDN => crate::util::fqdn::partners_fqdn(env),
            ADMIN_FQDN => crate::util::fqdn::admin_fqdn(env),
            APP_MANAGEMENT_FQDN => crate::util::fqdn::app_management_fqdn(env),
            BUSINESS_PLATFORM_FQDN => crate::util::fqdn::business_platform_fqdn(env),
            IDENTITY_FQDN => crate::util::fqdn::identity_fqdn(env),
            DEVELOPER_DASHBOARD_FQDN => crate::util::fqdn::developer_dashboard_fqdn(env),
            _ => production_fqdn.to_string(),
        },
        ServiceEnvironment::Production => production_fqdn.to_string(),
    }
}

/// Resolved Partners API FQDN.
pub fn partners_fqdn(env: Option<&std::collections::HashMap<String, String>>) -> String {
    resolve_fqdn(PARTNERS_FQDN, env)
}

/// Resolved Admin API FQDN.
pub fn admin_fqdn(env: Option<&std::collections::HashMap<String, String>>) -> String {
    resolve_fqdn(ADMIN_FQDN, env)
}

/// Resolved App Management FQDN.
pub fn app_management_fqdn(env: Option<&std::collections::HashMap<String, String>>) -> String {
    resolve_fqdn(APP_MANAGEMENT_FQDN, env)
}

/// Resolved Business Platform FQDN.
pub fn business_platform_fqdn(env: Option<&std::collections::HashMap<String, String>>) -> String {
    resolve_fqdn(BUSINESS_PLATFORM_FQDN, env)
}

/// Resolved Identity FQDN.
pub fn identity_fqdn(env: Option<&std::collections::HashMap<String, String>>) -> String {
    resolve_fqdn(IDENTITY_FQDN, env)
}

/// Resolved Developer Dashboard FQDN.
pub fn developer_dashboard_fqdn(env: Option<&std::collections::HashMap<String, String>>) -> String {
    resolve_fqdn(DEVELOPER_DASHBOARD_FQDN, env)
}

/// Resolve the App Dev FQDN, which maps to the store's own domain in production
/// and to the App Management FQDN in local dev.
pub fn app_dev_fqdn(
    store_fqdn: &str,
    env: Option<&std::collections::HashMap<String, String>>,
) -> String {
    crate::util::fqdn::app_dev_fqdn(store_fqdn, env)
}

/// Resolve the theme kit access domain, optionally overridden via env var.
pub fn theme_kit_access_domain(env: Option<&std::collections::HashMap<String, String>>) -> String {
    get_env_opt(env, EnvVars::THEME_KIT_ACCESS_DOMAIN)
        .unwrap_or_else(|| DEFAULT_THEME_KIT_ACCESS_DOMAIN.to_string())
}

/// Normalise a raw store value to a full `*.myshopify.com` FQDN.
///
/// Strips `https://` prefix, `/admin` suffix, and trailing slashes.
/// If the cleaned string doesn't end with a recognised Shopify domain it
/// appends `.myshopify.com`.
pub fn normalize_store_fqdn(store: &str) -> String {
    crate::util::fqdn::normalize_store_fqdn(store, None)
}

/// Platform-appropriate directory for persistent cache data.
///
/// Respects `$XDG_CACHE_HOME/shopify-cli`, falling back to
/// `$HOME/.cache/shopify-cli` (Linux) or `/tmp/shopify-cli/cache`.
pub fn cache_path() -> String {
    if let Ok(xdg) = std::env::var("XDG_CACHE_HOME") {
        return format!("{}/shopify-cli", xdg.trim_end_matches('/'));
    }
    if let Some(home) = dirs::home_dir() {
        format!("{}/.cache/shopify-cli", home.to_string_lossy())
    } else {
        "/tmp/shopify-cli/cache".to_string()
    }
}

/// Vendor binaries directory under the cache tree.
pub fn vendor_binaries_path() -> String {
    format!("{}/vendor/binaries", cache_path())
}

/// Platform-appropriate directory for log files.
///
/// Respects `$XDG_DATA_HOME/shopify-cli/logs`, falling back to
/// `$HOME/.local/share/shopify-cli/logs` (Linux) or `/tmp/shopify-cli/logs`.
pub fn logs_path() -> String {
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        return format!("{}/shopify-cli/logs", xdg.trim_end_matches('/'));
    }
    if let Some(home) = dirs::home_dir() {
        format!("{}/.local/share/shopify-cli/logs", home.to_string_lossy())
    } else {
        "/tmp/shopify-cli/logs".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_environment_default_production() {
        assert!(matches!(
            service_environment(None),
            ServiceEnvironment::Production
        ));
    }

    #[test]
    fn test_service_environment_local() {
        let mut env = std::collections::HashMap::new();
        env.insert(EnvVars::SERVICE_ENV.to_string(), "local".to_string());
        assert!(matches!(
            service_environment(Some(&env)),
            ServiceEnvironment::Local
        ));
    }

    #[test]
    fn test_is_local_environment() {
        let mut env = std::collections::HashMap::new();
        env.insert(EnvVars::SERVICE_ENV.to_string(), "local".to_string());
        assert!(is_local_environment(Some(&env)));
        assert!(!is_local_environment(None));
    }

    #[test]
    fn test_fqdn_resolvers_default() {
        assert_eq!(partners_fqdn(None), PARTNERS_FQDN);
        assert_eq!(admin_fqdn(None), ADMIN_FQDN);
        assert_eq!(app_management_fqdn(None), APP_MANAGEMENT_FQDN);
        assert_eq!(business_platform_fqdn(None), BUSINESS_PLATFORM_FQDN);
        assert_eq!(identity_fqdn(None), IDENTITY_FQDN);
        assert_eq!(developer_dashboard_fqdn(None), DEVELOPER_DASHBOARD_FQDN);
    }

    #[test]
    fn test_app_dev_fqdn_production() {
        assert_eq!(
            app_dev_fqdn("test-store.myshopify.com", None),
            "test-store.myshopify.com"
        );
    }

    #[test]
    fn test_normalize_store_fqdn_adds_domain() {
        assert_eq!(
            normalize_store_fqdn("test-store"),
            "test-store.myshopify.com"
        );
    }

    #[test]
    fn test_normalize_store_fqdn_preserves_existing() {
        assert_eq!(
            normalize_store_fqdn("test-store.myshopify.com"),
            "test-store.myshopify.com"
        );
    }

    #[test]
    fn test_normalize_store_fqdn_removes_https() {
        assert_eq!(
            normalize_store_fqdn("https://test-store.myshopify.com"),
            "test-store.myshopify.com"
        );
    }

    #[test]
    fn test_normalize_store_fqdn_removes_admin() {
        assert_eq!(
            normalize_store_fqdn("test-store.myshopify.com/admin"),
            "test-store.myshopify.com"
        );
    }

    #[test]
    fn test_theme_kit_access_domain_default() {
        assert_eq!(
            theme_kit_access_domain(None),
            DEFAULT_THEME_KIT_ACCESS_DOMAIN
        );
    }

    #[test]
    fn test_theme_kit_access_domain_override() {
        let mut env = std::collections::HashMap::new();
        env.insert(
            EnvVars::THEME_KIT_ACCESS_DOMAIN.to_string(),
            "custom.example.com".to_string(),
        );
        assert_eq!(theme_kit_access_domain(Some(&env)), "custom.example.com");
    }

    #[test]
    fn test_bugsnag_key() {
        assert_eq!(BUGSNAG_API_KEY, "9e1e6889176fd0c795d5c659225e0fae");
    }

    #[test]
    fn test_cache_path_returns_string() {
        let path = cache_path();
        assert!(!path.is_empty());
    }

    #[test]
    fn test_logs_path_returns_string() {
        let path = logs_path();
        assert!(!path.is_empty());
    }

    #[test]
    fn test_is_env_truthy() {
        let mut env = std::collections::HashMap::new();
        env.insert("TEST_FLAG".to_string(), "1".to_string());
        assert!(is_env_truthy(Some(&env), "TEST_FLAG"));
        assert!(!is_env_truthy(None, "TEST_FLAG"));
    }

    #[test]
    fn test_get_env_opt() {
        let mut env = std::collections::HashMap::new();
        env.insert("TEST_KEY".to_string(), "val".to_string());
        assert_eq!(get_env_opt(Some(&env), "TEST_KEY"), Some("val".to_string()));
        assert_eq!(get_env_opt(None, "TEST_KEY"), None);
    }

    #[test]
    fn test_identity_client_id() {
        assert_eq!(identity_client_id(), "fbdb2649-e327-4907-8f67-908d24cfd7e3");
    }

    #[test]
    fn test_identity_application_id() {
        assert_eq!(
            identity_application_id("partners"),
            "271e16d403dfa18082ffb3d197bd2b5f4479c3fc32736d69296829cbb28d41a6"
        );
        assert_eq!(
            identity_application_id("admin"),
            "7ee65a63608843c577db8b23c4d7316ea0a01bd2f7594f8a9c06ea668c1b775c"
        );
    }
}
