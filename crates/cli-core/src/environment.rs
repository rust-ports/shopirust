use std::collections::HashMap;
use std::path::Path;

pub fn is_terminal_interactive() -> bool {
    is_terminal::is_terminal(std::io::stdout()) && !is_dumb_terminal()
}

fn is_dumb_terminal() -> bool {
    std::env::var("TERM").as_deref() == Ok("dumb")
}

pub fn home_directory() -> String {
    dirs::home_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "/tmp".to_string())
}

pub fn is_development(env: Option<&HashMap<String, String>>) -> bool {
    let val = env
        .and_then(|e| e.get("SHOPIFY_ENV"))
        .cloned()
        .unwrap_or_else(|| std::env::var("SHOPIFY_ENV").unwrap_or_default());
    val == "development"
}

pub fn is_verbose(env: Option<&HashMap<String, String>>) -> bool {
    let val = env
        .and_then(|e| e.get("SHOPIFY_CLI_VERBOSE"))
        .cloned()
        .unwrap_or_else(|| std::env::var("SHOPIFY_CLI_VERBOSE").unwrap_or_default());
    is_truthy(&val)
}

pub fn is_shopify(env: Option<&HashMap<String, String>>) -> bool {
    let val = env
        .and_then(|e| e.get("SHOPIFY_RUN_AS_SHOPIFY"))
        .cloned()
        .unwrap_or_else(|| std::env::var("SHOPIFY_RUN_AS_SHOPIFY").unwrap_or_default());
    is_truthy(&val)
}

pub fn is_unit_test(env: Option<&HashMap<String, String>>) -> bool {
    let val = env
        .and_then(|e| e.get("SHOPIFY_UNIT_TEST"))
        .cloned()
        .unwrap_or_else(|| std::env::var("SHOPIFY_UNIT_TEST").unwrap_or_default());
    is_truthy(&val)
}

pub fn analytics_disabled(env: Option<&HashMap<String, String>>) -> bool {
    is_truthy(
        &env.and_then(|e| e.get("SHOPIFY_CLI_NO_ANALYTICS"))
            .cloned()
            .unwrap_or_else(|| std::env::var("SHOPIFY_CLI_NO_ANALYTICS").unwrap_or_default()),
    ) || is_debug_mode(env)
}

fn is_debug_mode(env: Option<&HashMap<String, String>>) -> bool {
    let val = env
        .and_then(|e| e.get("DEBUG"))
        .cloned()
        .unwrap_or_else(|| std::env::var("DEBUG").unwrap_or_default());
    val == "*" || val == "true"
}

pub fn always_log_analytics(env: Option<&HashMap<String, String>>) -> bool {
    let val = env
        .and_then(|e| e.get("SHOPIFY_CLI_ALWAYS_LOG_ANALYTICS"))
        .cloned()
        .unwrap_or_else(|| std::env::var("SHOPIFY_CLI_ALWAYS_LOG_ANALYTICS").unwrap_or_default());
    is_truthy(&val)
}

pub fn always_log_metrics(env: Option<&HashMap<String, String>>) -> bool {
    let val = env
        .and_then(|e| e.get("SHOPIFY_CLI_ALWAYS_LOG_METRICS"))
        .cloned()
        .unwrap_or_else(|| std::env::var("SHOPIFY_CLI_ALWAYS_LOG_METRICS").unwrap_or_default());
    is_truthy(&val)
}

pub fn first_party_dev(env: Option<&HashMap<String, String>>) -> bool {
    let val = env
        .and_then(|e| e.get("SHOPIFY_CLI_1P"))
        .cloned()
        .unwrap_or_else(|| std::env::var("SHOPIFY_CLI_1P").unwrap_or_default());
    is_truthy(&val)
}

pub fn can_run_doctor_release(env: Option<&HashMap<String, String>>) -> bool {
    let val = env
        .and_then(|e| e.get("SHOPIFY_CLI_ENABLE_DOCTOR_RELEASE"))
        .cloned()
        .unwrap_or_else(|| std::env::var("SHOPIFY_CLI_ENABLE_DOCTOR_RELEASE").unwrap_or_default());
    is_truthy(&val)
}

pub fn gitpod_url(env: Option<&HashMap<String, String>>) -> Option<String> {
    env.and_then(|e| e.get("GITPOD_WORKSPACE_URL").cloned())
        .or_else(|| std::env::var("GITPOD_WORKSPACE_URL").ok())
}

pub fn codespace_name(env: Option<&HashMap<String, String>>) -> Option<String> {
    env.and_then(|e| e.get("CODESPACE_NAME").cloned())
        .or_else(|| std::env::var("CODESPACE_NAME").ok())
}

pub fn is_cloud_environment(env: Option<&HashMap<String, String>>) -> bool {
    gitpod_url(env).is_some() || codespace_name(env).is_some() || is_cloud_shell(env)
}

fn is_cloud_shell(env: Option<&HashMap<String, String>>) -> bool {
    is_truthy(
        &env.and_then(|e| e.get("GOOGLE_CLOUD_SHELL"))
            .cloned()
            .or_else(|| std::env::var("GOOGLE_CLOUD_SHELL").ok())
            .unwrap_or_default(),
    )
}

pub fn theme_token(env: Option<&HashMap<String, String>>) -> Option<String> {
    env.and_then(|e| e.get("SHOPIFY_CLI_THEME_TOKEN").cloned())
        .or_else(|| std::env::var("SHOPIFY_CLI_THEME_TOKEN").ok())
}

pub fn get_theme_kit_access_domain(env: Option<&HashMap<String, String>>) -> String {
    env.and_then(|e| e.get("SHOPIFY_CLI_THEME_KIT_ACCESS_DOMAIN").cloned())
        .or_else(|| std::env::var("SHOPIFY_CLI_THEME_KIT_ACCESS_DOMAIN").ok())
        .unwrap_or_else(|| "theme-kit-access.shopifyapps.com".to_string())
}

fn is_truthy(variable: &str) -> bool {
    matches!(variable, "1" | "true" | "TRUE" | "yes" | "YES")
}

/// Load `.env` from `directory` (or cwd) and export keys that are not already set.
/// Returns the parsed map (including pre-existing env values that were not overwritten).
pub fn load_environment(directory: Option<&Path>) -> HashMap<String, String> {
    let dir = directory
        .map(Path::to_path_buf)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| Path::new(".").to_path_buf()));
    let path = dir.join(".env");
    let mut out = HashMap::new();
    let Ok(raw) = std::fs::read_to_string(path) else {
        return out;
    };
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim().trim_matches('"').trim_matches('\'').to_string();
        if key.is_empty() {
            continue;
        }
        out.insert(key.to_string(), value.clone());
        if std::env::var(key).is_err() {
            std::env::set_var(key, value);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_development_checks_env() {
        let mut env = HashMap::new();
        env.insert("SHOPIFY_ENV".into(), "development".into());
        assert!(is_development(Some(&env)));
        assert!(!is_development(None));
    }

    #[test]
    fn test_first_party_dev() {
        let mut env = HashMap::new();
        env.insert("SHOPIFY_CLI_1P".into(), "1".into());
        assert!(first_party_dev(Some(&env)));
        assert!(!first_party_dev(None));
    }

    #[test]
    fn test_is_cloud_environment_gitpod() {
        let mut env = HashMap::new();
        env.insert("GITPOD_WORKSPACE_URL".into(), "https://gitpod.io".into());
        assert!(is_cloud_environment(Some(&env)));
    }

    #[test]
    fn test_is_cloud_environment_none() {
        assert!(!is_cloud_environment(None));
    }

    #[test]
    fn test_theme_token() {
        let mut env = HashMap::new();
        env.insert("SHOPIFY_CLI_THEME_TOKEN".into(), "shptka_test".into());
        assert_eq!(theme_token(Some(&env)), Some("shptka_test".into()));
        assert!(theme_token(None).is_none());
    }

    #[test]
    fn test_get_theme_kit_access_domain_default() {
        let domain = get_theme_kit_access_domain(None);
        assert_eq!(domain, "theme-kit-access.shopifyapps.com");
    }

    #[test]
    fn test_home_directory_returns_string() {
        let home = home_directory();
        assert!(!home.is_empty());
    }

    #[test]
    fn test_is_verbose() {
        let mut env = HashMap::new();
        env.insert("SHOPIFY_CLI_VERBOSE".into(), "1".into());
        assert!(is_verbose(Some(&env)));
        assert!(!is_verbose(None));
    }

    #[test]
    fn test_is_shopify() {
        let mut env = HashMap::new();
        env.insert("SHOPIFY_RUN_AS_SHOPIFY".into(), "true".into());
        assert!(is_shopify(Some(&env)));
        assert!(!is_shopify(None));
    }

    #[test]
    fn test_is_unit_test() {
        let mut env = HashMap::new();
        env.insert("SHOPIFY_UNIT_TEST".into(), "1".into());
        assert!(is_unit_test(Some(&env)));
        assert!(!is_unit_test(None));
    }

    #[test]
    fn test_analytics_disabled() {
        let mut env = HashMap::new();
        env.insert("SHOPIFY_CLI_NO_ANALYTICS".into(), "1".into());
        assert!(analytics_disabled(Some(&env)));
        assert!(!analytics_disabled(None));
    }

    #[test]
    fn test_always_log_analytics() {
        let mut env = HashMap::new();
        env.insert("SHOPIFY_CLI_ALWAYS_LOG_ANALYTICS".into(), "1".into());
        assert!(always_log_analytics(Some(&env)));
        assert!(!always_log_analytics(None));
    }

    #[test]
    fn test_always_log_metrics() {
        let mut env = HashMap::new();
        env.insert("SHOPIFY_CLI_ALWAYS_LOG_METRICS".into(), "TRUE".into());
        assert!(always_log_metrics(Some(&env)));
        assert!(!always_log_metrics(None));
    }

    #[test]
    fn test_can_run_doctor_release() {
        let mut env = HashMap::new();
        env.insert("SHOPIFY_CLI_ENABLE_DOCTOR_RELEASE".into(), "1".into());
        assert!(can_run_doctor_release(Some(&env)));
        assert!(!can_run_doctor_release(None));
    }

    #[test]
    fn test_gitpod_url() {
        let mut env = HashMap::new();
        env.insert("GITPOD_WORKSPACE_URL".into(), "https://gitpod.io".into());
        assert_eq!(gitpod_url(Some(&env)), Some("https://gitpod.io".into()));
        assert!(gitpod_url(None).is_none());
    }

    #[test]
    fn test_codespace_name() {
        let mut env = HashMap::new();
        env.insert("CODESPACE_NAME".into(), "my-codespace".into());
        assert_eq!(codespace_name(Some(&env)), Some("my-codespace".into()));
        assert!(codespace_name(None).is_none());
    }

    #[test]
    fn load_environment_parses_dotenv() {
        let dir = std::env::temp_dir().join(format!("cli-env-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(
            dir.join(".env"),
            "SHOPIFY_FLAG_STORE=demo.myshopify.com\n# comment\nEMPTY=\n",
        )
        .unwrap();
        let map = load_environment(Some(&dir));
        assert_eq!(
            map.get("SHOPIFY_FLAG_STORE").map(String::as_str),
            Some("demo.myshopify.com")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
