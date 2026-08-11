//! Tunnel client trait + Cloudflare / fake implementations for `app dev`.
//!
//! Install `cloudflared` on PATH (or set `SHOPIFY_CLI_CLOUDFLARED_PATH`) for Auto mode:
//! <https://developers.cloudflare.com/cloudflare-one/connections/connect-apps/install-and-setup/installation/>

mod cloudflare;
mod fake;

pub use cloudflare::CloudflareTunnel;
pub use fake::FakeTunnel;

/// Re-export app-side tunnel mode resolution (`Auto` / `UseLocalhost` / `Custom`).
pub use app::services::{get_tunnel_mode, TunnelMode, DEFAULT_LOCALHOST_PORT};

use async_trait::async_trait;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TunnelStatus {
    NotStarted,
    Starting,
    Connected { url: String },
    Error { message: String },
}

#[derive(Debug, Error)]
pub enum TunnelError {
    #[error("{0}")]
    Message(String),
    #[error(
        "cloudflared not found on PATH (set SHOPIFY_CLI_CLOUDFLARED_PATH or install cloudflared)"
    )]
    CloudflaredMissing,
    #[error("tunnel timed out waiting for URL")]
    Timeout,
}

impl TunnelError {
    pub fn message(msg: impl Into<String>) -> Self {
        Self::Message(msg.into())
    }
}

/// Provider-agnostic tunnel used by `app dev`.
#[async_trait]
pub trait TunnelClient: Send {
    /// Provider name (e.g. `"cloudflare"`).
    fn provider(&self) -> &str;

    /// Local port the tunnel forwards to.
    fn port(&self) -> u16;

    /// Start the tunnel (idempotent if already connected).
    async fn start(&mut self) -> Result<(), TunnelError>;

    /// Public URL once connected.
    fn get_url(&self) -> Option<String>;

    /// Current status snapshot.
    fn status(&self) -> TunnelStatus;

    /// Stop the tunnel process / release resources.
    async fn stop(&mut self);
}
