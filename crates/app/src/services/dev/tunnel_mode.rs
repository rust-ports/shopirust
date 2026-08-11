//! Resolve `app dev` tunnel / localhost networking from CLI flags.

use crate::error::AppError;

/// Default localhost proxy port (upstream `ports.localhost`).
pub const DEFAULT_LOCALHOST_PORT: u16 = 3458;

/// Default GraphiQL port (upstream `ports.graphiql`).
pub const DEFAULT_GRAPHIQL_PORT: u16 = 3457;

/// How the CLI exposes the local app to Shopify during `app dev`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TunnelMode {
    /// CLI creates a Cloudflare (or other) tunnel.
    Auto,
    /// Listen on localhost only (no public tunnel).
    UseLocalhost {
        requested_port: u16,
        actual_port: u16,
    },
    /// Developer-provided tunnel URL (`https://host:port`).
    Custom { url: String },
}

impl TunnelMode {
    pub fn mode_name(&self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::UseLocalhost { .. } => "use-localhost",
            Self::Custom { .. } => "custom",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct TunnelModeFlags {
    pub tunnel_url: Option<String>,
    pub use_localhost: bool,
    pub localhost_port: Option<u16>,
}

/// Resolve tunnel mode from flags (mirrors upstream `getTunnelMode`).
pub async fn get_tunnel_mode(flags: TunnelModeFlags) -> Result<TunnelMode, AppError> {
    if let Some(url) = flags.tunnel_url.filter(|u| !u.is_empty()) {
        return Ok(TunnelMode::Custom { url });
    }

    if !flags.use_localhost && flags.localhost_port.is_none() {
        return Ok(TunnelMode::Auto);
    }

    let requested_port = flags.localhost_port.unwrap_or(DEFAULT_LOCALHOST_PORT);
    let actual_port = get_available_tcp_port(Some(requested_port)).await?;

    if flags.localhost_port.is_some() && actual_port != requested_port {
        return Err(AppError::message(format!(
            "Port {requested_port} is not available. Choose a different port for the --localhost-port flag."
        )));
    }

    Ok(TunnelMode::UseLocalhost {
        requested_port,
        actual_port,
    })
}

/// Find an available TCP port, preferring `preferred` when set.
pub async fn get_available_tcp_port(preferred: Option<u16>) -> Result<u16, AppError> {
    if let Some(port) = preferred {
        if port_free(port).await {
            return Ok(port);
        }
        // If preferred was explicitly requested by caller of get_tunnel_mode, they compare;
        // for general use, scan upward.
        for candidate in (port + 1)..port.saturating_add(100) {
            if port_free(candidate).await {
                return Ok(candidate);
            }
        }
        return Err(AppError::message(format!(
            "Could not find an available TCP port near {port}"
        )));
    }

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|e| AppError::message(format!("failed to bind ephemeral port: {e}")))?;
    let port = listener
        .local_addr()
        .map_err(|e| AppError::message(e.to_string()))?
        .port();
    drop(listener);
    Ok(port)
}

async fn port_free(port: u16) -> bool {
    tokio::net::TcpListener::bind(("127.0.0.1", port))
        .await
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn custom_tunnel_url_wins() {
        let mode = get_tunnel_mode(TunnelModeFlags {
            tunnel_url: Some("https://example.trycloudflare.com:443".into()),
            use_localhost: true,
            localhost_port: Some(9999),
        })
        .await
        .unwrap();
        assert!(matches!(mode, TunnelMode::Custom { .. }));
        assert_eq!(mode.mode_name(), "custom");
    }

    #[tokio::test]
    async fn auto_when_no_localhost_flags() {
        let mode = get_tunnel_mode(TunnelModeFlags::default()).await.unwrap();
        assert_eq!(mode, TunnelMode::Auto);
        assert_eq!(mode.mode_name(), "auto");
    }

    #[tokio::test]
    async fn use_localhost_default_port() {
        let mode = get_tunnel_mode(TunnelModeFlags {
            tunnel_url: None,
            use_localhost: true,
            localhost_port: None,
        })
        .await
        .unwrap();
        match mode {
            TunnelMode::UseLocalhost {
                requested_port,
                actual_port,
            } => {
                assert_eq!(requested_port, DEFAULT_LOCALHOST_PORT);
                assert_eq!(actual_port, DEFAULT_LOCALHOST_PORT);
            }
            other => panic!("expected UseLocalhost, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn use_localhost_explicit_port() {
        // Bind an ephemeral free port first so we know it's free
        let free = get_available_tcp_port(None).await.unwrap();
        let mode = get_tunnel_mode(TunnelModeFlags {
            tunnel_url: None,
            use_localhost: false,
            localhost_port: Some(free),
        })
        .await
        .unwrap();
        match mode {
            TunnelMode::UseLocalhost {
                requested_port,
                actual_port,
            } => {
                assert_eq!(requested_port, free);
                assert_eq!(actual_port, free);
            }
            other => panic!("expected UseLocalhost, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn unavailable_explicit_port_errors() {
        // Occupy a port
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let err = get_tunnel_mode(TunnelModeFlags {
            tunnel_url: None,
            use_localhost: true,
            localhost_port: Some(port),
        })
        .await
        .unwrap_err();
        assert!(err.to_string().contains("is not available"));
        drop(listener);
    }
}
