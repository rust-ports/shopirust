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
        let name = web.name.clone().unwrap_or_else(|| {
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

async fn run_web_process(ctx: DevProcessContext, opts: WebProcessOptions) -> Result<(), AppError> {
    if let Some(ref predev) = opts.web.commands.predev {
        run_predev_command(predev, &opts, &ctx).await?;
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

    let mut child = spawn_shell_command(&dev_command, &opts)?;
    pipe_child_output(&mut child, ctx.prefix.clone(), ctx.log.clone());
    tokio::select! {
        _ = ctx.abort.cancelled() => {
            let _ = child.kill().await;
        }
        status = child.wait() => {
            let status = status.map_err(|error| {
                AppError::message(format!("failed to wait for web process: {error}"))
            })?;
            return Err(AppError::message(if status.success() {
                "web process exited unexpectedly".to_string()
            } else {
                format!("web process exited with {:?}", status.code())
            }));
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

async fn run_predev_command(
    script: &str,
    opts: &WebProcessOptions,
    ctx: &DevProcessContext,
) -> Result<(), AppError> {
    let mut child = spawn_shell_command(script, opts)?;
    pipe_child_output(&mut child, ctx.prefix.clone(), ctx.log.clone());
    let status = child
        .wait()
        .await
        .map_err(|error| AppError::message(format!("failed to wait for `{script}`: {error}")))?;
    if !status.success() {
        return Err(AppError::message(format!(
            "web command `{script}` exited with {:?}",
            status.code()
        )));
    }
    Ok(())
}

fn spawn_shell_command(
    script: &str,
    opts: &WebProcessOptions,
) -> Result<tokio::process::Child, AppError> {
    if script.trim().is_empty() {
        return Err(AppError::message("empty web command"));
    }
    let mut command = platform_shell_command(script);
    command
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

#[cfg(windows)]
fn platform_shell_command(script: &str) -> Command {
    let mut command = Command::new("cmd.exe");
    command.args(["/D", "/S", "/C", script]);
    command
}

#[cfg(not(windows))]
fn platform_shell_command(script: &str) -> Command {
    let mut command = Command::new("sh");
    command.args(["-c", script]);
    command
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
    use tempfile::tempdir;
    use tokio_util::sync::CancellationToken;

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

    fn process_options(directory: PathBuf, commands: WebCommands) -> WebProcessOptions {
        WebProcessOptions {
            web: WebInstance {
                directory,
                configuration_path: PathBuf::from("shopify.web.toml"),
                roles: vec!["backend".into()],
                name: Some("backend".into()),
                auth_callback_path: vec![],
                webhooks_path: None,
                port: None,
                commands,
                hmr_server: false,
            },
            proxy_url: "https://example.com".into(),
            port: 3457,
            api_key: "key".into(),
            api_secret: "secret".into(),
            scopes: "write_products".into(),
            frontend_port: 3000,
            backend_port: 3457,
            hmr_port: None,
        }
    }

    fn process_context() -> DevProcessContext {
        let (log, _rx) = tokio::sync::mpsc::unbounded_channel();
        DevProcessContext {
            abort: CancellationToken::new(),
            prefix: "backend".into(),
            log,
        }
    }

    #[cfg(not(windows))]
    const CHAIN_COMMAND: &str = "printf first > first.txt && printf second > second.txt";
    #[cfg(windows)]
    const CHAIN_COMMAND: &str = "echo first>first.txt && echo second>second.txt";

    #[cfg(not(windows))]
    const FAIL_COMMAND: &str = "exit 7";
    #[cfg(windows)]
    const FAIL_COMMAND: &str = "exit /B 7";

    #[cfg(not(windows))]
    const SUCCESS_COMMAND: &str = "exit 0";
    #[cfg(windows)]
    const SUCCESS_COMMAND: &str = "exit /B 0";

    #[tokio::test]
    async fn predev_executes_complete_shell_chain() {
        let directory = tempdir().unwrap();
        let opts = process_options(directory.path().to_path_buf(), WebCommands::default());

        run_predev_command(CHAIN_COMMAND, &opts, &process_context())
            .await
            .unwrap();

        assert_eq!(
            std::fs::read_to_string(directory.path().join("first.txt")).unwrap(),
            "first"
        );
        assert_eq!(
            std::fs::read_to_string(directory.path().join("second.txt")).unwrap(),
            "second"
        );
    }

    #[cfg(not(windows))]
    #[tokio::test]
    async fn shell_preserves_quoted_arguments() {
        let directory = tempdir().unwrap();
        let opts = process_options(directory.path().to_path_buf(), WebCommands::default());

        run_predev_command(
            "printf '%s' 'hello world' > quoted.txt",
            &opts,
            &process_context(),
        )
        .await
        .unwrap();

        assert_eq!(
            std::fs::read_to_string(directory.path().join("quoted.txt")).unwrap(),
            "hello world"
        );
    }

    #[tokio::test]
    async fn predev_nonzero_exit_is_an_error() {
        let directory = tempdir().unwrap();
        let opts = process_options(directory.path().to_path_buf(), WebCommands::default());

        let error = run_predev_command(FAIL_COMMAND, &opts, &process_context())
            .await
            .unwrap_err();

        assert!(error.to_string().contains("exited with"));
    }

    #[tokio::test]
    async fn successful_dev_exit_is_unexpected() {
        let directory = tempdir().unwrap();
        let opts = process_options(
            directory.path().to_path_buf(),
            WebCommands {
                dev: Some(SUCCESS_COMMAND.into()),
                ..Default::default()
            },
        );

        let error = run_web_process(process_context(), opts).await.unwrap_err();

        assert_eq!(error.to_string(), "web process exited unexpectedly");
    }
}
