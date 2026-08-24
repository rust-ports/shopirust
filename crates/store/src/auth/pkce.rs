use crate::auth::config::{
    store_auth_redirect_uri, DEFAULT_STORE_AUTH_PORT, STORE_AUTH_APP_CLIENT_ID,
};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use rand::RngCore;
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct StoreAuthorizationContext {
    pub store: String,
    pub scopes: Vec<String>,
    pub state: String,
    pub port: u16,
    pub redirect_uri: String,
    pub authorization_url: String,
    pub code_verifier: String,
}

#[derive(Debug, Clone)]
pub struct WaitForAuthCodeOptions {
    pub store: String,
    pub state: String,
    pub port: u16,
    pub timeout_ms: u64,
}

pub struct StoreAuthBootstrap {
    pub authorization: StoreAuthorizationContext,
    pub wait_for_auth_code_options: WaitForAuthCodeOptions,
}

pub fn generate_code_verifier() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

pub fn compute_code_challenge(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(digest)
}

pub fn build_store_auth_url(
    store: &str,
    scopes: &[String],
    state: &str,
    redirect_uri: &str,
    code_challenge: &str,
) -> String {
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    serializer.append_pair("client_id", STORE_AUTH_APP_CLIENT_ID);
    serializer.append_pair("scope", &scopes.join(","));
    serializer.append_pair("redirect_uri", redirect_uri);
    serializer.append_pair("state", state);
    serializer.append_pair("response_type", "code");
    serializer.append_pair("code_challenge", code_challenge);
    serializer.append_pair("code_challenge_method", "S256");
    format!(
        "https://{store}/admin/oauth/authorize?{}",
        serializer.finish()
    )
}

pub fn create_pkce_bootstrap(
    store: &str,
    scopes: Vec<String>,
    state: Option<String>,
) -> StoreAuthBootstrap {
    let port = DEFAULT_STORE_AUTH_PORT;
    let state = state.unwrap_or_else(|| Uuid::new_v4().to_string());
    let redirect_uri = store_auth_redirect_uri(port);
    let code_verifier = generate_code_verifier();
    let code_challenge = compute_code_challenge(&code_verifier);
    let authorization_url =
        build_store_auth_url(store, &scopes, &state, &redirect_uri, &code_challenge);

    StoreAuthBootstrap {
        authorization: StoreAuthorizationContext {
            store: store.to_string(),
            scopes,
            state: state.clone(),
            port,
            redirect_uri,
            authorization_url,
            code_verifier,
        },
        wait_for_auth_code_options: WaitForAuthCodeOptions {
            store: store.to_string(),
            state,
            port,
            timeout_ms: 5 * 60 * 1000,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_code_verifier_produces_base64url_43_chars() {
        let verifier = generate_code_verifier();
        assert_eq!(verifier.len(), 43);
        assert!(verifier
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
    }

    #[test]
    fn generate_code_verifier_produces_unique_values() {
        assert_ne!(generate_code_verifier(), generate_code_verifier());
    }

    #[test]
    fn compute_code_challenge_rfc7636_vector() {
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let expected = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";
        assert_eq!(compute_code_challenge(verifier), expected);
    }

    #[test]
    fn build_store_auth_url_includes_pkce_params() {
        let url = url::Url::parse(&build_store_auth_url(
            "shop.myshopify.com",
            &["read_products".into(), "write_products".into()],
            "state-123",
            "http://127.0.0.1:13387/auth/callback",
            "test-challenge-value",
        ))
        .unwrap();

        assert_eq!(url.host_str(), Some("shop.myshopify.com"));
        assert_eq!(url.path(), "/admin/oauth/authorize");
        let pairs: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(pairs.get("client_id").unwrap(), STORE_AUTH_APP_CLIENT_ID);
        assert_eq!(pairs.get("scope").unwrap(), "read_products,write_products");
        assert_eq!(pairs.get("state").unwrap(), "state-123");
        assert_eq!(
            pairs.get("redirect_uri").unwrap(),
            "http://127.0.0.1:13387/auth/callback"
        );
        assert_eq!(pairs.get("response_type").unwrap(), "code");
        assert_eq!(pairs.get("code_challenge").unwrap(), "test-challenge-value");
        assert_eq!(pairs.get("code_challenge_method").unwrap(), "S256");
        assert!(!pairs.contains_key("grant_options[]"));
    }
}
