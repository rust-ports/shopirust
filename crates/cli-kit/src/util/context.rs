use crate::util::system;

pub fn is_terminal_interactive() -> bool {
    is_terminal::is_terminal(&std::io::stdout())
        && !is_dumb_terminal()
}

fn is_dumb_terminal() -> bool {
    std::env::var("TERM").as_deref() == Ok("dumb")
}

pub fn home_directory() -> String {
    dirs::home_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "/tmp".to_string())
}

pub fn is_development(env: Option<&std::collections::HashMap<String, String>>) -> bool {
    let val = env
        .and_then(|e| e.get("SHOPIFY_ENV"))
        .cloned()
        .unwrap_or_else(|| std::env::var("SHOPIFY_ENV").unwrap_or_default());
    val == "development"
}

pub fn is_verbose(env: Option<&std::collections::HashMap<String, String>>) -> bool {
    let val = env
        .and_then(|e| e.get("SHOPIFY_CLI_VERBOSE"))
        .cloned()
        .unwrap_or_else(|| std::env::var("SHOPIFY_CLI_VERBOSE").unwrap_or_default());
    is_truthy(&val)
}

pub fn is_shopify(env: Option<&std::collections::HashMap<String, String>>) -> bool {
    let val = env
        .and_then(|e| e.get("SHOPIFY_RUN_AS_SHOPIFY"))
        .cloned()
        .unwrap_or_else(|| std::env::var("SHOPIFY_RUN_AS_SHOPIFY").unwrap_or_default());
    is_truthy(&val)
}

pub fn is_unit_test(env: Option<&std::collections::HashMap<String, String>>) -> bool {
    let val = env
        .and_then(|e| e.get("SHOPIFY_UNIT_TEST"))
        .cloned()
        .unwrap_or_else(|| std::env::var("SHOPIFY_UNIT_TEST").unwrap_or_default());
    is_truthy(&val)
}

pub fn analytics_disabled(env: Option<&std::collections::HashMap<String, String>>) -> bool {
    let val = env
        .and_then(|e| e.get("SHOPIFY_CLI_NO_ANALYTICS"))
        .cloned()
        .unwrap_or_else(|| std::env::var("SHOPIFY_CLI_NO_ANALYTICS").unwrap_or_default());
    is_truthy(&val) || is_debug_mode(env)
}

fn is_debug_mode(env: Option<&std::collections::HashMap<String, String>>) -> bool {
    let val = env
        .and_then(|e| e.get("DEBUG"))
        .cloned()
        .unwrap_or_else(|| std::env::var("DEBUG").unwrap_or_default());
    val == "*" || val == "true"
}

pub fn always_log_analytics(env: Option<&std::collections::HashMap<String, String>>) -> bool {
    let val = env
        .and_then(|e| e.get("SHOPIFY_CLI_ALWAYS_LOG_ANALYTICS"))
        .cloned()
        .unwrap_or_else(|| std::env::var("SHOPIFY_CLI_ALWAYS_LOG_ANALYTICS").unwrap_or_default());
    is_truthy(&val)
}

pub fn always_log_metrics(env: Option<&std::collections::HashMap<String, String>>) -> bool {
    let val = env
        .and_then(|e| e.get("SHOPIFY_CLI_ALWAYS_LOG_METRICS"))
        .cloned()
        .unwrap_or_else(|| std::env::var("SHOPIFY_CLI_ALWAYS_LOG_METRICS").unwrap_or_default());
    is_truthy(&val)
}

pub fn first_party_dev(env: Option<&std::collections::HashMap<String, String>>) -> bool {
    let val = env
        .and_then(|e| e.get("SHOPIFY_CLI_1P"))
        .cloned()
        .unwrap_or_else(|| std::env::var("SHOPIFY_CLI_1P").unwrap_or_default());
    is_truthy(&val)
}

pub fn can_run_doctor_release(env: Option<&std::collections::HashMap<String, String>>) -> bool {
    let val = env
        .and_then(|e| e.get("SHOPIFY_CLI_ENABLE_DOCTOR_RELEASE"))
        .cloned()
        .unwrap_or_else(|| std::env::var("SHOPIFY_CLI_ENABLE_DOCTOR_RELEASE").unwrap_or_default());
    is_truthy(&val)
}

pub fn gitpod_url(env: Option<&std::collections::HashMap<String, String>>) -> Option<String> {
    env.and_then(|e| e.get("GITPOD_WORKSPACE_URL").cloned())
        .or_else(|| std::env::var("GITPOD_WORKSPACE_URL").ok())
}

pub fn codespace_name(env: Option<&std::collections::HashMap<String, String>>) -> Option<String> {
    env.and_then(|e| e.get("CODESPACE_NAME").cloned())
        .or_else(|| std::env::var("CODESPACE_NAME").ok())
}

pub fn is_cloud_environment(env: Option<&std::collections::HashMap<String, String>>) -> bool {
    gitpod_url(env).is_some() || codespace_name(env).is_some() || is_cloud_shell(env)
}

fn is_cloud_shell(env: Option<&std::collections::HashMap<String, String>>) -> bool {
    let val = env
        .and_then(|e| e.get("GOOGLE_CLOUD_SHELL"))
        .cloned()
        .or_else(|| std::env::var("GOOGLE_CLOUD_SHELL").ok())
        .unwrap_or_default();
    is_truthy(&val)
}

pub fn theme_token(env: Option<&std::collections::HashMap<String, String>>) -> Option<String> {
    env.and_then(|e| e.get("SHOPIFY_CLI_THEME_TOKEN").cloned())
        .or_else(|| std::env::var("SHOPIFY_CLI_THEME_TOKEN").ok())
}

pub fn get_theme_kit_access_domain(env: Option<&std::collections::HashMap<String, String>>) -> String {
    env.and_then(|e| e.get("SHOPIFY_CLI_THEME_KIT_ACCESS_DOMAIN").cloned())
        .or_else(|| std::env::var("SHOPIFY_CLI_THEME_KIT_ACCESS_DOMAIN").ok())
        .unwrap_or_else(|| "theme-kit-access.shopifyapps.com".to_string())
}

pub async fn has_git() -> bool {
    system::capture_output("git", &["--version"]).await.is_ok()
}

fn is_truthy(variable: &str) -> bool {
    matches!(variable, "1" | "true" | "TRUE" | "yes" | "YES")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_home_directory_returns_string() {
        let home = home_directory();
        assert!(!home.is_empty());
    }

    #[test]
    fn test_is_development_checks_env() {
        let mut env = std::collections::HashMap::new();
        env.insert("SHOPIFY_ENV".into(), "development".into());
        assert!(is_development(Some(&env)));
        assert!(!is_development(None));
    }

    #[test]
    fn test_first_party_dev() {
        let mut env = std::collections::HashMap::new();
        env.insert("SHOPIFY_CLI_1P".into(), "1".into());
        assert!(first_party_dev(Some(&env)));
        assert!(!first_party_dev(None));
    }

    #[test]
    fn test_is_cloud_environment_gitpod() {
        let mut env = std::collections::HashMap::new();
        env.insert("GITPOD_WORKSPACE_URL".into(), "https://gitpod.io".into());
        assert!(is_cloud_environment(Some(&env)));
    }

    #[test]
    fn test_is_cloud_environment_none() {
        assert!(!is_cloud_environment(None));
    }

    #[test]
    fn test_theme_token() {
        let mut env = std::collections::HashMap::new();
        env.insert("SHOPIFY_CLI_THEME_TOKEN".into(), "shptka_test".into());
        assert_eq!(theme_token(Some(&env)), Some("shptka_test".into()));
        assert!(theme_token(None).is_none());
    }

    #[test]
    fn test_get_theme_kit_access_domain_default() {
        let domain = get_theme_kit_access_domain(None);
        assert_eq!(domain, "theme-kit-access.shopifyapps.com");
    }
}
