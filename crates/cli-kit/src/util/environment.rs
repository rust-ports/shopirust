use crate::constants::{self, get_env, get_env_opt, is_env_truthy, EnvVars};
use std::collections::HashMap;

pub fn get_environment_variables() -> HashMap<String, String> {
    std::env::vars().collect()
}

pub fn is_truthy(variable: &str) -> bool {
    matches!(variable, "1" | "true" | "TRUE" | "yes" | "YES")
}

pub fn get_app_automation_token() -> Option<String> {
    std::env::var(EnvVars::APP_AUTOMATION_TOKEN)
        .ok()
        .or_else(|| std::env::var(EnvVars::PARTNERS_TOKEN).ok())
}

pub fn get_organization() -> Option<String> {
    std::env::var(EnvVars::ORGANIZATION).ok()
}

pub fn get_backend_port() -> Option<u16> {
    std::env::var(EnvVars::BACKEND_PORT)
        .ok()
        .and_then(|v| v.parse::<u16>().ok())
}

pub fn get_identity_token_information() -> Option<IdentityTokenInfo> {
    let access_token = std::env::var(EnvVars::IDENTITY_TOKEN).ok()?;
    let refresh_token = std::env::var(EnvVars::REFRESH_TOKEN).ok()?;
    let user_id = std::env::var("SHOPIFY_CLI_USER_ID")
        .ok()
        .unwrap_or_else(|| {
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(access_token.as_bytes());
            hex::encode(hasher.finalize())
        });
    Some(IdentityTokenInfo {
        access_token,
        refresh_token,
        user_id,
    })
}

pub struct IdentityTokenInfo {
    pub access_token: String,
    pub refresh_token: String,
    pub user_id: String,
}

pub fn json_output_enabled(environment: Option<&HashMap<String, String>>) -> bool {
    is_env_truthy(environment, EnvVars::JSON)
}

pub fn block_partners_access() -> bool {
    is_truthy(&std::env::var(EnvVars::NEVER_USE_PARTNERS_API).unwrap_or_default())
}

pub fn skip_network_level_retry(environment: Option<&HashMap<String, String>>) -> bool {
    is_env_truthy(environment, EnvVars::SKIP_NETWORK_RETRY)
}

pub fn max_request_time_for_network_calls_ms(environment: Option<&HashMap<String, String>>) -> u64 {
    get_env_opt(environment, EnvVars::MAX_REQUEST_TIME_MS)
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(30_000)
}

/// Check if verbose mode is enabled (SHOPIFY_FLAG_VERBOSE).
pub fn is_verbose(environment: Option<&HashMap<String, String>>) -> bool {
    is_env_truthy(environment, EnvVars::VERBOSE)
}

/// Check if analytics are disabled.
pub fn analytics_disabled(environment: Option<&HashMap<String, String>>) -> bool {
    is_env_truthy(environment, EnvVars::NO_ANALYTICS) || is_debug_mode(environment)
}

fn is_debug_mode(environment: Option<&HashMap<String, String>>) -> bool {
    let val = get_env(environment, "DEBUG");
    val == "*" || val == "true"
}

/// Check if always-log-analytics is enabled.
pub fn always_log_analytics(environment: Option<&HashMap<String, String>>) -> bool {
    is_env_truthy(environment, EnvVars::ALWAYS_LOG_ANALYTICS)
}

/// Check if first-party dev mode is active.
pub fn first_party_dev(environment: Option<&HashMap<String, String>>) -> bool {
    is_env_truthy(environment, EnvVars::FIRST_PARTY)
}

/// Check if running as a Shopify internal user.
pub fn is_shopify(environment: Option<&HashMap<String, String>>) -> bool {
    is_env_truthy(environment, EnvVars::RUN_AS_SHOPIFY)
}

/// Get the theme token from environment.
pub fn theme_token(environment: Option<&HashMap<String, String>>) -> Option<String> {
    get_env_opt(environment, EnvVars::THEME_TOKEN)
}

/// Get the theme kit access domain with env override support.
pub fn theme_kit_access_domain(environment: Option<&HashMap<String, String>>) -> String {
    get_env_opt(environment, EnvVars::THEME_KIT_ACCESS_DOMAIN)
        .unwrap_or_else(|| constants::DEFAULT_THEME_KIT_ACCESS_DOMAIN.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_truthy_true() {
        assert!(is_truthy("1"));
        assert!(is_truthy("true"));
        assert!(is_truthy("TRUE"));
        assert!(is_truthy("yes"));
        assert!(is_truthy("YES"));
    }

    #[test]
    fn test_is_truthy_false() {
        assert!(!is_truthy("0"));
        assert!(!is_truthy("false"));
        assert!(!is_truthy(""));
        assert!(!is_truthy("no"));
    }

    #[test]
    fn test_block_partners_access_default() {
        assert!(!block_partners_access());
    }

    #[test]
    fn test_json_output_enabled_default() {
        assert!(!json_output_enabled(None));
    }

    #[test]
    fn test_max_request_timeout_default() {
        assert_eq!(max_request_time_for_network_calls_ms(None), 30_000);
    }

    #[test]
    fn test_is_verbose() {
        let mut env = HashMap::new();
        env.insert(EnvVars::VERBOSE.to_string(), "1".to_string());
        assert!(is_verbose(Some(&env)));
        assert!(!is_verbose(None));
    }

    #[test]
    fn test_analytics_disabled() {
        let mut env = HashMap::new();
        env.insert(EnvVars::NO_ANALYTICS.to_string(), "1".to_string());
        assert!(analytics_disabled(Some(&env)));
        assert!(!analytics_disabled(None));
    }

    #[test]
    fn test_first_party_dev() {
        let mut env = HashMap::new();
        env.insert(EnvVars::FIRST_PARTY.to_string(), "1".to_string());
        assert!(first_party_dev(Some(&env)));
        assert!(!first_party_dev(None));
    }

    #[test]
    fn test_is_shopify() {
        let mut env = HashMap::new();
        env.insert(EnvVars::RUN_AS_SHOPIFY.to_string(), "1".to_string());
        assert!(is_shopify(Some(&env)));
        assert!(!is_shopify(None));
    }

    #[test]
    fn test_theme_token() {
        let mut env = HashMap::new();
        env.insert(EnvVars::THEME_TOKEN.to_string(), "shptka_test".to_string());
        assert_eq!(theme_token(Some(&env)), Some("shptka_test".to_string()));
        assert!(theme_token(None).is_none());
    }

    #[test]
    fn test_theme_kit_access_domain_default() {
        assert_eq!(
            theme_kit_access_domain(None),
            constants::DEFAULT_THEME_KIT_ACCESS_DOMAIN
        );
    }

    #[test]
    fn test_skip_network_level_retry() {
        let mut env = HashMap::new();
        env.insert(EnvVars::SKIP_NETWORK_RETRY.to_string(), "1".to_string());
        assert!(skip_network_level_retry(Some(&env)));
        assert!(!skip_network_level_retry(None));
    }

    #[test]
    fn test_get_organization() {
        assert!(get_organization().is_none());
    }
}
