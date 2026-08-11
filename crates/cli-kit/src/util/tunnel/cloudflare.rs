//! Cloudflare quick tunnel via `cloudflared` on PATH.

use super::{TunnelClient, TunnelError, TunnelStatus};
use async_trait::async_trait;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::oneshot;
use tokio::time::timeout;

const TUNNEL_TIMEOUT_SECS: u64 = 40;
const DEFAULT_DOMAIN: &str = "trycloudflare.com";

pub struct CloudflareTunnel {
    port: u16,
    status: Arc<Mutex<TunnelStatus>>,
    child: Option<Child>,
}

impl CloudflareTunnel {
    pub fn new(port: u16) -> Self {
        Self {
            port,
            status: Arc::new(Mutex::new(TunnelStatus::NotStarted)),
            child: None,
        }
    }

    fn bin_path() -> Result<String, TunnelError> {
        if let Ok(path) = std::env::var("SHOPIFY_CLI_CLOUDFLARED_PATH") {
            if !path.is_empty() {
                return Ok(path);
            }
        }
        which_cloudflared().ok_or(TunnelError::CloudflaredMissing)
    }

    fn tunnel_domain() -> String {
        std::env::var("SHOPIFY_CLI_CLOUDFLARED_DOMAIN").unwrap_or_else(|_| DEFAULT_DOMAIN.into())
    }
}

fn which_cloudflared() -> Option<String> {
    let candidates = ["cloudflared", "cloudflared.exe"];
    for name in candidates {
        if let Ok(output) = std::process::Command::new("which").arg(name).output() {
            if output.status.success() {
                let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !path.is_empty() {
                    return Some(path);
                }
            }
        }
        if std::process::Command::new(name)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
        {
            return Some(name.to_string());
        }
    }
    None
}

fn find_url(line: &str, domain: &str) -> Option<String> {
    let domain_dot = format!(".{domain}");
    for part in line.split_whitespace() {
        if part.starts_with("https://") && part.contains(&domain_dot) {
            return Some(
                part.trim_matches(|c: char| !c.is_ascii_graphic())
                    .to_string(),
            );
        }
    }
    if let Some(start) = line.find("https://") {
        let rest = &line[start..];
        let end = rest
            .find(|c: char| c.is_whitespace() || c == '\'' || c == '"')
            .unwrap_or(rest.len());
        let url = &rest[..end];
        if url.contains(&domain_dot) {
            return Some(url.to_string());
        }
    }
    None
}

fn find_connection(line: &str) -> bool {
    line.contains("Registered tunnel connection") || line.contains("INF Connection")
}

fn spawn_reader<R>(
    stream: Option<R>,
    status: Arc<Mutex<TunnelStatus>>,
    tx: Arc<Mutex<Option<oneshot::Sender<Result<String, TunnelError>>>>>,
    domain: String,
) where
    R: AsyncRead + Unpin + Send + 'static,
{
    let Some(stream) = stream else {
        return;
    };
    tokio::spawn(async move {
        let mut lines = BufReader::new(stream).lines();
        let mut url: Option<String> = None;
        let mut connected = false;
        while let Ok(Some(line)) = lines.next_line().await {
            tracing::debug!(target: "tunnel", "{line}");
            if url.is_none() {
                url = find_url(&line, &domain);
            }
            if find_connection(&line) {
                connected = true;
            }
            if connected {
                if let Some(ref u) = url {
                    let mut guard = tx.lock().unwrap();
                    if let Some(sender) = guard.take() {
                        let _ = sender.send(Ok(u.clone()));
                    }
                    *status.lock().unwrap() = TunnelStatus::Connected { url: u.clone() };
                    return;
                }
            }
        }
    });
}

#[async_trait]
impl TunnelClient for CloudflareTunnel {
    fn provider(&self) -> &str {
        "cloudflare"
    }

    fn port(&self) -> u16 {
        self.port
    }

    async fn start(&mut self) -> Result<(), TunnelError> {
        {
            let st = self.status.lock().unwrap();
            if matches!(*st, TunnelStatus::Connected { .. }) {
                return Ok(());
            }
        }
        *self.status.lock().unwrap() = TunnelStatus::Starting;

        let bin = Self::bin_path()?;
        let domain = Self::tunnel_domain();
        let url_arg = format!("http://localhost:{}", self.port);
        let args = ["tunnel", "--url", &url_arg, "--no-autoupdate"];

        let mut child = Command::new(&bin)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| {
                TunnelError::message(format!(
                    "failed to spawn cloudflared ({bin}): {e}. Install cloudflared or set SHOPIFY_CLI_CLOUDFLARED_PATH."
                ))
            })?;

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let status = self.status.clone();
        let (tx, rx) = oneshot::channel::<Result<String, TunnelError>>();
        let tx = Arc::new(Mutex::new(Some(tx)));

        spawn_reader(stdout, status.clone(), tx.clone(), domain.clone());
        spawn_reader(stderr, status.clone(), tx, domain);

        match timeout(Duration::from_secs(TUNNEL_TIMEOUT_SECS), rx).await {
            Ok(Ok(Ok(url))) => {
                *self.status.lock().unwrap() = TunnelStatus::Connected { url };
                self.child = Some(child);
                Ok(())
            }
            Ok(Ok(Err(e))) => {
                let _ = child.kill().await;
                *self.status.lock().unwrap() = TunnelStatus::Error {
                    message: e.to_string(),
                };
                Err(e)
            }
            Ok(Err(_)) => {
                let _ = child.kill().await;
                let msg = "tunnel channel closed before URL".to_string();
                *self.status.lock().unwrap() = TunnelStatus::Error {
                    message: msg.clone(),
                };
                Err(TunnelError::message(msg))
            }
            Err(_) => {
                let _ = child.kill().await;
                *self.status.lock().unwrap() = TunnelStatus::Error {
                    message: "timeout".into(),
                };
                Err(TunnelError::Timeout)
            }
        }
    }

    fn get_url(&self) -> Option<String> {
        match &*self.status.lock().unwrap() {
            TunnelStatus::Connected { url } => Some(url.clone()),
            _ => None,
        }
    }

    fn status(&self) -> TunnelStatus {
        self.status.lock().unwrap().clone()
    }

    async fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill().await;
            let _ = child.wait().await;
        }
        *self.status.lock().unwrap() = TunnelStatus::NotStarted;
    }
}
