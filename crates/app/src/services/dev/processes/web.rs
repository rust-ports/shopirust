//! Spawn web (frontend/backend) processes — npm/vite when present, else skip.

use super::types::{DevProcess, DevProcessKind};
use crate::error::AppError;
use crate::models::loader::WebInstance;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::process::Command;

#[derive(Debug, Clone)]
pub struct WebProcessOptions {
    pub web: WebInstance,
    pub proxy_url: String,
    pub port: u16,
    pub api_key: String,
    pub api_secret: String,
    pub scopes: String,
}

/// Build web processes; skips directories without a `package.json` (happy-path stub).
pub fn setup_web_processes(
    webs: &[WebInstance],
    proxy_url: &str,
    frontend_port: u16,
    backend_port: u16,
    api_key: &str,
    api_secret: &str,
    scopes: &str,
) -> Vec<DevProcess> {
    let mut out = Vec::new();
    for (i, web) in webs.iter().enumerate() {
        let is_frontend = web.roles.iter().any(|r| r.eq_ignore_ascii_case("frontend"));
        let port = if is_frontend {
            frontend_port
        } else {
            backend_port.saturating_add(i as u16)
        };
        let name = web.name.clone().unwrap_or_else(|| format!("web-{}", i + 1));
        let opts = WebProcessOptions {
            web: web.clone(),
            proxy_url: proxy_url.to_string(),
            port,
            api_key: api_key.to_string(),
            api_secret: api_secret.to_string(),
            scopes: scopes.to_string(),
        };
        out.push(DevProcess::new(name, DevProcessKind::Web, move |ctx| {
            run_web_process(ctx.abort, opts)
        }));
    }
    out
}

async fn run_web_process(
    abort: tokio_util::sync::CancellationToken,
    opts: WebProcessOptions,
) -> Result<(), AppError> {
    let package_json = opts.web.directory.join("package.json");
    if !package_json.exists() {
        tracing::info!(
            target: "app_dev",
            "skipping web at {} (no package.json)",
            opts.web.directory.display()
        );
        abort.cancelled().await;
        return Ok(());
    }

    let script = detect_dev_script(&package_json).unwrap_or_else(|| "dev".into());
    let mut child = Command::new(npm_cmd())
        .arg("run")
        .arg(&script)
        .current_dir(&opts.web.directory)
        .env("PORT", opts.port.to_string())
        .env("BACKEND_PORT", opts.port.to_string())
        .env("FRONTEND_PORT", opts.port.to_string())
        .env("SHOPIFY_API_KEY", &opts.api_key)
        .env("SHOPIFY_API_SECRET", &opts.api_secret)
        .env("SCOPES", &opts.scopes)
        .env("HOST", &opts.proxy_url)
        .env("APP_URL", &opts.proxy_url)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| AppError::message(format!("failed to spawn npm run {script}: {e}")))?;

    tokio::select! {
        _ = abort.cancelled() => {
            let _ = child.kill().await;
        }
        status = child.wait() => {
            let status = status.map_err(|e| AppError::message(e.to_string()))?;
            if !status.success() && !abort.is_cancelled() {
                return Err(AppError::message(format!(
                    "web process exited with {status}"
                )));
            }
        }
    }
    Ok(())
}

fn npm_cmd() -> &'static str {
    if cfg!(windows) {
        "npm.cmd"
    } else {
        "npm"
    }
}

fn detect_dev_script(package_json: &PathBuf) -> Option<String> {
    let text = std::fs::read_to_string(package_json).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    let scripts = v.get("scripts")?.as_object()?;
    if scripts.contains_key("dev") {
        Some("dev".into())
    } else if scripts.contains_key("start") {
        Some("start".into())
    } else {
        None
    }
}
