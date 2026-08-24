//! In-memory tunnel for unit tests.

use super::{TunnelClient, TunnelError, TunnelStatus};
use async_trait::async_trait;

pub struct FakeTunnel {
    port: u16,
    url: String,
    status: TunnelStatus,
    fail_start: bool,
}

impl FakeTunnel {
    pub fn new(port: u16) -> Self {
        Self {
            port,
            url: format!("https://fake-tunnel.example:{port}"),
            status: TunnelStatus::NotStarted,
            fail_start: false,
        }
    }

    pub fn with_url(mut self, url: impl Into<String>) -> Self {
        self.url = url.into();
        self
    }

    pub fn failing(mut self) -> Self {
        self.fail_start = true;
        self
    }
}

#[async_trait]
impl TunnelClient for FakeTunnel {
    fn provider(&self) -> &str {
        "fake"
    }

    fn port(&self) -> u16 {
        self.port
    }

    async fn start(&mut self) -> Result<(), TunnelError> {
        self.status = TunnelStatus::Starting;
        if self.fail_start {
            let msg = "fake tunnel start failed".to_string();
            self.status = TunnelStatus::Error {
                message: msg.clone(),
            };
            return Err(TunnelError::message(msg));
        }
        self.status = TunnelStatus::Connected {
            url: self.url.clone(),
        };
        Ok(())
    }

    fn get_url(&self) -> Option<String> {
        match &self.status {
            TunnelStatus::Connected { url } => Some(url.clone()),
            _ => None,
        }
    }

    fn status(&self) -> TunnelStatus {
        self.status.clone()
    }

    async fn stop(&mut self) {
        self.status = TunnelStatus::NotStarted;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn fake_tunnel_connects() {
        let mut t = FakeTunnel::new(9999);
        assert!(matches!(t.status(), TunnelStatus::NotStarted));
        t.start().await.unwrap();
        assert_eq!(
            t.get_url().as_deref(),
            Some("https://fake-tunnel.example:9999")
        );
        t.stop().await;
        assert!(matches!(t.status(), TunnelStatus::NotStarted));
    }
}
