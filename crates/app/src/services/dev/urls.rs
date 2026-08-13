//! Frontend / application URL helpers for `app dev`.

use crate::error::AppError;
use crate::local_storage::{get_cached_app_info, set_cached_app_info, CachedAppInfo};
use crate::models::loader::LoadedApp;
use crate::prompts::dev::prompt_update_urls;
use crate::prompts::Prompter;
use crate::services::config::patch_app_configuration_file;
use cli_api::DeveloperPlatformClient;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppProxyUrls {
    pub proxy_url: String,
    pub proxy_sub_path: String,
    pub proxy_sub_path_prefix: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplicationUrls {
    pub application_url: String,
    pub redirect_url_whitelist: Vec<String>,
    pub app_proxy: Option<AppProxyUrls>,
}

#[derive(Debug, Clone)]
pub enum FrontendUrlOptions {
    Localhost {
        port: u16,
    },
    TunnelUrl {
        tunnel_url: String,
    },
    /// Tunnel client not yet available (T7); caller provides a resolved URL.
    Resolved {
        frontend_url: String,
        frontend_port: u16,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontendUrlResult {
    pub frontend_url: String,
    pub frontend_port: u16,
    pub using_localhost: bool,
}

/// Resolve the frontend URL used for extension preview / app proxying.
pub fn generate_frontend_url(options: FrontendUrlOptions) -> Result<FrontendUrlResult, AppError> {
    if let Ok(url) = std::env::var("CODESPACE_NAME") {
        if !url.is_empty() {
            let domain = std::env::var("GITHUB_CODESPACES_PORT_FORWARDING_DOMAIN")
                .unwrap_or_else(|_| "app.github.dev".into());
            let port = 4040u16;
            return Ok(FrontendUrlResult {
                frontend_url: format!("https://{url}-{port}.{domain}"),
                frontend_port: port,
                using_localhost: false,
            });
        }
    }
    if let Ok(gitpod) = std::env::var("GITPOD_WORKSPACE_URL") {
        if !gitpod.is_empty() {
            let host = gitpod.trim_start_matches("https://");
            let port = 4040u16;
            return Ok(FrontendUrlResult {
                frontend_url: format!("https://{port}-{host}"),
                frontend_port: port,
                using_localhost: false,
            });
        }
    }
    match options {
        FrontendUrlOptions::Localhost { port } => Ok(FrontendUrlResult {
            frontend_url: "https://localhost".into(),
            frontend_port: port,
            using_localhost: true,
        }),
        FrontendUrlOptions::TunnelUrl { tunnel_url } => {
            let matches = parse_tunnel_url(&tunnel_url)?;
            Ok(FrontendUrlResult {
                frontend_url: matches.0,
                frontend_port: matches.1,
                using_localhost: false,
            })
        }
        FrontendUrlOptions::Resolved {
            frontend_url,
            frontend_port,
        } => Ok(FrontendUrlResult {
            frontend_url,
            frontend_port,
            using_localhost: false,
        }),
    }
}

/// Proxy URL including port when using localhost (`https://localhost:{port}`).
pub fn proxy_url_from_frontend(frontend: &FrontendUrlResult) -> String {
    if frontend.using_localhost {
        format!("{}:{}", frontend.frontend_url, frontend.frontend_port)
    } else {
        frontend.frontend_url.clone()
    }
}

fn parse_tunnel_url(tunnel_url: &str) -> Result<(String, u16), AppError> {
    // Upstream format: "https://my-tunnel-url:port"
    let re = regex_lite::Regex::new(r"(https://[^:]+):([0-9]+)").unwrap();
    let caps = re.captures(tunnel_url).ok_or_else(|| {
        AppError::message(format!(
            "Invalid tunnel URL: {tunnel_url}. Valid format: \"https://my-tunnel-url:port\""
        ))
    })?;
    let url = caps.get(1).unwrap().as_str().to_string();
    let port: u16 = caps
        .get(2)
        .unwrap()
        .as_str()
        .parse()
        .map_err(|_| AppError::message("Invalid tunnel port"))?;
    Ok((url, port))
}

/// Build application + auth callback whitelist URLs from a base URL.
pub fn generate_application_urls(
    base_url: &str,
    auth_callback_path: Option<&[String]>,
    proxy_fields: Option<(String, String, String)>,
) -> ApplicationUrls {
    let redirect_url_whitelist = if let Some(paths) = auth_callback_path {
        if !paths.is_empty() {
            paths
                .iter()
                .filter(|p| !p.is_empty())
                .map(|p| format!("{base_url}{p}"))
                .collect()
        } else {
            default_callbacks(base_url)
        }
    } else {
        default_callbacks(base_url)
    };

    let app_proxy = proxy_fields.map(|(url, subpath, prefix)| AppProxyUrls {
        proxy_url: replace_host(&prepend_application_url(&url, base_url), base_url),
        proxy_sub_path: subpath,
        proxy_sub_path_prefix: prefix,
    });

    ApplicationUrls {
        application_url: base_url.to_string(),
        redirect_url_whitelist,
        app_proxy,
    }
}

fn default_callbacks(base_url: &str) -> Vec<String> {
    vec![
        format!("{base_url}/auth/callback"),
        format!("{base_url}/auth/shopify/callback"),
        format!("{base_url}/api/auth/callback"),
    ]
}

fn prepend_application_url(url: &str, base_url: &str) -> String {
    if url.starts_with("http://") || url.starts_with("https://") {
        url.to_string()
    } else if url.starts_with('/') {
        format!("{base_url}{url}")
    } else {
        format!("{base_url}/{url}")
    }
}

fn replace_host(old_url: &str, new_url: &str) -> String {
    let Ok(mut old) = url::Url::parse(old_url) else {
        return old_url.trim_end_matches('/').to_string();
    };
    let Ok(new) = url::Url::parse(new_url) else {
        return old_url.trim_end_matches('/').to_string();
    };
    let _ = old.set_host(new.host_str());
    if let Some(port) = new.port() {
        let _ = old.set_port(Some(port));
    } else {
        let _ = old.set_port(None);
    }
    old.to_string().trim_end_matches('/').to_string()
}

/// Read currently published URLs from a remote app configuration object.
pub fn get_urls(remote_app_config: Option<&Value>) -> ApplicationUrls {
    let application_url = remote_app_config
        .and_then(|c| c.get("application_url"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let redirect_url_whitelist = remote_app_config
        .and_then(|c| c.pointer("/auth/redirect_urls"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    let app_proxy = remote_app_config.and_then(|c| c.get("app_proxy")).and_then(|p| {
        Some(AppProxyUrls {
            proxy_url: p.get("url")?.as_str()?.to_string(),
            proxy_sub_path: p.get("subpath")?.as_str()?.to_string(),
            proxy_sub_path_prefix: p.get("prefix")?.as_str()?.to_string(),
        })
    });
    ApplicationUrls {
        application_url,
        redirect_url_whitelist,
        app_proxy,
    }
}

/// Push URLs to Partners (`updateURLs`) and optionally patch local TOML when client_id matches.
pub async fn update_urls(
    urls: &ApplicationUrls,
    api_key: &str,
    client: &dyn DeveloperPlatformClient,
    local_app: Option<&LoadedApp>,
) -> Result<(), AppError> {
    let mut input = json!({
        "apiKey": api_key,
        "applicationUrl": urls.application_url,
        "redirectUrlWhitelist": urls.redirect_url_whitelist,
    });
    if let Some(proxy) = &urls.app_proxy {
        input["appProxy"] = json!({
            "proxyUrl": proxy.proxy_url,
            "proxySubPath": proxy.proxy_sub_path,
            "proxySubPathPrefix": proxy.proxy_sub_path_prefix,
        });
    }
    let result = client
        .update_urls(input)
        .await
        .map_err(|e| AppError::message(e.to_string()))?;
    let errors = result
        .pointer("/appUpdate/userErrors")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    if !errors.is_empty() {
        let messages: Vec<_> = errors
            .iter()
            .filter_map(|e| e.get("message").and_then(|m| m.as_str()))
            .collect();
        return Err(AppError::message(messages.join(", ")));
    }

    if let Some(app) = local_app {
        if app.configuration.client_id.as_deref() == Some(api_key) {
            let mut patch = json!({
                "application_url": urls.application_url,
                "auth": { "redirect_urls": urls.redirect_url_whitelist },
            });
            if let Some(proxy) = &urls.app_proxy {
                patch["app_proxy"] = json!({
                    "url": proxy.proxy_url,
                    "subpath": proxy.proxy_sub_path,
                    "prefix": proxy.proxy_sub_path_prefix,
                });
            }
            patch_app_configuration_file(&app.configuration_path, &patch)?;
        }
    }
    Ok(())
}

pub struct ShouldUpdateUrlsOptions<'a> {
    pub current_urls: ApplicationUrls,
    pub app_directory: &'a Path,
    pub cached_update_urls: Option<bool>,
    pub new_app: bool,
    pub local_app: Option<&'a LoadedApp>,
    pub api_key: String,
    pub new_urls: ApplicationUrls,
    pub using_dev_sessions: bool,
    pub interactive: bool,
}

/// Decide whether to push/patch URLs (`--no-update` skips the caller).
pub fn should_or_prompt_update_urls(
    options: ShouldUpdateUrlsOptions<'_>,
    prompter: Option<&dyn Prompter>,
) -> Result<bool, AppError> {
    if let Some(app) = options.local_app {
        if app.configuration.client_id.as_deref() != Some(options.api_key.as_str()) {
            return Ok(true);
        }
    }
    if options.new_app || !options.interactive {
        return Ok(true);
    }
    if let Some(cached) = options.cached_update_urls {
        return Ok(cached);
    }

    let should = if let Some(prompter) = prompter {
        prompt_update_urls(
            prompter,
            options.using_dev_sessions,
            &options.current_urls.application_url,
            &options.current_urls.redirect_url_whitelist,
            &options.new_urls,
        )?
    } else {
        true
    };

    if let Some(app) = options.local_app {
        patch_app_configuration_file(
            &app.configuration_path,
            &json!({ "build": { "automatically_update_urls_on_dev": should } }),
        )?;
    } else {
        set_cached_app_info(&CachedAppInfo {
            directory: options.app_directory.display().to_string(),
            update_urls: Some(should),
            ..get_cached_app_info(options.app_directory).unwrap_or_default()
        })?;
    }
    Ok(should)
}

/// Collect `auth_callback_path` from web instances (first non-empty wins).
pub fn auth_callback_paths_from_webs(
    webs: &[crate::models::loader::WebInstance],
) -> Option<Vec<String>> {
    webs.iter()
        .map(|w| w.auth_callback_path.clone())
        .find(|p| !p.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::app::AppConfiguration;
    use crate::models::identifiers::Identifiers;
    use crate::prompts::InjectedPrompter;
    use crate::test_support::MockClient;
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[test]
    fn localhost_frontend_url() {
        let result = generate_frontend_url(FrontendUrlOptions::Localhost { port: 9293 }).unwrap();
        assert_eq!(result.frontend_url, "https://localhost");
        assert_eq!(result.frontend_port, 9293);
        assert!(result.using_localhost);
        assert_eq!(proxy_url_from_frontend(&result), "https://localhost:9293");
    }

    #[test]
    fn tunnel_url_parses() {
        let result = generate_frontend_url(FrontendUrlOptions::TunnelUrl {
            tunnel_url: "https://example.trycloudflare.com:4040".into(),
        })
        .unwrap();
        assert_eq!(result.frontend_url, "https://example.trycloudflare.com");
        assert_eq!(result.frontend_port, 4040);
        assert_eq!(
            proxy_url_from_frontend(&result),
            "https://example.trycloudflare.com"
        );
    }

    #[test]
    fn invalid_tunnel_url_errors() {
        assert!(generate_frontend_url(FrontendUrlOptions::TunnelUrl {
            tunnel_url: "not-a-url".into(),
        })
        .is_err());
        assert!(generate_frontend_url(FrontendUrlOptions::TunnelUrl {
            tunnel_url: "https://my-tunnel-provider.io".into(),
        })
        .is_err());
    }

    #[test]
    fn application_urls_default_callbacks() {
        let urls = generate_application_urls("https://app.example", None, None);
        assert_eq!(urls.application_url, "https://app.example");
        assert_eq!(urls.redirect_url_whitelist.len(), 3);
        assert!(urls.redirect_url_whitelist[0].ends_with("/auth/callback"));
    }

    #[test]
    fn application_urls_custom_callbacks() {
        let paths = vec!["/custom/cb".into()];
        let urls = generate_application_urls("https://app.example", Some(&paths), None);
        assert_eq!(
            urls.redirect_url_whitelist,
            vec!["https://app.example/custom/cb"]
        );
    }

    #[test]
    fn application_urls_empty_override_uses_defaults() {
        let urls = generate_application_urls("http://my-base-url", Some(&[]), None);
        assert_eq!(urls.redirect_url_whitelist.len(), 3);
    }

    #[test]
    fn application_urls_array_override() {
        let paths = vec!["/my/custom/path1".into(), "/my/custom/path2".into()];
        let urls = generate_application_urls("http://my-base-url", Some(&paths), None);
        assert_eq!(
            urls.redirect_url_whitelist,
            vec![
                "http://my-base-url/my/custom/path1".to_string(),
                "http://my-base-url/my/custom/path2".to_string()
            ]
        );
    }

    #[test]
    fn application_urls_with_proxy_fields() {
        let urls = generate_application_urls(
            "http://my-base-url",
            Some(&[]),
            Some((
                "http://my-base-url".into(),
                "subpath".into(),
                "prefix".into(),
            )),
        );
        let proxy = urls.app_proxy.unwrap();
        assert_eq!(proxy.proxy_url, "http://my-base-url");
        assert_eq!(proxy.proxy_sub_path, "subpath");
        assert_eq!(proxy.proxy_sub_path_prefix, "prefix");
    }

    #[test]
    fn application_urls_proxy_replaces_host_only() {
        let urls = generate_application_urls(
            "http://my-base-url",
            Some(&[]),
            Some((
                "http://old-base-url/subpath".into(),
                "subpath".into(),
                "prefix".into(),
            )),
        );
        assert_eq!(
            urls.app_proxy.unwrap().proxy_url,
            "http://my-base-url/subpath"
        );
    }

    #[test]
    fn application_urls_relative_proxy() {
        let urls = generate_application_urls(
            "http://my-base-url",
            Some(&[]),
            Some(("/subpath".into(), "subpath".into(), "prefix".into())),
        );
        assert_eq!(
            urls.app_proxy.unwrap().proxy_url,
            "http://my-base-url/subpath"
        );
    }

    #[test]
    fn get_urls_from_remote_config() {
        let remote = json!({
            "application_url": "https://example.com",
            "auth": { "redirect_urls": ["https://example.com/cb"] },
            "app_proxy": { "url": "https://example.com/p", "subpath": "apps", "prefix": "a" }
        });
        let urls = get_urls(Some(&remote));
        assert_eq!(urls.application_url, "https://example.com");
        assert_eq!(urls.redirect_url_whitelist, vec!["https://example.com/cb"]);
        assert_eq!(urls.app_proxy.unwrap().proxy_sub_path, "apps");
    }

    fn loaded(dir: &Path, client_id: &str) -> LoadedApp {
        let config_path = dir.join("shopify.app.toml");
        std::fs::write(
            &config_path,
            format!("client_id = \"{client_id}\"\nname = \"Demo\"\n"),
        )
        .unwrap();
        LoadedApp {
            directory: dir.to_path_buf(),
            configuration_path: config_path,
            configuration: AppConfiguration {
                client_id: Some(client_id.into()),
                name: Some("Demo".into()),
                ..Default::default()
            },
            hidden_config: Default::default(),
            extensions: vec![],
            webs: vec![],
            identifiers: Identifiers::new(),
            name: "Demo".into(),
            errors: vec![],
            dev_application_urls: None,
        }
    }

    fn sample_urls() -> ApplicationUrls {
        ApplicationUrls {
            application_url: "https://example.com".into(),
            redirect_url_whitelist: vec![
                "https://example.com/auth/callback".into(),
                "https://example.com/auth/shopify/callback".into(),
                "https://example.com/api/auth/callback".into(),
            ],
            app_proxy: None,
        }
    }

    #[tokio::test]
    async fn update_urls_sends_request() {
        let client = MockClient::default();
        let urls = sample_urls();
        update_urls(&urls, "apiKey", &client, None).await.unwrap();
        let calls = client.update_urls_calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0]["apiKey"], "apiKey");
        assert_eq!(calls[0]["applicationUrl"], "https://example.com");
    }

    #[tokio::test]
    async fn update_urls_patches_toml_when_client_matches() {
        let dir = tempdir().unwrap();
        let app = loaded(dir.path(), "apiKey");
        let client = MockClient::default();
        update_urls(&sample_urls(), "apiKey", &client, Some(&app))
            .await
            .unwrap();
        let raw = std::fs::read_to_string(&app.configuration_path).unwrap();
        assert!(raw.contains("application_url"));
        assert!(raw.contains("redirect_urls") || raw.contains("auth"));
    }

    #[tokio::test]
    async fn update_urls_user_error() {
        let client = MockClient {
            update_urls_user_errors: vec!["Boom!".into()],
            ..Default::default()
        };
        let err = update_urls(&sample_urls(), "apiKey", &client, None)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("Boom!"));
    }

    #[tokio::test]
    async fn update_urls_includes_app_proxy() {
        let client = MockClient::default();
        let mut urls = sample_urls();
        urls.app_proxy = Some(AppProxyUrls {
            proxy_url: "https://example.com".into(),
            proxy_sub_path: "subpath".into(),
            proxy_sub_path_prefix: "prefix".into(),
        });
        update_urls(&urls, "apiKey", &client, None).await.unwrap();
        let calls = client.update_urls_calls.lock().unwrap();
        assert_eq!(calls[0]["appProxy"]["proxySubPath"], "subpath");
    }

    #[tokio::test]
    async fn update_urls_patches_proxy_in_toml() {
        let dir = tempdir().unwrap();
        let app = loaded(dir.path(), "apiKey");
        let mut urls = sample_urls();
        urls.app_proxy = Some(AppProxyUrls {
            proxy_url: "https://example.com".into(),
            proxy_sub_path: "subpath".into(),
            proxy_sub_path_prefix: "prefix".into(),
        });
        update_urls(&urls, "apiKey", &MockClient::default(), Some(&app))
            .await
            .unwrap();
        let raw = std::fs::read_to_string(&app.configuration_path).unwrap();
        assert!(raw.contains("app_proxy") || raw.contains("subpath"));
    }

    #[test]
    fn should_update_true_for_new_app() {
        let dir = tempdir().unwrap();
        let got = should_or_prompt_update_urls(
            ShouldUpdateUrlsOptions {
                current_urls: sample_urls(),
                app_directory: dir.path(),
                cached_update_urls: None,
                new_app: true,
                local_app: None,
                api_key: "api-key".into(),
                new_urls: sample_urls(),
                using_dev_sessions: false,
                interactive: true,
            },
            None,
        )
        .unwrap();
        assert!(got);
    }

    #[test]
    fn should_update_uses_cached_true() {
        let dir = tempdir().unwrap();
        let got = should_or_prompt_update_urls(
            ShouldUpdateUrlsOptions {
                current_urls: sample_urls(),
                app_directory: dir.path(),
                cached_update_urls: Some(true),
                new_app: false,
                local_app: None,
                api_key: "api-key".into(),
                new_urls: sample_urls(),
                using_dev_sessions: false,
                interactive: true,
            },
            None,
        )
        .unwrap();
        assert!(got);
    }

    #[test]
    fn should_update_uses_cached_false() {
        let dir = tempdir().unwrap();
        let got = should_or_prompt_update_urls(
            ShouldUpdateUrlsOptions {
                current_urls: sample_urls(),
                app_directory: dir.path(),
                cached_update_urls: Some(false),
                new_app: false,
                local_app: None,
                api_key: "api-key".into(),
                new_urls: sample_urls(),
                using_dev_sessions: false,
                interactive: true,
            },
            None,
        )
        .unwrap();
        assert!(!got);
    }

    #[test]
    fn should_update_prompt_yes_caches() {
        let dir = tempdir().unwrap();
        let p = InjectedPrompter::new();
        p.push_confirm(true);
        let got = should_or_prompt_update_urls(
            ShouldUpdateUrlsOptions {
                current_urls: sample_urls(),
                app_directory: dir.path(),
                cached_update_urls: None,
                new_app: false,
                local_app: None,
                api_key: "api-key".into(),
                new_urls: sample_urls(),
                using_dev_sessions: false,
                interactive: true,
            },
            Some(&p),
        )
        .unwrap();
        assert!(got);
        let cached = get_cached_app_info(dir.path()).unwrap();
        assert_eq!(cached.update_urls, Some(true));
    }

    #[test]
    fn should_update_prompt_no() {
        let dir = tempdir().unwrap();
        let p = InjectedPrompter::new();
        p.push_confirm(false);
        let got = should_or_prompt_update_urls(
            ShouldUpdateUrlsOptions {
                current_urls: sample_urls(),
                app_directory: dir.path(),
                cached_update_urls: None,
                new_app: false,
                local_app: None,
                api_key: "api-key".into(),
                new_urls: sample_urls(),
                using_dev_sessions: false,
                interactive: true,
            },
            Some(&p),
        )
        .unwrap();
        assert!(!got);
    }

    #[test]
    fn mismatched_client_id_returns_true_without_cache() {
        let dir = tempdir().unwrap();
        let app = loaded(dir.path(), "different");
        let p = InjectedPrompter::new();
        p.push_confirm(true);
        let got = should_or_prompt_update_urls(
            ShouldUpdateUrlsOptions {
                current_urls: sample_urls(),
                app_directory: dir.path(),
                cached_update_urls: None,
                new_app: false,
                local_app: Some(&app),
                api_key: "api-key".into(),
                new_urls: sample_urls(),
                using_dev_sessions: false,
                interactive: true,
            },
            Some(&p),
        )
        .unwrap();
        assert!(got);
        assert!(get_cached_app_info(dir.path()).is_none());
    }

    #[test]
    fn matching_client_patches_build_flag() {
        let dir = tempdir().unwrap();
        let app = loaded(dir.path(), "api-key");
        let p = InjectedPrompter::new();
        p.push_confirm(true);
        let got = should_or_prompt_update_urls(
            ShouldUpdateUrlsOptions {
                current_urls: sample_urls(),
                app_directory: dir.path(),
                cached_update_urls: None,
                new_app: false,
                local_app: Some(&app),
                api_key: "api-key".into(),
                new_urls: sample_urls(),
                using_dev_sessions: true,
                interactive: true,
            },
            Some(&p),
        )
        .unwrap();
        assert!(got);
        let raw = std::fs::read_to_string(&app.configuration_path).unwrap();
        assert!(raw.contains("automatically_update_urls_on_dev"));
    }

    #[test]
    fn unused_pathbuf_import_ok() {
        let _ = PathBuf::from(".");
    }
}
