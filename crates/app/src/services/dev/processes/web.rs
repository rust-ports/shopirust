//! Spawn web (frontend/backend) processes from `shopify.web.toml` commands.

use super::types::{DevProcess, DevProcessContext, DevProcessKind};
use crate::error::AppError;
use crate::models::loader::WebInstance;
use std::path::Path;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug, Clone)]
pub struct WebProcessOptions {
    pub web: WebInstance,
    pub proxy_url: String,
    pub port: u16,
    pub api_key: String,
    pub api_secret: String,
    pub scopes: String,
    pub frontend_port: u16,
    pub backend_port: u16,
    pub hmr_port: Option<u16>,
}

/// Build web processes from loaded `shopify.web.toml` instances.
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
    let hmr_port = webs.iter().any(|w| w.hmr_server).then_some(frontend_port);
    for (i, web) in webs.iter().enumerate() {
        let is_frontend = web.roles.iter().any(|r| r.eq_ignore_ascii_case("frontend"));
        let port = web.port.unwrap_or(if is_frontend {
            frontend_port
        } else {
            backend_port.saturating_add(i as u16)
        });
        let name = web
            .name
            .clone()
            .unwrap_or_else(|| {
                let mut parts = vec!["web".to_string()];
                parts.extend(web.roles.iter().cloned());
                parts.join("-")
            });
        let opts = WebProcessOptions {
            web: web.clone(),
            proxy_url: proxy_url.to_string(),
            port,
            api_key: api_key.to_string(),
            api_secret: api_secret.to_string(),
            scopes: scopes.to_string(),
            frontend_port,
            backend_port,
            hmr_port: if is_frontend { hmr_port } else { None },
        };
        out.push(DevProcess::new(name, DevProcessKind::Web, move |ctx| {
            run_web_process(ctx, opts)
        }));
    }
    out
}

async fn run_web_process(
    ctx: DevProcessContext,
    opts: WebProcessOptions,
) -> Result<(), AppError> {
    if let Some(ref predev) = opts.web.commands.predev {
        run_command_chain(predev, &opts, false).await?;
    }

    let dev_command = opts
        .web
        .commands
        .dev
        .clone()
        .or_else(|| detect_npm_script(&opts.web.directory.join("package.json")));

    let Some(dev_command) = dev_command else {
        ctx.emit(format!(
            "skipping web at {} (no [commands] dev / package.json script)",
            opts.web.directory.display()
        ));
        ctx.abort.cancelled().await;
        return Ok(());
    };

    let mut child = spawn_command_chain(&dev_command, &opts)?;
    pipe_child_output(&mut child, ctx.prefix.clone(), ctx.log.clone());
    tokio::select! {
        _ = ctx.abort.cancelled() => {
            let _ = child.kill().await;
        }
        status = child.wait() => {
            if let Ok(s) = status {
                if !s.success() {
                    return Err(AppError::message(format!(
                        "web process exited with {:?}",
                        s.code()
                    )));
                }
            }
        }
    }
    Ok(())
}

fn pipe_child_output(
    child: &mut tokio::process::Child,
    prefix: String,
    log: UnboundedSender<(String, String)>,
) {
    if let Some(stdout) = child.stdout.take() {
        spawn_line_pump(prefix.clone(), stdout, log.clone());
    }
    if let Some(stderr) = child.stderr.take() {
        spawn_line_pump(prefix, stderr, log);
    }
}

fn spawn_line_pump(
    prefix: String,
    pipe: impl tokio::io::AsyncRead + Unpin + Send + 'static,
    log: UnboundedSender<(String, String)>,
) {
    tokio::spawn(async move {
        let mut lines = BufReader::new(pipe).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let _ = log.send((prefix.clone(), line));
        }
    });
}

fn spawn_command_chain(
    script: &str,
    opts: &WebProcessOptions,
) -> Result<tokio::process::Child, AppError> {
    // Upstream splits on `&&` and runs sequentially; for the long-running `dev` command
    // we execute the last segment after running the prefixes.
    let parts: Vec<&str> = script.split("&&").map(str::trim).filter(|s| !s.is_empty()).collect();
    if parts.is_empty() {
        return Err(AppError::message("empty web dev command"));
    }
    let last = *parts.last().unwrap();
    spawn_one(last, opts)
}

async fn run_command_chain(
    script: &str,
    opts: &WebProcessOptions,
    inherit: bool,
) -> Result<(), AppError> {
    for part in script.split("&&").map(str::trim).filter(|s| !s.is_empty()) {
        let mut child = spawn_one(part, opts)?;
        if !inherit {
            // predev: capture, don't steal TTY
        }
        let status = child.wait().await.map_err(|e| AppError::message(e.to_string()))?;
        if !status.success() {
            return Err(AppError::message(format!(
                "web command `{part}` exited with {:?}",
                status.code()
            )));
        }
    }
    Ok(())
}

fn spawn_one(script: &str, opts: &WebProcessOptions) -> Result<tokio::process::Child, AppError> {
    let mut parts = script.split_whitespace();
    let cmd = parts.next().ok_or_else(|| AppError::message("empty command"))?;
    let args: Vec<&str> = parts.collect();
    let mut command = Command::new(cmd);
    command
        .args(&args)
        .current_dir(&opts.web.directory)
        .env("PORT", opts.port.to_string())
        .env("SERVER_PORT", opts.port.to_string())
        .env("BACKEND_PORT", opts.backend_port.to_string())
        .env("FRONTEND_PORT", opts.frontend_port.to_string())
        .env("SHOPIFY_API_KEY", &opts.api_key)
        .env("SHOPIFY_API_SECRET", &opts.api_secret)
        .env("SCOPES", &opts.scopes)
        .env("HOST", &opts.proxy_url)
        .env("APP_URL", &opts.proxy_url)
        .env("APP_ENV", "development")
        .env("NODE_ENV", "development")
        .env("REMIX_DEV_ORIGIN", &opts.proxy_url)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    if let Some(hmr) = opts.hmr_port {
        command.env("HMR_SERVER_PORT", hmr.to_string());
    }
    command
        .spawn()
        .map_err(|e| AppError::message(format!("Failed to start `{script}`: {e}")))
}

fn detect_npm_script(package_json: &Path) -> Option<String> {
    let raw = std::fs::read_to_string(package_json).ok()?;
    let v: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let scripts = v.get("scripts")?.as_object()?;
    if scripts.contains_key("dev") {
        return Some(format!("{} run dev", npm_cmd()));
    }
    if scripts.contains_key("start") {
        return Some(format!("{} run start", npm_cmd()));
    }
    None
}

fn npm_cmd() -> &'static str {
    if cfg!(windows) {
        "npm.cmd"
    } else {
        "npm"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::loader::{parse_web_commands, WebCommands, WebInstance};
    use std::path::PathBuf;

    fn web(role: &str, commands: WebCommands) -> WebInstance {
        WebInstance {
            directory: PathBuf::from("/app/web"),
            configuration_path: PathBuf::from("/app/web/shopify.web.toml"),
            roles: vec![role.into()],
            name: Some(role.into()),
            auth_callback_path: vec![],
            webhooks_path: None,
            port: None,
            commands,
            hmr_server: false,
        }
    }

    #[test]
    fn creates_one_process_per_web() {
        let procs = setup_web_processes(
            &[
                web(
                    "frontend",
                    WebCommands {
                        dev: Some("npm run dev".into()),
                        ..Default::default()
                    },
                ),
                web(
                    "backend",
                    WebCommands {
                        dev: Some("npm run start".into()),
                        ..Default::default()
                    },
                ),
            ],
            "https://example.trycloudflare.com",
            3000,
            3457,
            "key",
            "sec",
            "write_products",
        );
        assert_eq!(procs.len(), 2);
        assert!(procs.iter().all(|p| p.kind == DevProcessKind::Web));
    }

    #[test]
    fn parse_commands_from_toml() {
        let value: toml::Value = toml::from_str(
            r#"
            roles = ["frontend"]
            [commands]
            dev = "npm run dev"
            predev = "npm run setup"
            "#,
        )
        .unwrap();
        let cmds = parse_web_commands(&value);
        assert_eq!(cmds.dev.as_deref(), Some("npm run dev"));
        assert_eq!(cmds.predev.as_deref(), Some("npm run setup"));
    }
}
