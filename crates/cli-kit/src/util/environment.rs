use std::collections::HashMap;

pub fn get_environment_variables() -> HashMap<String, String> {
    std::env::vars().collect()
}

pub fn is_truthy(variable: &str) -> bool {
    matches!(variable, "1" | "true" | "TRUE" | "yes" | "YES")
}

pub fn get_app_automation_token() -> Option<String> {
    std::env::var("SHOPIFY_APP_AUTOMATION_TOKEN")
        .ok()
        .or_else(|| std::env::var("SHOPIFY_CLI_PARTNERS_TOKEN").ok())
}

pub fn get_organization() -> Option<String> {
    std::env::var("SHOPIFY_ORG").ok()
}

pub fn get_backend_port() -> Option<u16> {
    std::env::var("SHOPIFY_BACKEND_PORT")
        .ok()
        .and_then(|v| v.parse::<u16>().ok())
}

pub fn get_identity_token_information() -> Option<IdentityTokenInfo> {
    let access_token = std::env::var("SHOPIFY_CLI_IDENTITY_TOKEN").ok()?;
    let refresh_token = std::env::var("SHOPIFY_CLI_REFRESH_TOKEN").ok()?;
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
    let val = environment
        .and_then(|e| e.get("SHOPIFY_FLAG_JSON"))
        .cloned()
        .or_else(|| std::env::var("SHOPIFY_FLAG_JSON").ok())
        .unwrap_or_default();
    is_truthy(&val)
}

pub fn block_partners_access() -> bool {
    let val = std::env::var("SHOPIFY_CLI_NEVER_USE_PARTNERS_API").unwrap_or_default();
    is_truthy(&val)
}

pub fn skip_network_level_retry(environment: Option<&HashMap<String, String>>) -> bool {
    let val = environment
        .and_then(|e| e.get("SHOPIFY_CLI_SKIP_NETWORK_LEVEL_RETRY"))
        .cloned()
        .or_else(|| std::env::var("SHOPIFY_CLI_SKIP_NETWORK_LEVEL_RETRY").ok())
        .unwrap_or_default();
    is_truthy(&val)
}

pub fn max_request_time_for_network_calls_ms(environment: Option<&HashMap<String, String>>) -> u64 {
    let val = environment
        .and_then(|e| e.get("SHOPIFY_CLI_REQUEST_TIMEOUT_MS"))
        .cloned()
        .or_else(|| std::env::var("SHOPIFY_CLI_REQUEST_TIMEOUT_MS").ok());
    val.and_then(|v| v.parse::<u64>().ok()).unwrap_or(30_000)
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
}
