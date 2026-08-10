use crate::dev::{allowed_hosts, HotReloadEvent};
use crate::filesystem::ThemeAsset;
use crate::theme_ext::fs::{
    get_extension_in_memory_templates, mount_theme_extension_file_system, ThemeExtFsEventName,
    ThemeExtFsEventPayload, ThemeExtensionFileSystem,
};
use axum::extract::{Path as AxumPath, Request, State};
use axum::http::header::{CONTENT_TYPE, HOST};
use axum::http::{HeaderMap, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{any, get};
use axum::Router;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use futures::Stream;
use percent_encoding::percent_decode_str;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::{broadcast, watch};
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

pub const DEFAULT_THEME_EXT_HOST: &str = "127.0.0.1";
pub const DEFAULT_THEME_EXT_PORT: u16 = 9293;

const HOT_RELOAD_VERSION: &str = "1";

#[derive(Debug, thiserror::Error)]
pub enum ThemeExtServerError {
    #[error("Unable to bind theme extension server to {0}: {1}")]
    Bind(SocketAddr, std::io::Error),
}

#[derive(Debug, Clone)]
pub struct ThemeExtServerContext {
    pub host: String,
    pub port: u16,
    pub directory: PathBuf,
    pub theme_id: i64,
    pub filesystem: ThemeExtensionFileSystem,
}

#[derive(Clone)]
struct AppState {
    ctx: Arc<ThemeExtServerContext>,
    reload_tx: broadcast::Sender<HotReloadEvent>,
}

pub struct ThemeExtServerHandle {
    shutdown_tx: watch::Sender<bool>,
    join: tokio::task::JoinHandle<()>,
}

impl ThemeExtServerHandle {
    pub async fn close(self) {
        let _ = self.shutdown_tx.send(true);
        let _ = self.join.await;
    }
}

/// Builds the extension-server context: mounts FS, forces hot-reload semantics.
pub fn build_theme_extension_context(
    directory: impl Into<PathBuf>,
    theme_id: i64,
    port: Option<u16>,
) -> ThemeExtServerContext {
    let directory = directory.into();
    let filesystem = mount_theme_extension_file_system(&directory);
    filesystem.ready();
    ThemeExtServerContext {
        host: DEFAULT_THEME_EXT_HOST.into(),
        port: port.unwrap_or(DEFAULT_THEME_EXT_PORT),
        directory,
        theme_id,
        filesystem,
    }
}

/// Starts a minimal Axum theme-extension server on `127.0.0.1:9293` by default.
///
/// Reuses host-validation and hot-reload patterns from `dev.rs`. Live reload is
/// unconditional (upstream always sets `liveReload: 'hot-reload'`).
pub async fn run_theme_extension_server(
    ctx: ThemeExtServerContext,
) -> Result<ThemeExtServerHandle, ThemeExtServerError> {
    setup_in_memory_template_watcher(&ctx);

    let addr = SocketAddr::new(
        ctx.host
            .parse()
            .unwrap_or_else(|_| DEFAULT_THEME_EXT_HOST.parse().expect("default host")),
        ctx.port,
    );
    let listener = TcpListener::bind(addr)
        .await
        .map_err(|error| ThemeExtServerError::Bind(addr, error))?;

    let (reload_tx, _) = broadcast::channel(256);
    let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
    let state = AppState {
        ctx: Arc::new(ctx),
        reload_tx,
    };
    let app = build_theme_extension_router(state);

    let join = tokio::spawn(async move {
        let server = axum::serve(listener, app).with_graceful_shutdown(async move {
            let _ = shutdown_rx.changed().await;
        });
        if let Err(error) = server.await {
            eprintln!("Theme extension server error: {error}");
        }
    });

    Ok(ThemeExtServerHandle { shutdown_tx, join })
}

fn build_theme_extension_router(state: AppState) -> Router {
    Router::new()
        .route("/__theme_dev/hot-reload", get(hot_reload))
        .route("/assets/*path", get(asset))
        .fallback(any(fallback))
        .with_state(state)
}

/// Builds a router from a server context (for tests / embedding).
pub fn theme_extension_router_from_context(ctx: ThemeExtServerContext) -> Router {
    let (reload_tx, _) = broadcast::channel(256);
    build_theme_extension_router(AppState {
        ctx: Arc::new(ctx),
        reload_tx,
    })
}

fn setup_in_memory_template_watcher(ctx: &ThemeExtServerContext) {
    let filesystem = ctx.filesystem.clone();

    let on_update = Arc::new(move |_payload: ThemeExtFsEventPayload| {
        // Unsynced keys are tracked by `handle_file_update`; the main theme SFR
        // consumes them via `get_extension_in_memory_templates`.
    });

    let add = Arc::clone(&on_update);
    let change = Arc::clone(&on_update);
    filesystem.add_event_listener(ThemeExtFsEventName::Add, move |payload| add(payload));
    filesystem.add_event_listener(ThemeExtFsEventName::Change, move |payload| change(payload));

    let _ = filesystem.start_watcher();
}

async fn hot_reload(
    State(state): State<AppState>,
) -> Sse<Pin<Box<dyn Stream<Item = Result<Event, Infallible>> + Send>>> {
    let rx = state.reload_tx.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|event| {
        let event = match event {
            Ok(event) => event,
            Err(_) => return None,
        };
        let data = serde_json::to_string(&event).ok()?;
        Some(Ok(Event::default().data(data)))
    });
    let _ = state.reload_tx.send(HotReloadEvent::Open {
        version: HOT_RELOAD_VERSION.into(),
        pid: std::process::id().to_string(),
        theme_id: state.ctx.theme_id.to_string(),
    });
    let stream: Pin<Box<dyn Stream<Item = Result<Event, Infallible>> + Send>> = Box::pin(stream);
    Sse::new(stream).keep_alive(KeepAlive::default())
}

async fn asset(State(state): State<AppState>, AxumPath(path): AxumPath<String>) -> Response {
    let key = format!("assets/{}", decode_path(&path));
    serve_extension_asset(&state.ctx.filesystem, &key)
}

async fn fallback(State(state): State<AppState>, request: Request) -> Response {
    if !valid_host(&state.ctx, request.headers()) {
        return (StatusCode::BAD_REQUEST, "Invalid Host header").into_response();
    }
    if should_ignore(request.uri().path()) {
        return StatusCode::NO_CONTENT.into_response();
    }
    // Extension HTML/proxy paths are stubbed; unsynced templates remain available via
    // `get_extension_in_memory_templates` for the main theme SFR.
    let _ = get_extension_in_memory_templates(&state.ctx.filesystem);
    StatusCode::NO_CONTENT.into_response()
}

fn serve_extension_asset(filesystem: &ThemeExtensionFileSystem, key: &str) -> Response {
    let Some(asset) = filesystem.files().get(key).cloned() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    match asset_bytes(&asset) {
        Some(bytes) => {
            let mime = mime_guess::from_path(key)
                .first_or_octet_stream()
                .essence_str()
                .to_string();
            ([(CONTENT_TYPE, mime)], bytes).into_response()
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

fn asset_bytes(asset: &ThemeAsset) -> Option<Vec<u8>> {
    if let Some(value) = &asset.value {
        return Some(value.as_bytes().to_vec());
    }
    asset
        .attachment
        .as_ref()
        .and_then(|attachment| BASE64_STANDARD.decode(attachment).ok())
}

pub fn valid_extension_host(ctx: &ThemeExtServerContext, headers: &HeaderMap) -> bool {
    valid_host(ctx, headers)
}

fn valid_host(ctx: &ThemeExtServerContext, headers: &HeaderMap) -> bool {
    let Some(host) = headers.get(HOST).and_then(|value| value.to_str().ok()) else {
        return false;
    };
    allowed_hosts(&ctx.host, ctx.port).contains(&normalize_host_header(host))
}

fn normalize_host_header(host: &str) -> String {
    host.to_lowercase().replace(".:", ":")
}

fn should_ignore(path: &str) -> bool {
    [
        "/.well-known",
        "/shopify/monorail",
        "/mini-profiler-resources",
        "/web-pixels-manager",
        "/web-pixels@",
        "/wpm",
        "/services/",
        "/api/collect",
        "/cdn-cgi/challenge-platform",
    ]
    .iter()
    .any(|endpoint| path.starts_with(endpoint))
}

fn decode_path(path: &str) -> String {
    percent_decode_str(path).decode_utf8_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context_at(port: u16) -> ThemeExtServerContext {
        ThemeExtServerContext {
            host: DEFAULT_THEME_EXT_HOST.into(),
            port,
            directory: PathBuf::from("tmp"),
            theme_id: 1,
            filesystem: mount_theme_extension_file_system("tmp"),
        }
    }

    fn host_headers(host: Option<&str>) -> HeaderMap {
        let mut headers = HeaderMap::new();
        if let Some(host) = host {
            headers.insert(HOST, host.parse().unwrap());
        }
        headers
    }

    #[test]
    fn accepts_loopback_hosts() {
        let ctx = context_at(9293);
        for host in [
            "localhost:9293",
            "127.0.0.1:9293",
            "[::1]:9293",
            "LOCALHOST:9293",
            "localhost.:9293",
        ] {
            assert!(
                valid_extension_host(&ctx, &host_headers(Some(host))),
                "host={host}"
            );
        }
    }

    #[test]
    fn rejects_attacker_hosts_and_missing_host() {
        let ctx = context_at(9293);
        for host in [
            "attacker.com:9293",
            "poc.mzero.cloud:9293",
            "localhost:1234",
        ] {
            assert!(
                !valid_extension_host(&ctx, &host_headers(Some(host))),
                "host={host}"
            );
        }
        assert!(!valid_extension_host(&ctx, &host_headers(None)));
    }

    #[test]
    fn accepts_uppercase_bound_host_flag() {
        let mut ctx = context_at(9293);
        ctx.host = "LOCALHOST".into();
        assert!(valid_extension_host(
            &ctx,
            &host_headers(Some("localhost:9293"))
        ));
    }

    #[test]
    fn default_port_is_9293() {
        assert_eq!(DEFAULT_THEME_EXT_PORT, 9293);
        assert_eq!(DEFAULT_THEME_EXT_HOST, "127.0.0.1");
    }

    #[test]
    fn normalize_strips_trailing_dot_before_port() {
        assert_eq!(normalize_host_header("localhost.:9293"), "localhost:9293");
    }

    #[test]
    fn ignored_paths_include_wpm() {
        assert!(should_ignore("/wpm@something"));
        assert!(!should_ignore("/"));
    }
}
