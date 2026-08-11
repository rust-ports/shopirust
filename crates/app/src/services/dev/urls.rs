//! Frontend / application URL helpers for `app dev` (tunnel wiring is T7).

use crate::error::AppError;
use serde::{Deserialize, Serialize};

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
    }
    old.to_string().trim_end_matches('/').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn localhost_frontend_url() {
        let result = generate_frontend_url(FrontendUrlOptions::Localhost { port: 9293 }).unwrap();
        assert_eq!(result.frontend_url, "https://localhost");
        assert_eq!(result.frontend_port, 9293);
        assert!(result.using_localhost);
    }

    #[test]
    fn tunnel_url_parses() {
        let result = generate_frontend_url(FrontendUrlOptions::TunnelUrl {
            tunnel_url: "https://example.trycloudflare.com:4040".into(),
        })
        .unwrap();
        assert_eq!(result.frontend_url, "https://example.trycloudflare.com");
        assert_eq!(result.frontend_port, 4040);
    }

    #[test]
    fn invalid_tunnel_url_errors() {
        assert!(generate_frontend_url(FrontendUrlOptions::TunnelUrl {
            tunnel_url: "not-a-url".into(),
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
}
