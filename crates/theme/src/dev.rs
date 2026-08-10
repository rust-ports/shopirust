use crate::checksum::Checksum;
use crate::filesystem::{read_theme_asset, ThemeAsset, ThemeFileSystem, ThemeFsError};
use crate::ignore::{apply_ignore_filters, IgnoreFilters};
use crate::sync::{self, SyncError, SyncOptions, ThemeSyncAdmin};
use crate::utilities::notifier::Notifier;
use crate::watcher::{normalize_event, start_watcher, ThemeFileEvent, ThemeWatchState};
use axum::body::Body;
use axum::extract::{Path as AxumPath, Query, Request, State};
use axum::http::header::{ACCEPT, CONTENT_LENGTH, CONTENT_TYPE, HOST, LOCATION};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, Uri};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{any, get};
use axum::Router;
use colored::Colorize;
use futures::future::BoxFuture;
use futures::Stream;
use percent_encoding::percent_decode_str;
use regex_lite::Regex;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::convert::Infallible;
use std::net::{IpAddr, SocketAddr, TcpListener};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::net::TcpListener as TokioTcpListener;
use tokio::sync::{broadcast, mpsc, watch};
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;
use tower_http::cors::{AllowOrigin, CorsLayer};
use url::Url;

pub const DEFAULT_DEV_HOST: &str = "127.0.0.1";
pub const DEFAULT_DEV_PORT: u16 = 9292;
const HOT_RELOAD_VERSION: &str = "1";
const HOT_RELOAD_SCRIPT_ID: &str = "hot-reload-client";
const HOT_RELOAD_SCRIPT_URL: &str = "/cdn/shopifycloud/theme-hot-reload/theme-hot-reload.js";
const LOCAL_HOT_RELOAD_SCRIPT_ENDPOINT: &str = "/@shopify/theme-hot-reload";
const STANDARD_EVENTS_RUNTIME_URL: &str = "https://cdn.shopify.com/storefront/standard-events.js";
const STANDARD_EVENTS_RUNTIME_DEV_URL: &str =
    "https://cdn.shopify.com/storefront/standard-events.dev.js";
const STANDARD_EVENTS_INSPECTOR_URL: &str =
    "https://cdn.shopify.com/storefront/standard-events-inspector.js";
const STANDARD_EVENTS_INSPECTOR_SCRIPT_ID: &str = "shopify-cli-standard-events-inspector";
const THEME_EDITOR_POLLING_INTERVAL: Duration = Duration::from_secs(3);
const MAX_THEME_EDITOR_POLLING_FAILURES: usize = 5;
const MAX_THEME_ID_MISMATCH_REDIRECTS: usize = 5;
const REQUEST_LOG_PATH_TRUNCATION_LIMIT: usize = 80;
const POLARIS_STYLESHEET_URL: &str =
    "https://unpkg.com/@shopify/polaris@13.9.2/build/esm/styles.css";
pub const EXTENSION_CDN_PREFIX: &str = "/ext/cdn/";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DevServerKind {
    #[default]
    Theme,
    ThemeExtension,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorPageError {
    pub message: String,
    pub code: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiveReloadMode {
    HotReload,
    FullPage,
    Off,
}

impl LiveReloadMode {
    pub fn parse(value: &str) -> Result<Self, DevError> {
        match value {
            "hot-reload" => Ok(Self::HotReload),
            "full-page" => Ok(Self::FullPage),
            "off" => Ok(Self::Off),
            _ => Err(DevError::InvalidOption {
                flag: "--live-reload",
                value: value.into(),
                allowed: "hot-reload, full-page, off",
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorOverlayMode {
    Default,
    Silent,
}

impl ErrorOverlayMode {
    pub fn parse(value: &str) -> Result<Self, DevError> {
        match value {
            "default" => Ok(Self::Default),
            "silent" => Ok(Self::Silent),
            _ => Err(DevError::InvalidOption {
                flag: "--error-overlay",
                value: value.into(),
                allowed: "default, silent",
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DevServerOptions {
    pub root: PathBuf,
    pub host: String,
    pub port: u16,
    pub explicit_port: bool,
    pub live_reload: LiveReloadMode,
    pub error_overlay: ErrorOverlayMode,
    pub poll: bool,
    pub theme_editor_sync: bool,
    pub standard_events_dev_bundle: bool,
    pub standard_events_inspector: bool,
    pub nodelete: bool,
    pub filters: IgnoreFilters,
    pub notify: Option<String>,
    pub store_password: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DevServerSession {
    pub store_fqdn: String,
    pub admin_token: String,
    pub storefront_token: Option<String>,
    pub theme_access_domain: Option<String>,
    pub session_cookies: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DevServerTheme {
    pub id: i64,
    pub name: String,
    pub role: String,
}

/// Re-authenticates the dev server session on demand (theme-ID mismatch recovery).
pub type DevServerRefresh =
    Arc<dyn Fn() -> BoxFuture<'static, Result<DevServerSession, String>> + Send + Sync>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DevServerUrls {
    pub local: String,
    pub preview: String,
    pub editor: String,
    pub gift_card: String,
}

#[derive(Debug, Clone)]
pub struct DevServerContext {
    pub options: DevServerOptions,
    pub session: DevServerSession,
    pub theme: DevServerTheme,
    pub kind: DevServerKind,
}

#[derive(Debug, Clone)]
pub struct DevServerHandle {
    pub urls: DevServerUrls,
}

pub struct DevServerRuntime {
    pub refresh_rx: Option<mpsc::Receiver<Result<DevServerSession, String>>>,
    pub refresh: Option<DevServerRefresh>,
    pub terminal_controls: bool,
}

impl std::fmt::Debug for DevServerRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DevServerRuntime")
            .field("refresh_rx", &self.refresh_rx.is_some())
            .field("refresh", &self.refresh.is_some())
            .field("terminal_controls", &self.terminal_controls)
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReconciliationChoice {
    Remote,
    Local,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct JsonReconciliationDiff {
    pub local_only: Vec<Checksum>,
    pub remote_only: Vec<Checksum>,
    pub conflicts: Vec<Checksum>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct JsonReconciliationPlan {
    pub local_files_to_delete: Vec<String>,
    pub files_to_download: Vec<String>,
    pub remote_files_to_delete: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type")]
pub enum HotReloadEvent {
    #[serde(rename = "open", rename_all = "camelCase")]
    Open {
        version: String,
        pid: String,
        theme_id: String,
    },
    #[serde(rename = "full", rename_all = "camelCase")]
    Full {
        version: String,
        theme_id: String,
        key: String,
    },
    #[serde(rename = "update", rename_all = "camelCase")]
    Update {
        version: String,
        sync: String,
        theme_id: String,
        key: String,
        payload: HotReloadPayload,
    },
    #[serde(rename = "delete", rename_all = "camelCase")]
    Delete {
        version: String,
        sync: String,
        theme_id: String,
        key: String,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HotReloadPayload {
    pub section_names: Vec<String>,
    pub replace_templates: BTreeMap<String, String>,
    pub updated_file_parts: Option<UpdatedFileParts>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatedFileParts {
    pub stylesheet_tag: bool,
    pub javascript_tag: bool,
    pub schema_tag: bool,
    pub liquid: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileDetailsEntry {
    checksum: String,
    liquid: String,
    stylesheet_tag: String,
    javascript_tag: String,
    schema_tag: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RemoteReconciliationPlan {
    pub download: Vec<String>,
    pub keep_local: Vec<String>,
    pub conflicts: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum DevError {
    #[error("{flag} must be one of {allowed}, got '{value}'")]
    InvalidOption {
        flag: &'static str,
        value: String,
        allowed: &'static str,
    },
    #[error("--host cannot be empty")]
    EmptyHost,
    #[error("Invalid --host value: {0}")]
    InvalidHost(String),
    #[error("Port {0} is not available")]
    PortUnavailable(u16),
    #[error("Unable to bind dev server to {0}: {1}")]
    Bind(SocketAddr, std::io::Error),
    #[error(transparent)]
    FileSystem(#[from] ThemeFsError),
    #[error(transparent)]
    Sync(#[from] SyncError),
    #[error("File watcher failed: {0}")]
    Watch(String),
    #[error("Dev server failed: {0}")]
    Server(String),
    #[error("Request failed: Hostname mismatch. Expected host: {expected}. Resulting URL hostname: {actual}")]
    HostnameMismatch { expected: String, actual: String },
    #[error("{0}")]
    Reconciliation(String),
    #[error("Too many polling errors. Please check the errors above and ensure you have a stable internet connection.")]
    PollingFailed,
}

pub fn validate_host(host: &str) -> Result<String, DevError> {
    let host = host.trim();
    if host.is_empty() {
        return Err(DevError::EmptyHost);
    }
    host.parse::<IpAddr>()
        .map(|_| host.to_string())
        .or_else(|_| {
            if host == "localhost"
                || host
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '.')
            {
                Ok(host.to_string())
            } else {
                Err(DevError::InvalidHost(host.into()))
            }
        })
}

pub fn resolve_port(host: &str, requested: Option<u16>) -> Result<u16, DevError> {
    if let Some(port) = requested {
        if port_available(host, port) {
            return Ok(port);
        }
        return Err(DevError::PortUnavailable(port));
    }

    for port in DEFAULT_DEV_PORT..DEFAULT_DEV_PORT + 100 {
        if port_available(host, port) {
            return Ok(port);
        }
    }
    Err(DevError::PortUnavailable(DEFAULT_DEV_PORT))
}

fn port_available(host: &str, port: u16) -> bool {
    TcpListener::bind((host, port)).is_ok()
}

pub fn build_urls(ctx: &DevServerContext) -> DevServerUrls {
    let local = format!("http://{}:{}", ctx.options.host, ctx.options.port);
    DevServerUrls {
        local: local.clone(),
        preview: if ctx.theme.role == "live" {
            format!("https://{}", ctx.session.store_fqdn)
        } else {
            format!(
                "https://{}?preview_theme_id={}",
                ctx.session.store_fqdn, ctx.theme.id
            )
        },
        editor: format!(
            "https://{}/admin/themes/{}/editor?hr={}",
            ctx.session.store_fqdn, ctx.theme.id, ctx.options.port
        ),
        gift_card: format!("{local}/gift_cards/[store_id]/preview"),
    }
}

/// Banner lines shown after a successful `theme dev` sync (keypress hints).
pub fn render_dev_links(urls: &DevServerUrls) -> Vec<String> {
    vec![
        format!("Preview your theme (t)\n  {}", urls.local),
        format!("Share your theme (p)\n  {}", urls.preview),
        format!("Customize your theme (e)\n  {}", urls.editor),
        format!("Preview your gift cards (g)\n  {}", urls.gift_card),
    ]
}

pub async fn run_dev_server<A>(
    api: &A,
    ctx: DevServerContext,
    mut filesystem: ThemeFileSystem,
    runtime: DevServerRuntime,
) -> Result<DevServerHandle, DevError>
where
    A: ThemeSyncAdmin + Sync,
{
    let urls = build_urls(&ctx);
    let reload_tx = broadcast::channel(256).0;
    let session = Arc::new(Mutex::new(ctx.session.clone()));
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let watch_state = ThemeWatchState::default();
    let state = AppState {
        ctx: Arc::new(ctx.clone()),
        session: session.clone(),
        files: Arc::new(Mutex::new(filesystem.files.clone())),
        watch: watch_state.clone(),
        last_requested_path: Arc::new(Mutex::new(String::new())),
        reload_tx: reload_tx.clone(),
        client: reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|error| DevError::Server(error.to_string()))?,
        refresh: runtime.refresh.clone(),
        theme_id_mismatch_redirects: Arc::new(AtomicUsize::new(0)),
        section_names_by_file: Arc::new(Mutex::new(BTreeMap::new())),
        file_details_cache: Arc::new(Mutex::new(BTreeMap::new())),
        extension_files: Arc::new(Mutex::new(BTreeMap::new())),
        extension_unsynced: Arc::new(Mutex::new(BTreeSet::new())),
    };

    let listener = TokioTcpListener::bind((ctx.options.host.as_str(), ctx.options.port))
        .await
        .map_err(|error| DevError::Bind(socket_addr(&ctx.options.host, ctx.options.port), error))?;
    let app = router(state.clone());
    let (watch_tx, mut watch_rx) = mpsc::channel(256);
    let _watcher = start_watcher(&ctx.options.root, ctx.options.poll, watch_tx)
        .map_err(|error| DevError::Watch(error.to_string()))?;
    let notifier = ctx.options.notify.as_ref().map(Notifier::new);
    let mut refresh_rx = runtime.refresh_rx;
    let terminal_task = runtime.terminal_controls.then(|| {
        start_terminal_controls(
            urls.clone(),
            state.last_requested_path.clone(),
            shutdown_tx.clone(),
        )
    });
    let mut poll_interval = tokio::time::interval(THEME_EDITOR_POLLING_INTERVAL);
    let mut remote_checksums = if ctx.options.theme_editor_sync {
        Some(api.fetch_checksums(ctx.theme.id).await?)
    } else {
        None
    };
    let mut polling_failures = 0usize;

    let server = axum::serve(listener, app).with_graceful_shutdown(async move {
        let mut shutdown_rx = shutdown_rx;
        let _ = shutdown_rx.changed().await;
    });
    let mut server = tokio::spawn(async move { server.await });

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                let _ = shutdown_tx.send(true);
            }
            result = &mut server => {
                result
                    .map_err(|error| DevError::Server(error.to_string()))?
                    .map_err(|error| DevError::Server(error.to_string()))?;
                break;
            }
            _ = poll_interval.tick(), if ctx.options.theme_editor_sync => {
                let previous = remote_checksums.clone().unwrap_or_default();
                match poll_theme_editor_changes(api, &ctx, &mut filesystem, &state, previous).await {
                    Ok(latest) => {
                        remote_checksums = Some(latest);
                        polling_failures = 0;
                    }
                    Err(error) => {
                        polling_failures += 1;
                        eprintln!("Error while polling for changes: {error}");
                        if polling_failures >= MAX_THEME_EDITOR_POLLING_FAILURES {
                            let _ = shutdown_tx.send(true);
                            return Err(DevError::PollingFailed);
                        }
                    }
                }
            }
            Some(refresh) = async {
                match refresh_rx.as_mut() {
                    Some(rx) => rx.recv().await,
                    None => None,
                }
            }, if refresh_rx.is_some() => {
                match refresh {
                    Ok(new_session) => {
                        *session.lock().expect("session state poisoned") = new_session;
                    }
                    Err(error) => {
                        eprintln!("Session could not be refreshed: {error}");
                    }
                }
            }
            Some(event) = watch_rx.recv() => {
                if let Some(event) = normalize_event(&ctx.options.root, event) {
                    let events = apply_ignore_filters(vec![event], &ctx.options.filters);
                    if events.is_empty() {
                        continue;
                    }
                    for event in events {
                        handle_file_event(api, &ctx, &mut filesystem, &state, notifier.as_ref(), event).await?;
                    }
                }
            }
        }
    }

    if let Some(task) = terminal_task {
        let _ = tokio::time::timeout(Duration::from_secs(1), task).await;
    }
    Ok(DevServerHandle { urls })
}

async fn handle_file_event<A>(
    api: &A,
    ctx: &DevServerContext,
    filesystem: &mut ThemeFileSystem,
    state: &AppState,
    notifier: Option<&Notifier>,
    event: ThemeFileEvent,
) -> Result<(), DevError>
where
    A: ThemeSyncAdmin + Sync,
{
    use crate::sync::FileOperation;
    use crate::watcher::ThemeFileEventKind;

    match event.kind {
        ThemeFileEventKind::CreateOrUpdate => {
            let Some(asset) = read_theme_asset(&ctx.options.root, &event.key)
                .map_err(|error| DevError::Watch(error.to_string()))?
            else {
                return Ok(());
            };
            state.watch.mark_unsynced(event.key.clone());
            filesystem.files.insert(event.key.clone(), asset.clone());
            state
                .files
                .lock()
                .expect("file state poisoned")
                .insert(event.key.clone(), asset.clone());
            if asset.key.ends_with(".json") && !ctx.options.theme_editor_sync {
                if let Some(content) = asset.value.as_deref() {
                    save_sections_from_json(
                        &mut state
                            .section_names_by_file
                            .lock()
                            .expect("section names poisoned"),
                        &asset.key,
                        content,
                    );
                }
            }
            let payload = hot_reload_payload_for_state(state, &event.key, Some(&asset));
            emit_reload(
                ctx,
                &state.reload_tx,
                &event.key,
                false,
                "local",
                payload.clone(),
            );
            let results = api
                .upload_assets(ctx.theme.id, vec![asset.clone()])
                .await
                .map_err(|error| DevError::Watch(error.to_string()))?;
            remember_watch_upload_errors(&state.watch, results, FileOperation::Upload);
            state.watch.mark_synced(&event.key);
            if let Some(notifier) = notifier {
                let _ = notifier.notify(&event.key).await;
            }
            emit_reload(ctx, &state.reload_tx, &event.key, false, "remote", payload);
        }
        ThemeFileEventKind::Delete => {
            state.watch.mark_unsynced(event.key.clone());
            filesystem.files.remove(&event.key);
            state
                .files
                .lock()
                .expect("file state poisoned")
                .remove(&event.key);
            if event.key.ends_with(".json") && !ctx.options.theme_editor_sync {
                state
                    .section_names_by_file
                    .lock()
                    .expect("section names poisoned")
                    .remove(&event.key);
            }
            state
                .file_details_cache
                .lock()
                .expect("file details poisoned")
                .remove(&event.key);
            emit_reload(
                ctx,
                &state.reload_tx,
                &event.key,
                true,
                "local",
                HotReloadPayload::default(),
            );
            if !ctx.options.nodelete {
                let results = api
                    .delete_assets(ctx.theme.id, vec![event.key.clone()])
                    .await
                    .map_err(|error| DevError::Watch(error.to_string()))?;
                remember_watch_upload_errors(&state.watch, results, FileOperation::Delete);
            }
            state.watch.mark_synced(&event.key);
            if let Some(notifier) = notifier {
                let _ = notifier.notify(&event.key).await;
            }
            emit_reload(
                ctx,
                &state.reload_tx,
                &event.key,
                true,
                "remote",
                HotReloadPayload::default(),
            );
        }
    }
    Ok(())
}

fn remember_watch_upload_errors(
    watch: &ThemeWatchState,
    results: Vec<crate::sync::RemoteResult>,
    operation: crate::sync::FileOperation,
) {
    watch.remember_upload_errors(results, operation);
}

fn hot_reload_payload_for_state(
    state: &AppState,
    key: &str,
    asset: Option<&ThemeAsset>,
) -> HotReloadPayload {
    let files = state.files.lock().expect("file state poisoned");
    let section_names = state
        .section_names_by_file
        .lock()
        .expect("section names poisoned");
    let unsynced = state.watch.unsynced_file_keys();
    let mut cache = state
        .file_details_cache
        .lock()
        .expect("file details poisoned");
    hot_reload_payload(key, asset, &section_names, &files, &unsynced, &mut cache)
}

fn emit_reload(
    ctx: &DevServerContext,
    tx: &broadcast::Sender<HotReloadEvent>,
    key: &str,
    deleted: bool,
    sync: &str,
    payload: HotReloadPayload,
) {
    if ctx.options.live_reload == LiveReloadMode::Off {
        return;
    }
    let theme_id = ctx.theme.id.to_string();
    if ctx.options.live_reload == LiveReloadMode::FullPage || requires_full_page_reload(key) {
        if sync == "remote" || ctx.options.live_reload == LiveReloadMode::FullPage {
            let _ = tx.send(HotReloadEvent::Full {
                version: HOT_RELOAD_VERSION.into(),
                theme_id,
                key: key.into(),
            });
        }
        return;
    }
    let event = if deleted {
        HotReloadEvent::Delete {
            version: HOT_RELOAD_VERSION.into(),
            sync: sync.into(),
            theme_id,
            key: key.into(),
        }
    } else {
        HotReloadEvent::Update {
            version: HOT_RELOAD_VERSION.into(),
            sync: sync.into(),
            theme_id,
            key: key.into(),
            payload,
        }
    };
    let _ = tx.send(event);
}

pub fn requires_full_page_reload(key: &str) -> bool {
    key.starts_with("layout/")
        || key == "config/settings_schema.json"
        || key == "config/settings_data.json"
        || key.starts_with("locales/")
}

pub fn hot_reload_payload(
    key: &str,
    asset: Option<&ThemeAsset>,
    section_names_by_file: &BTreeMap<String, Vec<(String, String)>>,
    files: &BTreeMap<String, ThemeAsset>,
    unsynced: &BTreeSet<String>,
    file_details_cache: &mut BTreeMap<String, FileDetailsEntry>,
) -> HotReloadPayload {
    HotReloadPayload {
        section_names: if key.starts_with("sections/") {
            find_section_names_to_reload(key, files, section_names_by_file)
        } else {
            Vec::new()
        },
        replace_templates: if needs_template_update(key) {
            get_in_memory_templates(files, unsynced, None, None)
        } else {
            BTreeMap::new()
        },
        updated_file_parts: asset.and_then(|asset| updated_file_parts(asset, file_details_cache)),
    }
}

fn needs_template_update(key: &str) -> bool {
    !key.starts_with("assets/") && (key.ends_with(".liquid") || key.ends_with(".json"))
}

fn updated_file_parts(
    asset: &ThemeAsset,
    cache: &mut BTreeMap<String, FileDetailsEntry>,
) -> Option<UpdatedFileParts> {
    let key = asset.key.as_str();
    let valid = ["sections/", "snippets/", "blocks/"]
        .iter()
        .any(|prefix| key.starts_with(prefix))
        && key.ends_with(".liquid");
    if !valid {
        return None;
    }
    let mut result = UpdatedFileParts::default();
    let cached = cache.get(key).cloned();
    if cached
        .as_ref()
        .is_some_and(|entry| entry.checksum == asset.checksum)
    {
        return Some(result);
    }
    cache.remove(key);
    let value = asset.value.as_deref()?;
    let normalize = |content: &str| {
        content
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .trim()
            .to_string()
    };
    let mut other_content = value.to_string();
    let mut cache_entry = FileDetailsEntry {
        checksum: asset.checksum.clone(),
        ..Default::default()
    };
    for tag in ["stylesheet", "javascript", "schema"] {
        if let Some(matched) = liquid_tag(value, tag) {
            other_content = other_content.replacen(matched, "", 1);
            let content = normalize(matched);
            let changed = cached
                .as_ref()
                .map(|entry| match tag {
                    "stylesheet" => content != entry.stylesheet_tag,
                    "javascript" => content != entry.javascript_tag,
                    "schema" => content != entry.schema_tag,
                    _ => true,
                })
                .unwrap_or(true);
            match tag {
                "stylesheet" => {
                    result.stylesheet_tag = changed;
                    cache_entry.stylesheet_tag = content;
                }
                "javascript" => {
                    result.javascript_tag = changed;
                    cache_entry.javascript_tag = content;
                }
                "schema" => {
                    result.schema_tag = changed;
                    cache_entry.schema_tag = content;
                }
                _ => {}
            }
        }
    }
    let other_content = {
        let stripped = Regex::new(r"(?s)<!--.*?-->")
            .map(|regex| regex.replace_all(&other_content, "").to_string())
            .unwrap_or(other_content);
        let stripped = Regex::new(r"(?s)\{%\s*comment\s*%\}.*?\{%\s*endcomment\s*%\}")
            .map(|regex| regex.replace_all(&stripped, "").to_string())
            .unwrap_or(stripped);
        let stripped = Regex::new(r"(?s)\{%\s*doc\s*%\}.*?\{%\s*enddoc\s*%\}")
            .map(|regex| regex.replace_all(&stripped, "").to_string())
            .unwrap_or(stripped);
        normalize(&stripped)
    };
    cache_entry.liquid = other_content.clone();
    result.liquid = cached
        .as_ref()
        .map(|entry| other_content != entry.liquid)
        .unwrap_or(true);
    cache.insert(key.to_string(), cache_entry);
    Some(result)
}

fn liquid_tag<'a>(value: &'a str, tag: &str) -> Option<&'a str> {
    let pattern = format!(r"(?s)\{{%-?\s*{tag}\s*-?%\}}(.*?)\{{%-?\s*end{tag}\s*-?%\}}");
    Regex::new(&pattern)
        .ok()
        .and_then(|regex| regex.captures(value))
        .and_then(|captures| captures.get(1))
        .map(|matched| matched.as_str())
}

pub fn identify_json_reconciliation(
    remote_checksums: Vec<Checksum>,
    filesystem: &ThemeFileSystem,
    filters: &IgnoreFilters,
) -> JsonReconciliationDiff {
    let remote = filter_json_checksums(remote_checksums, filesystem, filters, &BTreeSet::new());
    let remote_by_key: BTreeMap<_, _> = remote
        .iter()
        .map(|checksum| (checksum.key.as_str(), checksum))
        .collect();
    let local_json = filter_json_assets(filesystem.files.values().cloned().collect(), filters);

    let remote_only = remote
        .iter()
        .filter(|checksum| !filesystem.files.contains_key(&checksum.key))
        .cloned()
        .collect();
    let conflicts = remote
        .iter()
        .filter(|checksum| {
            filesystem
                .files
                .get(&checksum.key)
                .is_some_and(|local| local.checksum != checksum.checksum)
        })
        .cloned()
        .collect();
    let local_only = local_json
        .into_iter()
        .filter(|asset| !remote_by_key.contains_key(asset.key.as_str()))
        .map(|asset| Checksum {
            key: asset.key,
            checksum: asset.checksum,
        })
        .collect();

    JsonReconciliationDiff {
        local_only,
        remote_only,
        conflicts,
    }
}

pub fn build_json_reconciliation_plan(
    diff: &JsonReconciliationDiff,
    nodelete: bool,
    local_only: Option<ReconciliationChoice>,
    remote_only: Option<ReconciliationChoice>,
    conflicts: Option<ReconciliationChoice>,
) -> Result<JsonReconciliationPlan, DevError> {
    let mut plan = JsonReconciliationPlan::default();
    if !nodelete && !diff.local_only.is_empty() {
        match local_only.ok_or_else(non_interactive_reconciliation_error)? {
            ReconciliationChoice::Remote => plan
                .local_files_to_delete
                .extend(diff.local_only.iter().map(|file| file.key.clone())),
            ReconciliationChoice::Local => {}
        }
    }
    if !diff.remote_only.is_empty() {
        match remote_only.ok_or_else(non_interactive_reconciliation_error)? {
            ReconciliationChoice::Remote => plan
                .files_to_download
                .extend(diff.remote_only.iter().map(|file| file.key.clone())),
            ReconciliationChoice::Local => plan
                .remote_files_to_delete
                .extend(diff.remote_only.iter().map(|file| file.key.clone())),
        }
    }
    if !diff.conflicts.is_empty() {
        match conflicts.ok_or_else(non_interactive_reconciliation_error)? {
            ReconciliationChoice::Remote => plan
                .files_to_download
                .extend(diff.conflicts.iter().map(|file| file.key.clone())),
            ReconciliationChoice::Local => {}
        }
    }
    Ok(plan)
}

fn non_interactive_reconciliation_error() -> DevError {
    DevError::Reconciliation(
        "Theme editor sync requires an interactive reconciliation choice for JSON files".into(),
    )
}

pub async fn apply_json_reconciliation<A: ThemeSyncAdmin + Sync>(
    api: &A,
    theme_id: i64,
    filesystem: &mut ThemeFileSystem,
    plan: JsonReconciliationPlan,
) -> Result<Vec<Checksum>, DevError> {
    for key in plan.local_files_to_delete {
        filesystem.delete(&key)?;
    }
    for batch in sync::batches(&plan.files_to_download, sync::DOWNLOAD_BATCH_SIZE) {
        for asset in api.fetch_assets(theme_id, batch).await? {
            filesystem.write(&asset)?;
        }
    }
    for batch in sync::batches(&plan.remote_files_to_delete, sync::MUTATION_BATCH_SIZE) {
        let _ = api.delete_assets(theme_id, batch).await?;
    }
    Ok(api.fetch_checksums(theme_id).await?)
}

async fn poll_theme_editor_changes<A>(
    api: &A,
    ctx: &DevServerContext,
    filesystem: &mut ThemeFileSystem,
    state: &AppState,
    previous_checksums: Vec<Checksum>,
) -> Result<Vec<Checksum>, DevError>
where
    A: ThemeSyncAdmin + Sync,
{
    let unsynced = state.watch.unsynced_file_keys();
    let previous = filter_json_checksums(
        previous_checksums,
        filesystem,
        &ctx.options.filters,
        &unsynced,
    );
    let latest = filter_json_checksums(
        api.fetch_checksums(ctx.theme.id).await?,
        filesystem,
        &ctx.options.filters,
        &unsynced,
    );
    let previous_by_key: BTreeMap<_, _> = previous
        .iter()
        .map(|checksum| (checksum.key.as_str(), checksum))
        .collect();
    let latest_by_key: BTreeMap<_, _> = latest
        .iter()
        .map(|checksum| (checksum.key.as_str(), checksum))
        .collect();
    let changed: Vec<_> = latest
        .iter()
        .filter(|item| {
            previous_by_key
                .get(item.key.as_str())
                .map_or(true, |previous| previous.checksum != item.checksum)
        })
        .cloned()
        .collect();
    let deleted: Vec<_> = previous
        .iter()
        .filter(|item| !latest_by_key.contains_key(item.key.as_str()))
        .cloned()
        .collect();

    abort_if_multiple_sources_changed(filesystem, &changed)?;
    let download_keys = changed
        .iter()
        .filter(|checksum| {
            filesystem
                .files
                .get(&checksum.key)
                .map_or(true, |local| local.checksum != checksum.checksum)
        })
        .map(|checksum| checksum.key.clone())
        .collect::<Vec<_>>();
    for batch in sync::batches(&download_keys, sync::DOWNLOAD_BATCH_SIZE) {
        for asset in api.fetch_assets(ctx.theme.id, batch).await? {
            filesystem.write(&asset)?;
            state
                .files
                .lock()
                .expect("file state poisoned")
                .insert(asset.key.clone(), asset);
        }
    }
    if !ctx.options.nodelete {
        for checksum in deleted {
            if filesystem.files.contains_key(&checksum.key) {
                filesystem.delete(&checksum.key)?;
                state
                    .files
                    .lock()
                    .expect("file state poisoned")
                    .remove(&checksum.key);
            }
        }
    }
    Ok(latest)
}

pub fn abort_if_multiple_sources_changed(
    filesystem: &ThemeFileSystem,
    changed: &[Checksum],
) -> Result<(), DevError> {
    for checksum in changed {
        if let Some(previous) = filesystem.files.get(&checksum.key) {
            let current = read_theme_asset(&filesystem.root, &checksum.key)?;
            if current
                .as_ref()
                .is_some_and(|current| current.checksum != previous.checksum)
            {
                return Err(DevError::Reconciliation(format!(
                    "Detected changes to the file '{}' on both local and remote sources. Aborting...",
                    checksum.key
                )));
            }
        }
    }
    Ok(())
}

pub fn filter_json_checksums(
    checksums: Vec<Checksum>,
    filesystem: &ThemeFileSystem,
    filters: &IgnoreFilters,
    unsynced: &BTreeSet<String>,
) -> Vec<Checksum> {
    apply_ignore_filters(
        apply_ignore_filters(checksums, &filesystem.filters),
        filters,
    )
    .into_iter()
    .filter(|file| file.key.ends_with(".json"))
    .filter(|file| !unsynced.contains(&file.key))
    .collect()
}

fn filter_json_assets(assets: Vec<ThemeAsset>, filters: &IgnoreFilters) -> Vec<ThemeAsset> {
    apply_ignore_filters(assets, filters)
        .into_iter()
        .filter(|file| file.key.ends_with(".json"))
        .collect()
}

fn start_terminal_controls(
    urls: DevServerUrls,
    last_requested_path: Arc<Mutex<String>>,
    shutdown_tx: watch::Sender<bool>,
) -> tokio::task::JoinHandle<()> {
    tokio::task::spawn_blocking(move || {
        use crossterm::event::{self, Event as TerminalEvent, KeyCode, KeyModifiers};
        use std::io::IsTerminal;

        if !std::io::stdin().is_terminal() {
            return;
        }
        let raw_enabled = crossterm::terminal::enable_raw_mode().is_ok();
        // Leading 100ms debounce matching upstream createKeypressHandler.
        let last_open: Arc<Mutex<Option<std::time::Instant>>> = Arc::new(Mutex::new(None));
        while !*shutdown_tx.borrow() {
            if !event::poll(Duration::from_millis(100)).unwrap_or(false) {
                continue;
            }
            let Ok(TerminalEvent::Key(key)) = event::read() else {
                continue;
            };
            if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                let _ = shutdown_tx.send(true);
                break;
            }
            let target = match key.code {
                KeyCode::Char('t') => Some(urls.local.clone()),
                KeyCode::Char('p') => Some(urls.preview.clone()),
                KeyCode::Char('g') => Some(urls.gift_card.clone()),
                KeyCode::Char('e') => {
                    let path = last_requested_path
                        .lock()
                        .expect("last requested path poisoned")
                        .clone();
                    if path.is_empty() || path == "/" {
                        Some(urls.editor.clone())
                    } else {
                        // Editor URL already contains `?hr=`; append with `&`.
                        Some(format!(
                            "{}&previewPath={}",
                            urls.editor,
                            url::form_urlencoded::byte_serialize(path.as_bytes())
                                .collect::<String>()
                        ))
                    }
                }
                _ => None,
            };
            if let Some(target) = target {
                let now = std::time::Instant::now();
                let mut last = last_open.lock().expect("last open poisoned");
                if last
                    .map(|instant| now.duration_since(instant) < Duration::from_millis(100))
                    .unwrap_or(false)
                {
                    continue;
                }
                *last = Some(now);
                drop(last);
                if let Err(error) = open::that(&target) {
                    eprintln!("Failed to open {target}: {error}");
                }
            }
        }
        if raw_enabled {
            let _ = crossterm::terminal::disable_raw_mode();
        }
    })
}

type SectionNamesByFile = Arc<Mutex<BTreeMap<String, Vec<(String, String)>>>>;

#[derive(Clone)]
struct AppState {
    ctx: Arc<DevServerContext>,
    session: Arc<Mutex<DevServerSession>>,
    files: Arc<Mutex<BTreeMap<String, ThemeAsset>>>,
    watch: ThemeWatchState,
    last_requested_path: Arc<Mutex<String>>,
    reload_tx: broadcast::Sender<HotReloadEvent>,
    client: reqwest::Client,
    refresh: Option<DevServerRefresh>,
    theme_id_mismatch_redirects: Arc<AtomicUsize>,
    section_names_by_file: SectionNamesByFile,
    file_details_cache: Arc<Mutex<BTreeMap<String, FileDetailsEntry>>>,
    extension_files: Arc<Mutex<BTreeMap<String, ThemeAsset>>>,
    extension_unsynced: Arc<Mutex<BTreeSet<String>>>,
}

fn router(state: AppState) -> Router {
    let origins = [
        local_origin(&state.ctx),
        format!("https://{}", state.ctx.session.store_fqdn),
        "https://online-store-web.shopifyapps.com".into(),
    ];
    let cors =
        CorsLayer::new().allow_origin(AllowOrigin::predicate(move |origin: &HeaderValue, _| {
            origin
                .to_str()
                .ok()
                .is_some_and(|origin| origins.iter().any(|allowed| allowed == origin))
        }));

    Router::new()
        .route("/__theme_dev/hot-reload", get(hot_reload))
        .route("/assets/*path", get(asset))
        .route("/compiled_assets/*path", get(compiled_asset_or_proxy))
        .route("/cdn/*path", get(cdn_asset_or_proxy))
        .route("/ext/cdn/*path", get(ext_cdn_asset_or_proxy))
        .route(
            LOCAL_HOT_RELOAD_SCRIPT_ENDPOINT,
            get(local_hot_reload_script),
        )
        .fallback(any(proxy_or_render))
        .layer(cors)
        .with_state(state)
}

fn local_origin(ctx: &DevServerContext) -> String {
    format!("http://{}:{}", ctx.options.host, ctx.options.port)
}

async fn local_hot_reload_script() -> Response {
    let Some(path) = std::env::var("SHOPIFY_CLI_LOCAL_HOT_RELOAD").ok() else {
        return StatusCode::NO_CONTENT.into_response();
    };
    match tokio::fs::read_to_string(&path).await {
        Ok(content) => ([(CONTENT_TYPE, "application/javascript")], content).into_response(),
        Err(error) => {
            eprintln!("Failed to read local hot-reload script at {path}: {error}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
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
        theme_id: state.ctx.theme.id.to_string(),
    });
    let stream: Pin<Box<dyn Stream<Item = Result<Event, Infallible>> + Send>> = Box::pin(stream);
    Sse::new(stream).keep_alive(KeepAlive::default())
}

async fn asset(
    State(state): State<AppState>,
    AxumPath(path): AxumPath<String>,
) -> impl IntoResponse {
    serve_asset(&state, &format!("assets/{}", decode_path(&path)))
}

async fn cdn_asset_or_proxy(
    State(state): State<AppState>,
    AxumPath(path): AxumPath<String>,
    request: Request,
) -> Response {
    if let Some(response) = compiled_asset_response(&state, &path) {
        return response;
    }
    if let Some(key) = local_theme_asset_key_for_cdn_path(&path) {
        if let Some(response) = local_asset_response(&state, &key) {
            return response;
        }
    }
    if let Some(key) = local_extension_asset_key_for_cdn_path(&path) {
        if let Some(response) = local_extension_asset_response(&state, &key) {
            return response;
        }
    }
    proxy_request(state, request).await
}

async fn ext_cdn_asset_or_proxy(
    State(state): State<AppState>,
    AxumPath(path): AxumPath<String>,
    request: Request,
) -> Response {
    if let Some(key) = local_extension_asset_key_for_cdn_path(&path) {
        if let Some(response) = local_extension_asset_response(&state, &key) {
            return response;
        }
    }
    proxy_request(state, request).await
}

async fn compiled_asset_or_proxy(
    State(state): State<AppState>,
    AxumPath(path): AxumPath<String>,
    request: Request,
) -> Response {
    if let Some(response) = compiled_asset_response(&state, &path) {
        return response;
    }
    proxy_request(state, request).await
}

fn local_theme_asset_key_for_cdn_path(path: &str) -> Option<String> {
    if path.starts_with("extensions/") {
        return None;
    }
    let index = path.find("/assets/")?;
    let asset_path = path[index + "/assets/".len()..]
        .split(['?', '#'])
        .next()
        .unwrap_or_default();
    Some(format!("assets/{}", decode_path(asset_path)))
}

/// Maps `/extensions/<uuid>/<app>/assets/<name>` CDN paths to `assets/<name>`.
pub fn local_extension_asset_key_for_cdn_path(path: &str) -> Option<String> {
    let path = path.trim_start_matches('/');
    let path = path
        .strip_prefix(EXTENSION_CDN_PREFIX.trim_start_matches('/'))
        .unwrap_or(path)
        .trim_start_matches('/');
    if !path.starts_with("extensions/") {
        return None;
    }
    let index = path.find("/assets/")?;
    let asset_path = path[index + "/assets/".len()..]
        .split(['?', '#'])
        .next()
        .unwrap_or_default();
    if asset_path.is_empty() {
        return None;
    }
    Some(format!("assets/{}", decode_path(asset_path)))
}

fn serve_asset(state: &AppState, key: &str) -> Response {
    local_asset_response(state, key).unwrap_or_else(|| StatusCode::NOT_FOUND.into_response())
}

fn local_asset_response(state: &AppState, key: &str) -> Option<Response> {
    let files = state.files.lock().expect("file state poisoned");
    let extension_files = state.extension_files.lock().expect("ext files poisoned");
    let asset = files.get(key)?;
    asset_to_local_response(
        asset,
        key,
        &state.ctx.session.store_fqdn,
        &files,
        &extension_files,
        state.ctx.options.standard_events_dev_bundle,
    )
}

fn local_extension_asset_response(state: &AppState, key: &str) -> Option<Response> {
    let files = state.files.lock().expect("file state poisoned");
    let extension_files = state.extension_files.lock().expect("ext files poisoned");
    let asset = extension_files.get(key)?;
    asset_to_local_response(
        asset,
        key,
        &state.ctx.session.store_fqdn,
        &files,
        &extension_files,
        state.ctx.options.standard_events_dev_bundle,
    )
}

fn asset_to_local_response(
    asset: &ThemeAsset,
    key: &str,
    store_fqdn: &str,
    files: &BTreeMap<String, ThemeAsset>,
    extension_files: &BTreeMap<String, ThemeAsset>,
    standard_events_dev_bundle: bool,
) -> Option<Response> {
    let content_type = mime_guess::from_path(key).first_or_octet_stream();
    let (mut response, content_length) = if let Some(value) = &asset.value {
        let body = inject_cdn_proxy(
            value,
            store_fqdn,
            files,
            extension_files,
            standard_events_dev_bundle,
        );
        let length = body.len();
        (body.into_response(), length)
    } else if let Some(attachment) = &asset.attachment {
        match base64::Engine::decode(&base64::engine::general_purpose::STANDARD, attachment) {
            Ok(bytes) => {
                let length = bytes.len();
                (bytes.into_response(), length)
            }
            Err(_) => return Some(StatusCode::INTERNAL_SERVER_ERROR.into_response()),
        }
    } else {
        (String::new().into_response(), 0)
    };
    response.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_str(content_type.as_ref())
            .unwrap_or(HeaderValue::from_static("application/octet-stream")),
    );
    if let Ok(value) = HeaderValue::from_str(&content_length.to_string()) {
        response.headers_mut().insert(CONTENT_LENGTH, value);
    }
    response
        .headers_mut()
        .insert("x-local-asset", HeaderValue::from_static("true"));
    Some(response)
}

fn compiled_asset_response(state: &AppState, path: &str) -> Option<Response> {
    let asset_name = compiled_asset_name(path)?;
    let files = state.files.lock().expect("file state poisoned");
    let (content_type, body) = match asset_name {
        "styles.css" => ("text/css", compiled_stylesheet(&files)),
        "block-scripts.js" => ("text/javascript", compiled_javascript(&files, "block")),
        "snippet-scripts.js" => ("text/javascript", compiled_javascript(&files, "snippet")),
        "scripts.js" => ("text/javascript", compiled_javascript(&files, "section")),
        _ => return None,
    };
    let mut response = Body::from(body.clone()).into_response();
    response
        .headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_static(content_type));
    if let Ok(value) = HeaderValue::from_str(&body.len().to_string()) {
        response.headers_mut().insert(CONTENT_LENGTH, value);
    }
    response
        .headers_mut()
        .insert("x-local-asset", HeaderValue::from_static("true"));
    Some(response)
}

fn compiled_asset_name(path: &str) -> Option<&str> {
    path.strip_prefix("compiled_assets/")
        .or_else(|| path.rsplit_once("/compiled_assets/").map(|(_, name)| name))
        .or_else(|| (!path.contains('/')).then_some(path))
        .and_then(|name| name.split(['?', '#']).next())
}

fn compiled_stylesheet(files: &BTreeMap<String, ThemeAsset>) -> String {
    let mut stylesheets = vec!["/*** GENERATED LOCALLY ***/\n".to_string()];
    for (_, asset) in liquid_files_by_kind(files, "section")
        .into_iter()
        .chain(liquid_files_by_kind(files, "block"))
        .chain(liquid_files_by_kind(files, "snippet"))
    {
        if let Some(content) = tagged_compiled_content(asset, "stylesheet") {
            stylesheets.push(content);
        }
    }
    stylesheets.join("\n")
}

fn compiled_javascript(files: &BTreeMap<String, ThemeAsset>, kind: &str) -> String {
    let plural = format!("{kind}s");
    let mut scripts = vec![format!(
        r#"
      /*** GENERATED LOCALLY ***/

      (function () {{
        var __{plural}__ = {{}};

        (function () {{
          var element = document.getElementById("{plural}-script");
          var attribute = element ? element.getAttribute("data-{plural}") : "";
          var {plural} = attribute.split(",").filter(Boolean);

          for (var i = 0; i < {plural}.length; i++) {{
            __{plural}__[{plural}[i]] = true;
          }}
        }})();"#
    )];

    for (key, asset) in liquid_files_by_kind(files, kind) {
        let Some(javascript) = tagged_compiled_content(asset, "javascript") else {
            continue;
        };
        let base_name = key
            .rsplit('/')
            .next()
            .and_then(|name| name.strip_suffix(".liquid"))
            .unwrap_or(key);
        scripts.push(format!(
            r#"
        (function () {{
          if (!__{plural}__["{base_name}"] && !Shopify.designMode) return;
          try {{
            {javascript}
          }} catch (e) {{
            console.error(e);
          }}
        }})();"#
        ));
    }
    scripts.push("})();".into());
    scripts.join("\n")
}

fn tagged_compiled_content(asset: &ThemeAsset, tag: &str) -> Option<String> {
    let value = asset.value.as_deref()?;
    let content = liquid_tag(value, tag)?.trim_end();
    if content.is_empty() {
        return None;
    }
    Some(format!("/* {} */\n{}", asset.key, content))
}

fn liquid_files_by_kind<'a>(
    files: &'a BTreeMap<String, ThemeAsset>,
    kind: &str,
) -> Vec<(&'a str, &'a ThemeAsset)> {
    let prefix = format!("{kind}s/");
    files
        .iter()
        .filter(|(key, _)| key.starts_with(&prefix) && key.ends_with(".liquid"))
        .map(|(key, asset)| (key.as_str(), asset))
        .collect()
}

async fn proxy_or_render(
    State(state): State<AppState>,
    Query(query): Query<BTreeMap<String, String>>,
    request: Request,
) -> Response {
    if !valid_host(&state.ctx, request.headers()) {
        return (StatusCode::BAD_REQUEST, "Invalid Host header").into_response();
    }
    if should_ignore(request.uri().path()) {
        return StatusCode::NO_CONTENT.into_response();
    }
    if state.ctx.options.live_reload != LiveReloadMode::Off {
        let accepts_sse = request
            .headers()
            .get(ACCEPT)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value == "text/event-stream");
        if accepts_sse {
            return hot_reload(State(state)).await.into_response();
        }
        if let Some(raw) = query.get("hr-log") {
            return handle_hr_log(raw);
        }
        if query.contains_key("section_id") || query.contains_key("app_block_id") {
            return render_section(state, request, query).await;
        }
    }
    if can_proxy_request(request.method(), request.uri(), request.headers()) {
        return proxy_request(state, request).await;
    }
    render_storefront(state, request, query).await
}

fn valid_host(ctx: &DevServerContext, headers: &HeaderMap) -> bool {
    let Some(host) = headers.get(HOST).and_then(|value| value.to_str().ok()) else {
        return false;
    };
    allowed_hosts(&ctx.options.host, ctx.options.port).contains(&normalize_host_header(host))
}

pub fn allowed_hosts(host: &str, port: u16) -> BTreeSet<String> {
    let mut allowed = BTreeSet::new();
    let normalized = host.to_lowercase();
    let suffix = format!(":{port}");
    allowed.insert(format!("{normalized}{suffix}"));
    if ["localhost", "127.0.0.1", "::1", "0.0.0.0"].contains(&normalized.as_str()) {
        allowed.insert(format!("localhost{suffix}"));
        allowed.insert(format!("127.0.0.1{suffix}"));
        allowed.insert(format!("[::1]{suffix}"));
    }
    // Wildcard bind is reachable via every NIC address (LAN preview from other devices).
    if normalized == "0.0.0.0" || normalized == "::" {
        for address in local_interface_addresses() {
            let formatted = match address {
                IpAddr::V4(v4) => v4.to_string(),
                IpAddr::V6(v6) => format!("[{v6}]"),
            };
            allowed.insert(format!("{formatted}{suffix}"));
        }
    }
    allowed
}

/// Enumerates local interface IPs (best-effort; Linux `/proc` + UDP bind probe).
fn local_interface_addresses() -> Vec<IpAddr> {
    let mut addresses = BTreeSet::new();
    if let Ok(contents) = std::fs::read_to_string("/proc/net/fib_trie") {
        for line in contents.lines() {
            let trimmed = line.trim();
            if let Some(ip) = trimmed.strip_prefix("|-- ") {
                if let Ok(addr) = ip.parse::<std::net::Ipv4Addr>() {
                    if !addr.is_unspecified() {
                        addresses.insert(IpAddr::V4(addr));
                    }
                }
            }
        }
    }
    // Always include loopback variants for wildcard binds.
    addresses.insert(IpAddr::V4(std::net::Ipv4Addr::LOCALHOST));
    addresses.insert(IpAddr::V6(std::net::Ipv6Addr::LOCALHOST));
    // UDP connect trick discovers the primary outbound interface address.
    if let Ok(socket) = std::net::UdpSocket::bind("0.0.0.0:0") {
        if socket.connect("8.8.8.8:80").is_ok() {
            if let Ok(local) = socket.local_addr() {
                addresses.insert(local.ip());
            }
        }
    }
    if let Ok(socket) = std::net::UdpSocket::bind("[::]:0") {
        if socket.connect("[2001:4860:4860::8888]:80").is_ok() {
            if let Ok(local) = socket.local_addr() {
                addresses.insert(local.ip());
            }
        }
    }
    addresses.into_iter().collect()
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

pub fn can_proxy_request(method: &Method, uri: &Uri, headers: &HeaderMap) -> bool {
    let path = uri.path();
    if method != Method::GET {
        return true;
    }
    if path.starts_with("/cart/")
        || path == "/cart.json"
        || (path.starts_with("/checkouts/") && !path.starts_with("/checkouts/internal/"))
        || path == "/account"
        || path == "/account/"
        || path == "/account/logout"
        || path == "/account/logout/"
        || path == "/account/login/multipass"
        || path == "/account/login/multipass/"
        || path.starts_with("/account/login/multipass/")
        || path.starts_with("/cdn/")
        || path.starts_with("/ext/cdn/")
    {
        return true;
    }
    if storefront_api_path(path) {
        return true;
    }
    let accepts = headers
        .get(ACCEPT)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("*/*");
    let extension = Path::new(path).extension().and_then(|ext| ext.to_str());
    if extension == Some("html") || accepts.contains("text/html") {
        return false;
    }
    extension.is_some() || accepts != "*/*"
}

fn storefront_api_path(path: &str) -> bool {
    Regex::new(r"^/api/(unstable|\d{4}-\d{2})/graphql\.json$")
        .map(|regex| regex.is_match(path))
        .unwrap_or(false)
}

/// Builds the `replace_templates` rendered for a single-section re-render.
/// Returns `None` when the section template is unsynced but no longer present
/// locally (i.e. the section was removed), mirroring upstream.
fn build_section_replace_templates(
    state: &AppState,
    section_key: &str,
    section_id: &str,
    unsynced: &BTreeSet<String>,
) -> Option<BTreeMap<String, String>> {
    let mut replace_templates = BTreeMap::new();
    if section_id.is_empty() {
        return Some(replace_templates);
    }

    let section_template = {
        let files = state.files.lock().expect("file state poisoned");
        files.get(section_key).and_then(|asset| asset.value.clone())
    };
    if unsynced.contains(section_key) {
        let section_template = section_template?;
        replace_templates.insert(section_key.to_string(), section_template);
    }

    let (files, section_names) = {
        let files = state.files.lock().expect("file state poisoned");
        let section_names = state
            .section_names_by_file
            .lock()
            .expect("section names poisoned");
        (files.clone(), section_names.clone())
    };
    for file_key in unsynced {
        if !file_key.ends_with(".json") {
            continue;
        }
        let Some(entries) = section_names.get(file_key) else {
            continue;
        };
        for (_section_type, name) in entries {
            if section_id.ends_with(&format!("__{name}")) {
                if let Some(content) = files.get(file_key).and_then(|asset| asset.value.clone()) {
                    if !content.is_empty() {
                        replace_templates.insert(file_key.clone(), content);
                    }
                }
                continue;
            }
        }
    }
    Some(replace_templates)
}

/// Handles `?hr-log=` requests from the hot-reload client: parses the JSON
/// message and renders it as an error/warning/info for the developer
/// (mirrors upstream `hot-reload/server.ts`).
fn handle_hr_log(raw: &str) -> Response {
    let message = serde_json::from_str::<serde_json::Value>(raw).ok();
    if let Some(message) = message {
        let headline = message
            .get("headline")
            .and_then(|value| value.as_str())
            .unwrap_or_default();
        let headline = format!("[HotReload] {headline}");
        let body = message.get("body").and_then(|value| value.as_str());
        let rendered = match body {
            Some(body) => format!("{headline}\n{body}"),
            None => headline,
        };
        match message
            .get("type")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
        {
            "error" => eprintln!("{}", rendered.red()),
            "warn" => eprintln!("{}", rendered.yellow()),
            "info" => println!("{}", rendered),
            other => eprintln!("Unknown HotReload log type: {other}"),
        }
    }
    StatusCode::NO_CONTENT.into_response()
}

/// Re-renders a single section (or app block) in the storefront, triggered
/// by the hot-reload client after a template change (mirrors upstream
/// `hot-reload/server.ts` `getHotReloadHandler`).
async fn render_section(
    state: AppState,
    request: Request,
    query: BTreeMap<String, String>,
) -> Response {
    let section_key = query.get("section_key").cloned().unwrap_or_default();
    let section_id = query.get("section_id").cloned().unwrap_or_default();
    let app_block_id = query.get("app_block_id").cloned().unwrap_or_default();
    let path = request.uri().path().to_string();
    let mut browser_search: BTreeMap<String, String> = query
        .iter()
        .filter(|(key, _)| {
            !matches!(
                key.as_str(),
                "section_key" | "section_id" | "app_block_id" | "_fd" | "pb"
            )
        })
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();

    if section_id.is_empty() && app_block_id.is_empty() {
        return StatusCode::NO_CONTENT.into_response();
    }

    let Some(replace_templates) = build_section_replace_templates(
        &state,
        &section_key,
        &section_id,
        &state.watch.unsynced_file_keys(),
    ) else {
        return String::new().into_response();
    };

    let method = request.method().clone();
    let headers = request.headers().clone();
    let (method, body, headers) = if replace_templates.is_empty() {
        (method, None, headers)
    } else {
        let body = axum::body::Bytes::from(storefront_replace_templates_params(
            &replace_templates,
            method.as_str(),
        ));
        let mut headers = headers;
        headers.insert(
            CONTENT_TYPE,
            HeaderValue::from_static("application/x-www-form-urlencoded"),
        );
        (Method::POST, Some(body), headers)
    };

    if !section_id.is_empty() {
        browser_search.insert("section_id".into(), section_id);
    } else {
        browser_search.insert("app_block_id".into(), app_block_id.clone());
    }

    let response = remote_request(&state, method, &path, browser_search, headers, body).await;
    match response {
        Ok(response) => patch_section_response(state, response).await,
        Err(error) => {
            if app_block_id.is_empty() {
                eprintln!("{}", "Failed to render section on Hot Reload".red());
                create_error_page_response(
                    &state.ctx,
                    StatusCode::BAD_GATEWAY,
                    "Failed to render section on Hot Reload",
                    "Failed to render section on Hot Reload",
                    vec![ErrorPageError {
                        message: error.to_string(),
                        code: String::new(),
                    }],
                )
            } else {
                (StatusCode::BAD_GATEWAY, "").into_response()
            }
        }
    }
}

/// Patches a single-section rendering response by proxying CDN URLs and
/// rewriting base URL attributes, without injecting the HTML body patchers used
/// for full-page renders (mirrors upstream `patchRenderingResponse`).
async fn patch_section_response(state: AppState, response: reqwest::Response) -> Response {
    let status_code = response.status();
    let mut headers = response.headers().clone();
    update_session_cookies_from_headers(&state, &headers);
    patch_set_cookie_domains(&state, &mut headers);
    strip_response_headers(&mut headers);
    if let Some(link) = headers
        .get("link")
        .and_then(|value| value.to_str().ok())
        .map(|link| {
            inject_cdn_proxy(
                link,
                &state.ctx.session.store_fqdn,
                &state.files.lock().expect("file state poisoned"),
                &state.extension_files.lock().expect("ext files poisoned"),
                state.ctx.options.standard_events_dev_bundle,
            )
        })
    {
        headers.insert(
            "link",
            HeaderValue::from_str(&link).unwrap_or(HeaderValue::from_static("")),
        );
    }
    if status_code.is_redirection() {
        let body = response.bytes().await.unwrap_or_default();
        let mut out = Response::new(Body::from(body));
        *out.status_mut() = status_code;
        *out.headers_mut() = headers;
        return out;
    }
    let body = response.text().await.unwrap_or_default();
    let body = inject_cdn_proxy(
        &body,
        &state.ctx.session.store_fqdn,
        &state.files.lock().expect("file state poisoned"),
        &state.extension_files.lock().expect("ext files poisoned"),
        state.ctx.options.standard_events_dev_bundle,
    );
    let body = patch_base_url_attributes(&body, &local_origin(&state.ctx));
    let body = if state.ctx.options.standard_events_inspector {
        inject_standard_events_inspector(&body)
    } else {
        body
    };
    let mut out = (status_code, body).into_response();
    let is_json = headers
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.contains("application/json"));
    *out.headers_mut() = headers;
    if !is_json {
        out.headers_mut().insert(
            CONTENT_TYPE,
            HeaderValue::from_static("text/html; charset=utf-8"),
        );
    }
    out
}

async fn render_storefront(
    state: AppState,
    request: Request,
    mut query: BTreeMap<String, String>,
) -> Response {
    if request
        .headers()
        .get("sec-fetch-mode")
        .and_then(|value| value.to_str().ok())
        == Some("navigate")
    {
        let path = request
            .uri()
            .path_and_query()
            .map(|value| value.as_str().to_string())
            .unwrap_or_else(|| request.uri().path().to_string());
        *state
            .last_requested_path
            .lock()
            .expect("last requested path poisoned") = path;
    }
    if state.ctx.options.error_overlay != ErrorOverlayMode::Silent {
        let errors = state
            .watch
            .upload_errors
            .lock()
            .expect("upload errors poisoned");
        if !errors.is_empty() {
            return upload_error_page(&state.ctx, &errors);
        }
    }
    query.insert("preview_theme_id".into(), state.ctx.theme.id.to_string());
    query.insert("_fd".into(), "0".into());
    query.insert("pb".into(), "0".into());
    let browser = request
        .uri()
        .path_and_query()
        .map(|value| value.as_str().to_string())
        .unwrap_or_else(|| request.uri().path().to_string());
    let path = request.uri().path().to_string();
    let original_method = request.method().clone();
    let original_headers = request.headers().clone();
    let locale = original_headers
        .get("cookie")
        .and_then(|value| value.to_str().ok())
        .and_then(|cookies| parse_cookies(cookies).get("localization").cloned())
        .map(|locale| locale.to_ascii_lowercase());
    let unsynced = state.watch.unsynced_file_keys();
    let replace_templates = {
        let files = state.files.lock().expect("file state poisoned");
        get_in_memory_templates(&files, &unsynced, Some(&path), locale.as_deref())
    };
    let replace_extension_templates = {
        let files = state.extension_files.lock().expect("ext files poisoned");
        let unsynced = state
            .extension_unsynced
            .lock()
            .expect("ext unsynced poisoned");
        files
            .iter()
            .filter(|(key, _)| unsynced.contains(*key))
            .filter_map(|(key, asset)| {
                let content = asset.value.clone().or_else(|| asset.attachment.clone())?;
                Some((key.clone(), content))
            })
            .collect::<BTreeMap<_, _>>()
    };
    let (method, body, headers) = if replace_templates.is_empty()
        && replace_extension_templates.is_empty()
    {
        (original_method.clone(), None, original_headers.clone())
    } else {
        let body = axum::body::Bytes::from(storefront_replace_templates_params_with_extensions(
            &replace_templates,
            &replace_extension_templates,
            original_method.as_str(),
        ));
        let mut headers = original_headers.clone();
        headers.insert(
            CONTENT_TYPE,
            HeaderValue::from_static("application/x-www-form-urlencoded"),
        );
        (Method::POST, Some(body), headers)
    };
    let response = remote_request(&state, method, &path, query.clone(), headers, body).await;
    let response = match response {
        Ok(response) => {
            let status = response.status().as_u16();
            if (400..500).contains(&status) && !is_known_rendering_request(&query) {
                eprintln!("Render failed for {path} with {status}, trying proxy...");
                let proxy_response = remote_request(
                    &state,
                    original_method.clone(),
                    &path,
                    query,
                    original_headers,
                    None,
                )
                .await;
                match proxy_response {
                    Ok(proxy) if proxy.status().as_u16() < 400 => {
                        let proxy_status = proxy.status().as_u16();
                        eprintln!("Proxy status: {proxy_status}. Returning proxy response.");
                        log_request_line(
                            &path,
                            original_method.as_ref(),
                            proxy_status,
                            proxy
                                .headers()
                                .get("server-timing")
                                .and_then(|value| value.to_str().ok()),
                        );
                        let is_html = proxy
                            .headers()
                            .get(CONTENT_TYPE)
                            .and_then(|value| value.to_str().ok())
                            .is_some_and(|value| value.contains("text/html"));
                        let patch_html = is_html && state.ctx.options.standard_events_inspector;
                        return patch_response(state, Ok(proxy), patch_html, Some(browser)).await;
                    }
                    Ok(proxy) => {
                        eprintln!(
                            "Proxy status: {}. Returning render error.",
                            proxy.status().as_u16()
                        );
                        Ok(response)
                    }
                    Err(_) => Ok(response),
                }
            } else {
                Ok(response)
            }
        }
        Err(error) => {
            log_request_line(
                &path,
                original_method.as_ref(),
                u16::from(StatusCode::BAD_GATEWAY),
                None,
            );
            return create_error_page_response(
                &state.ctx,
                StatusCode::BAD_GATEWAY,
                "Failed to render storefront",
                "Failed to render storefront",
                vec![ErrorPageError {
                    message: error.to_string(),
                    code: String::new(),
                }],
            );
        }
    };
    log_request_line(
        &path,
        original_method.as_ref(),
        response
            .as_ref()
            .map_or(u16::from(StatusCode::BAD_GATEWAY), |response| {
                response.status().as_u16()
            }),
        response
            .as_ref()
            .ok()
            .and_then(|response| {
                response
                    .headers()
                    .get("server-timing")
                    .and_then(|value| value.to_str().ok())
                    .map(ToOwned::to_owned)
            })
            .as_deref(),
    );
    patch_response(state, response, true, Some(browser)).await
}

/// Detects SFR-specific rendering query params that should not fall back to proxy.
pub fn is_known_rendering_request(query: &BTreeMap<String, String>) -> bool {
    ["section_id", "sections", "app_block_id"]
        .iter()
        .any(|key| query.contains_key(*key))
}

async fn proxy_request(state: AppState, request: Request) -> Response {
    let method = request.method().clone();
    let path = request.uri().path().to_string();
    let query = request
        .uri()
        .query()
        .map(|query| {
            if is_stale_asset_query(&path, query) {
                BTreeMap::from([(
                    "v".into(),
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|duration| duration.as_millis().to_string())
                        .unwrap_or_default(),
                )])
            } else {
                url::form_urlencoded::parse(query.as_bytes())
                    .into_owned()
                    .collect::<BTreeMap<_, _>>()
            }
        })
        .unwrap_or_default();
    let headers = request.headers().clone();
    let body = axum::body::to_bytes(request.into_body(), usize::MAX)
        .await
        .ok();
    let response = remote_request(&state, method.clone(), &path, query, headers, body).await;
    let status = response
        .as_ref()
        .map_or(u16::from(StatusCode::BAD_GATEWAY), |response| {
            response.status().as_u16()
        });
    log_request_line(
        &path,
        method.as_ref(),
        status,
        response
            .as_ref()
            .ok()
            .and_then(|response| {
                response
                    .headers()
                    .get("server-timing")
                    .and_then(|value| value.to_str().ok())
                    .map(ToOwned::to_owned)
            })
            .as_deref(),
    );
    patch_response(state, response, false, None).await
}

async fn remote_request(
    state: &AppState,
    method: Method,
    path: &str,
    query: BTreeMap<String, String>,
    headers: HeaderMap,
    body: Option<axum::body::Bytes>,
) -> Result<reqwest::Response, DevError> {
    let session = state
        .session
        .lock()
        .expect("session state poisoned")
        .clone();
    let url = proxy_storefront_url(&state.ctx, path, &query)?;
    let mut builder = state.client.request(method, url);
    for (key, value) in proxy_storefront_headers(&headers, None).iter() {
        builder = builder.header(key, value);
    }
    if !storefront_api_path(path) {
        builder = builder
            .header("referer", format!("https://{}", session.store_fqdn))
            .header("cookie", build_cookie_header(&session, &headers));
        if should_send_storefront_bearer(state.ctx.kind) {
            if let Some(token) = &session.storefront_token {
                builder = builder.bearer_auth(token);
            }
        }
        if let Some(domain) = &session.theme_access_domain {
            builder = builder
                .header("x-shopify-shop", &session.store_fqdn)
                .header("x-shopify-access-token", &session.admin_token)
                .header("host", domain);
        }
    }
    if let Some(body) = body {
        builder = builder.body(body);
    }
    builder
        .send()
        .await
        .map_err(|error| DevError::Server(error.to_string()))
}

/// Whether storefront requests should include a Bearer token (theme servers only).
pub fn should_send_storefront_bearer(kind: DevServerKind) -> bool {
    kind == DevServerKind::Theme
}

fn proxy_storefront_url(
    ctx: &DevServerContext,
    path: &str,
    query: &BTreeMap<String, String>,
) -> Result<Url, DevError> {
    let extension_cdn = path.starts_with("/ext/cdn/");
    let host = if extension_cdn {
        "cdn.shopify.com"
    } else if let Some(domain) = &ctx.session.theme_access_domain {
        domain
    } else {
        &ctx.session.store_fqdn
    };
    let expected_host = host.to_string();
    if let Some(actual) = path
        .strip_prefix("//")
        .and_then(|path| path.split('/').next())
    {
        return Err(DevError::HostnameMismatch {
            expected: expected_host,
            actual: actual.to_string(),
        });
    }
    let proxy_path = if extension_cdn {
        path.trim_start_matches("/ext/cdn")
    } else if ctx.session.theme_access_domain.is_some() {
        "/cli/sfr"
    } else {
        path
    };
    if let Some(actual) = proxy_path
        .strip_prefix("//")
        .and_then(|path| path.split('/').next())
    {
        return Err(DevError::HostnameMismatch {
            expected: expected_host,
            actual: actual.to_string(),
        });
    }
    let mut url = Url::parse(&format!("https://{host}{proxy_path}")).expect("proxy URL is valid");
    if ctx.session.theme_access_domain.is_some() && !extension_cdn {
        let path = path.trim_start_matches('/');
        url.set_path(&format!("/cli/sfr/{path}"));
    }
    let mut query_pairs = query
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect::<Vec<_>>();
    if !storefront_api_path(path) {
        if !query.contains_key("_fd") {
            query_pairs.push(("_fd", "0"));
        }
        if !query.contains_key("pb") {
            query_pairs.push(("pb", "0"));
        }
    }
    if !query_pairs.is_empty() {
        url.query_pairs_mut().extend_pairs(query_pairs);
    }
    if url.host_str() != Some(expected_host.as_str()) {
        return Err(DevError::HostnameMismatch {
            expected: expected_host,
            actual: url.host_str().unwrap_or_default().to_string(),
        });
    }
    Ok(url)
}

fn proxy_storefront_headers(headers: &HeaderMap, client_ip: Option<&str>) -> HeaderMap {
    let mut out = HeaderMap::new();
    for (key, value) in headers.iter() {
        if is_hop_by_hop(key.as_str())
            || key
                .as_str()
                .eq_ignore_ascii_case("upgrade-insecure-requests")
        {
            continue;
        }
        out.append(key, value.clone());
    }
    if let Some(client_ip) = client_ip {
        if let Ok(value) = HeaderValue::from_str(client_ip) {
            out.insert("x-forwarded-for", value);
        }
    }
    out
}

fn upload_error_page(ctx: &DevServerContext, errors: &BTreeMap<String, Vec<String>>) -> Response {
    let page_errors = errors
        .iter()
        .map(|(key, errors)| ErrorPageError {
            message: key.clone(),
            code: errors.join("\n"),
        })
        .collect::<Vec<_>>();
    create_error_page_response(
        ctx,
        StatusCode::INTERNAL_SERVER_ERROR,
        "Failed to Upload Theme Files",
        "Upload Errors",
        page_errors,
    )
}

pub fn get_error_page(title: &str, header: &str, errors: &[ErrorPageError]) -> String {
    let error_blocks = errors
        .iter()
        .map(|error| {
            format!(
                r#"
                          <div>
                            <span class="Polaris-Text--root Polaris-Text--headingSm">{}</span>
                            <ul class="Polaris-List">
                              <li class="Polaris-List__Item">{}</li>
                            </ul>
                          </div>"#,
                escape_html(&error.message),
                escape_html(&error.code)
            )
        })
        .collect::<Vec<_>>()
        .join("");
    // Placeholders avoid format! interpreting CSS `{...}` custom properties.
    const TEMPLATE: &str = r#"<!DOCTYPE html>
    <html>
      <head>
        <title>__TITLE__</title>
        <link rel="stylesheet" href="__POLARIS_STYLESHEET_URL__" />
      </head>
      <body>
        <div style="display: flex; justify-content: center; padding-top: 2rem;">
          <div style="width: 80%;">
            <div class="Polaris-Banner Polaris-Banner--withinPage" tabindex="0" role="alert" aria-live="polite">
              <div class="Polaris-Box" style="--pc-box-width:100%">
                <div
                  class="Polaris-BlockStack"
                  style="--pc-block-stack-align:space-between;--pc-block-stack-order:column"
                >
                  <div
                    class="Polaris-Box"
                    style="--pc-box-color: var(--p-color-text-critical-on-bg-fill); --pc-box-background: var(--p-color-bg-fill-critical); --pc-box-padding-block-start-xs: var(--p-space-300); --pc-box-padding-block-end-xs: var(--p-space-300); --pc-box-padding-inline-start-xs: var(--p-space-300); --pc-box-padding-inline-end-xs: var(--p-space-300); --pc-box-border-start-start-radius: var(--p-border-radius-300); --pc-box-border-start-end-radius: var(--p-border-radius-300);"
                  >
                    <div
                      class="Polaris-InlineStack"
                      style="--pc-inline-stack-align:space-between;--pc-inline-stack-block-align:center;--pc-inline-stack-wrap:nowrap;--pc-inline-stack-gap-xs:var(--p-space-200);--pc-inline-stack-flex-direction-xs:row"
                    >
                      <div
                        class="Polaris-InlineStack"
                        style="--pc-inline-stack-wrap:nowrap;--pc-inline-stack-gap-xs:var(--p-space-100);--pc-inline-stack-flex-direction-xs:row"
                      >
                        <span class="Polaris-Banner--textCriticalOnBgFill">
                          <span class="Polaris-Icon">
                            <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20">
                              <path d="M10 6a.75.75 0 0 1 .75.75v3.5a.75.75 0 0 1-1.5 0v-3.5a.75.75 0 0 1 .75-.75Z" />
                              <path d="M11 13a1 1 0 1 1-2 0 1 1 0 0 1 2 0Z" />
                              <path
                                fill-rule="evenodd"
                                d="M17 10a7 7 0 1 1-14 0 7 7 0 0 1 14 0Zm-1.5 0a5.5 5.5 0 1 1-11 0 5.5 5.5 0 0 1 11 0Z"
                              />
                            </svg>
                          </span>
                        </span>
                        <h2 class="Polaris-Text--root Polaris-Text--headingSm Polaris-Text--break">
                          __HEADER__
                        </h2>
                      </div>
                    </div>
                  </div>
                  <div
                    class="Polaris-Box"
                    style="--pc-box-padding-block-start-xs:var(--p-space-300);--pc-box-padding-block-end-xs:var(--p-space-300);--pc-box-padding-block-end-md:var(--p-space-400);--pc-box-padding-inline-start-xs:var(--p-space-300);--pc-box-padding-inline-start-md:var(--p-space-400);--pc-box-padding-inline-end-xs:var(--p-space-300);--pc-box-padding-inline-end-md:var(--p-space-400)"
                  >
                    <div
                      class="Polaris-BlockStack"
                      style="--pc-block-stack-order:column;--pc-block-stack-gap-xs:var(--p-space-300)"
                    >
                      __ERROR_BLOCKS__
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </body>
    </html>"#;
    TEMPLATE
        .replace("__TITLE__", &escape_html(title))
        .replace("__POLARIS_STYLESHEET_URL__", POLARIS_STYLESHEET_URL)
        .replace("__HEADER__", &escape_html(header))
        .replace("__ERROR_BLOCKS__", &error_blocks)
}

fn create_error_page_response(
    ctx: &DevServerContext,
    status: StatusCode,
    title: &str,
    header: &str,
    errors: Vec<ErrorPageError>,
) -> Response {
    let mut html = inject_hot_reload_script(
        &get_error_page(title, header, &errors),
        ctx.options.live_reload,
    );
    if ctx.options.standard_events_inspector {
        html = inject_standard_events_inspector(&html);
    }
    (
        status,
        [(
            CONTENT_TYPE,
            HeaderValue::from_static("text/html; charset=utf-8"),
        )],
        html,
    )
        .into_response()
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#039;")
}

/// Extracts the theme ID embedded in a rendered storefront's `Shopify.theme` object.
fn theme_id_from_html(html: &str) -> Option<i64> {
    let regex = Regex::new(r#"Shopify\.theme\s*=\s*\{[^}]*"id":\s*"?(\d+)"?(}|,)"#).ok()?;
    regex.captures(html)?.get(1)?.as_str().parse::<i64>().ok()
}

/// Recovers from a rendered theme ID mismatch by refreshing the dev session and
/// redirecting to the same page (mirrors upstream `html.ts`).
async fn handle_theme_id_mismatch(
    state: AppState,
    actual_theme_id: i64,
    browser: Option<&str>,
) -> Response {
    let expected_theme_id = state.ctx.theme.id;
    let redirects = state
        .theme_id_mismatch_redirects
        .fetch_add(1, Ordering::SeqCst)
        + 1;
    if redirects > MAX_THEME_ID_MISMATCH_REDIRECTS {
        eprintln!(
            "Theme ID mismatch: expected {expected_theme_id} but got {actual_theme_id}. Aborting dev server after {MAX_THEME_ID_MISMATCH_REDIRECTS} consecutive mismatches."
        );
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Theme ID mismatch: expected {expected_theme_id} but got {actual_theme_id}."),
        )
            .into_response();
    }
    let refreshed = match &state.refresh {
        Some(refresh) => Some(refresh().await),
        None => None,
    };
    if let Some(Ok(new_session)) = refreshed {
        *state.session.lock().expect("session state poisoned") = new_session;
    } else {
        let message = [
            format!("Theme ID mismatch: expected {expected_theme_id} but got {actual_theme_id}."),
            "This is likely related to an issue in upstream Shopify APIs.".into(),
            "Please try again in a few minutes and report this issue:".into(),
            "https://community.shopify.dev/c/shopify-cli-libraries/14".into(),
        ]
        .join("\n");
        return (StatusCode::INTERNAL_SERVER_ERROR, message).into_response();
    }
    let location = browser
        .map(mismatch_location)
        .unwrap_or_else(|| "/".to_string());
    let mut response = StatusCode::FOUND.into_response();
    if let Ok(value) = HeaderValue::from_str(&location) {
        response.headers_mut().insert(LOCATION, value);
    }
    response
}

/// Strips `__sfr*` query params to avoid infinite redirect loops (mirrors upstream).
fn mismatch_location(browser: &str) -> String {
    let (pathname, search) = match browser.split_once('?') {
        Some((pathname, search)) => (pathname, search),
        None => (browser, ""),
    };
    let filtered = url::form_urlencoded::parse(search.as_bytes())
        .filter(|(key, _)| !key.starts_with("__sfr"))
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join("&");
    if filtered.is_empty() {
        pathname.to_string()
    } else {
        format!("{pathname}?{filtered}")
    }
}

/// Whether a dev-server request should appear in the request log (mirrors
/// upstream `log-request-line.ts`).
pub fn should_log_request(path: &str) -> bool {
    const IGNORED_PREFIXES: [&str; 4] = ["/ext/cdn/", "/cdn/", "/checkouts", "/payments"];
    const IGNORED_EXTENSIONS: [&str; 4] = [".js", ".css", ".json", ".map"];
    if IGNORED_PREFIXES
        .iter()
        .any(|prefix| path.starts_with(prefix))
    {
        return false;
    }
    let pathname = path.split('?').next().unwrap_or(path);
    let extension = Path::new(pathname)
        .extension()
        .and_then(|extension| extension.to_str());
    !IGNORED_EXTENSIONS
        .iter()
        .any(|ignored| extension == Some(&ignored[1..]))
}

/// Logs a dev-server request line with a colored status (mirrors upstream).
pub fn log_request_line(path: &str, method: &str, status: u16, server_timing: Option<&str>) {
    if !should_log_request(path) {
        return;
    }
    let truncated = if path.len() > REQUEST_LOG_PATH_TRUNCATION_LIMIT {
        format!("{}...", &path[..REQUEST_LOG_PATH_TRUNCATION_LIMIT])
    } else {
        path.to_string()
    };
    let duration = server_timing
        .and_then(|value| {
            Regex::new(r"cfRequestDuration;dur=([\d.]+)")
                .ok()?
                .captures(value)?
                .get(1)?
                .as_str()
                .parse::<f64>()
                .ok()
        })
        .map(|millis| format!("{}ms", millis.round()))
        .unwrap_or_default();
    let status_text = match status {
        0..=299 => status.to_string().green(),
        300..=399 => status.to_string().yellow(),
        _ => status.to_string().red(),
    };
    println!(
        "• {} Request » {:>6} {} {} {}",
        chrono::Local::now().format("%H:%M:%S"),
        method,
        status_text,
        truncated,
        duration.dimmed(),
    );
}

/// Detects a stale rendered `.css`/`.js` query string (`?1234`) so the dev server
/// replaces it with a fresh timestamp (mirrors upstream `proxy.ts`).
fn is_stale_asset_query(path: &str, query: &str) -> bool {
    if query.is_empty() || !query.bytes().all(|byte| byte.is_ascii_digit()) {
        return false;
    }
    let pathname = path.split('?').next().unwrap_or(path);
    Regex::new(r"/assets/[^/]+\.(css|js)$")
        .map(|regex| regex.is_match(pathname))
        .unwrap_or(false)
}

/// A section group: maps a section name to its section type (mirrors upstream
/// `SectionGroup`).
pub type SectionGroup = BTreeMap<String, serde_json::Value>;

/// Reads the `.sections` map from a JSON file into `section_names_by_file`
/// (mirrors upstream `saveSectionsFromJson`).
///
/// Stores `(section_type, section_name)` pairs so that a rendered section ID
/// like `sections--1__header` can be matched against its template.
fn save_sections_from_json(
    section_names_by_file: &mut BTreeMap<String, Vec<(String, String)>>,
    file_key: &str,
    content: &str,
) {
    let json = serde_json::from_str::<serde_json::Value>(content).ok();
    let Some(sections) = json
        .as_ref()
        .and_then(|json| json.get("sections"))
        .and_then(|sections| sections.as_object())
    else {
        section_names_by_file.remove(file_key);
        return;
    };
    if file_key.starts_with("locales/") {
        section_names_by_file.remove(file_key);
        return;
    }
    let entries = sections
        .iter()
        .filter_map(|(name, section)| {
            let section_type = section.get("type").and_then(|value| value.as_str())?;
            Some((section_type.to_string(), name.to_string()))
        })
        .collect::<Vec<_>>();
    section_names_by_file.insert(file_key.to_string(), entries);
}

/// Collects the in-memory templates to send as `replace_templates` when
/// rendering, filtered by the current route and locale (mirrors upstream
/// `getInMemoryTemplates`).
pub fn get_in_memory_templates(
    files: &BTreeMap<String, ThemeAsset>,
    unsynced: &BTreeSet<String>,
    current_route: Option<&str>,
    locale: Option<&str>,
) -> BTreeMap<String, String> {
    let json_template_re = Regex::new(r"^templates/.+\.json$").ok();
    let filter_template = current_route.map(|route| {
        let route = route.trim_start_matches('/').trim_end_matches(".html");
        let name = if route.is_empty() { "index" } else { route };
        format!("templates/{name}.json")
    });
    let has_route_template = current_route.is_some()
        && filter_template
            .as_deref()
            .is_some_and(|key| files.contains_key(key));
    let locale_re = Regex::new(r"^locales/.+\.json$").ok();
    let has_locale = locale.is_some()
        && (files.contains_key(&format!("locales/{}.json", locale.unwrap_or_default()))
            || files.contains_key(&format!(
                "locales/{}.default.json",
                locale.unwrap_or_default()
            )));
    let mut in_memory = BTreeMap::new();
    for key in unsynced {
        if !needs_template_update(key) {
            continue;
        }
        if has_route_template
            && json_template_re
                .as_ref()
                .is_some_and(|regex| regex.is_match(key))
        {
            if key.as_str() != filter_template.as_deref().unwrap_or_default() {
                continue;
            }
        } else if locale_re.as_ref().is_some_and(|regex| regex.is_match(key)) {
            if has_locale {
                if !key.starts_with(&format!("locales/{}.", locale.unwrap_or_default())) {
                    continue;
                }
            } else if !key.contains(".default.") {
                continue;
            }
        }
        let content = files
            .get(key)
            .and_then(|asset| asset.value.clone())
            .unwrap_or_default();
        in_memory.insert(key.clone(), content);
    }
    in_memory
}

/// Builds the `replace_templates[...]` form body for a rendering POST
/// (mirrors upstream `storefrontReplaceTemplatesParams`).
pub fn storefront_replace_templates_params(
    replace_templates: &BTreeMap<String, String>,
    method: &str,
) -> String {
    storefront_replace_templates_params_with_extensions(replace_templates, &BTreeMap::new(), method)
}

/// Like [`storefront_replace_templates_params`] plus `replace_extension_templates[bucket][path]`.
pub fn storefront_replace_templates_params_with_extensions(
    replace_templates: &BTreeMap<String, String>,
    replace_extension_templates: &BTreeMap<String, String>,
    method: &str,
) -> String {
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    for (key, content) in replace_templates {
        serializer.append_pair(&format!("replace_templates[{key}]"), content);
    }
    for (key, content) in replace_extension_templates {
        let (bucket, path) = match key.split_once('/') {
            Some((bucket, path)) => (bucket, path),
            None => ("", key.as_str()),
        };
        serializer.append_pair(
            &format!("replace_extension_templates[{bucket}][{path}]"),
            content,
        );
    }
    serializer.append_pair("_method", method);
    serializer.finish()
}

/// Finds the names of the sections that should be reloaded when a file changes
/// (mirrors upstream `findSectionNamesToReload`).
fn find_section_names_to_reload(
    key: &str,
    files: &BTreeMap<String, ThemeAsset>,
    section_names_by_file: &BTreeMap<String, Vec<(String, String)>>,
) -> Vec<String> {
    let mut sections = BTreeSet::new();
    if key.ends_with(".json") {
        if let Some(content) = files.get(key).and_then(|asset| asset.value.as_deref()) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(content) {
                if let Some(sections_map) = json.get("sections").and_then(|value| value.as_object())
                {
                    sections.extend(sections_map.keys().cloned());
                }
            }
        }
    } else if let Some(section_id) = key
        .strip_prefix("sections/")
        .and_then(|key| key.strip_suffix(".liquid"))
    {
        for entries in section_names_by_file.values() {
            for (section_type, name) in entries {
                if section_type == section_id {
                    sections.insert(name.clone());
                }
            }
        }
    }
    sections.into_iter().collect()
}

pub fn parse_cookies(cookies: &str) -> BTreeMap<String, String> {
    cookies
        .split(';')
        .filter_map(|cookie| {
            let (key, value) = cookie.split_once('=')?;
            Some((key.trim().to_string(), value.trim().to_string()))
        })
        .filter(|(key, _)| !key.is_empty())
        .collect()
}

pub fn serialize_cookies(cookies: &BTreeMap<String, String>) -> String {
    cookies
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join("; ")
}

pub fn cookie_from_set_cookie(headers: &[String], name: &str) -> Option<String> {
    headers
        .iter()
        .find_map(|header| parse_cookies(header).remove(name))
}

pub fn storefront_session_headers(session: &DevServerSession) -> BTreeMap<String, String> {
    let mut headers = BTreeMap::new();
    if let Some(token) = &session.storefront_token {
        headers.insert("authorization".into(), format!("Bearer {token}"));
    }
    if session.theme_access_domain.is_some() {
        headers.insert("x-shopify-shop".into(), session.store_fqdn.clone());
        headers.insert("x-shopify-access-token".into(), session.admin_token.clone());
    }
    headers
}

fn build_cookie_header(session: &DevServerSession, headers: &HeaderMap) -> String {
    let mut cookies = headers
        .get("cookie")
        .and_then(|value| value.to_str().ok())
        .map(parse_cookies)
        .unwrap_or_default();
    cookies.extend(session.session_cookies.clone());
    serialize_cookies(&cookies)
}

async fn patch_response(
    state: AppState,
    response: Result<reqwest::Response, DevError>,
    html: bool,
    browser: Option<String>,
) -> Response {
    let response = match response {
        Ok(response) => response,
        Err(error @ DevError::HostnameMismatch { .. }) => {
            return (StatusCode::BAD_REQUEST, error.to_string()).into_response();
        }
        Err(_) => return (StatusCode::BAD_GATEWAY, "Failed to reach storefront").into_response(),
    };
    let status =
        StatusCode::from_u16(response.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let mut headers = response.headers().clone();
    update_session_cookies_from_headers(&state, &headers);
    patch_set_cookie_domains(&state, &mut headers);
    strip_response_headers(&mut headers);
    if let Some(link) = headers
        .get("link")
        .and_then(|value| value.to_str().ok())
        .map(|link| {
            inject_cdn_proxy(
                link,
                &state.ctx.session.store_fqdn,
                &state.files.lock().expect("file state poisoned"),
                &state.extension_files.lock().expect("ext files poisoned"),
                state.ctx.options.standard_events_dev_bundle,
            )
        })
    {
        headers.insert(
            "link",
            HeaderValue::from_str(&link).unwrap_or(HeaderValue::from_static("")),
        );
    }
    if let Some(location) = headers
        .get(LOCATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|location| localize_location(location).ok())
    {
        headers.insert(
            LOCATION,
            HeaderValue::from_str(&location).unwrap_or(HeaderValue::from_static("/")),
        );
    }
    if status.is_redirection() {
        let body = response.bytes().await.unwrap_or_default();
        let mut out = Response::new(Body::from(body));
        *out.status_mut() = status;
        *out.headers_mut() = headers;
        return out;
    }
    if html {
        let body = response.text().await.unwrap_or_default();
        if let Some(actual_theme_id) = theme_id_from_html(&body) {
            if actual_theme_id != state.ctx.theme.id {
                return handle_theme_id_mismatch(state, actual_theme_id, browser.as_deref()).await;
            }
        }
        state.theme_id_mismatch_redirects.store(0, Ordering::SeqCst);
        let body = patch_html(
            &body,
            &state.ctx,
            &state.files.lock().expect("file state poisoned"),
            &state.extension_files.lock().expect("ext files poisoned"),
        );
        let mut out = (status, body).into_response();
        let is_json = headers
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.contains("application/json"));
        *out.headers_mut() = headers;
        if !is_json {
            out.headers_mut().insert(
                CONTENT_TYPE,
                HeaderValue::from_static("text/html; charset=utf-8"),
            );
        }
        out
    } else {
        let body = response.bytes().await.unwrap_or_default();
        let mut out = Response::new(Body::from(body));
        *out.status_mut() = status;
        *out.headers_mut() = headers;
        out
    }
}

fn update_session_cookies_from_headers(state: &AppState, headers: &HeaderMap) {
    let set_cookies = headers
        .get_all("set-cookie")
        .iter()
        .filter_map(|value| value.to_str().ok())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    let mut session = state.session.lock().expect("session state poisoned");
    for name in ["_shopify_essential", "storefront_digest"] {
        if let Some(value) = cookie_from_set_cookie(&set_cookies, name) {
            session.session_cookies.insert(name.into(), value);
        }
    }
}

fn patch_set_cookie_domains(state: &AppState, headers: &mut HeaderMap) {
    let store = state
        .session
        .lock()
        .expect("session state poisoned")
        .store_fqdn
        .clone();
    let set_cookies = headers
        .get_all("set-cookie")
        .iter()
        .filter_map(|value| value.to_str().ok())
        .map(|value| {
            Regex::new(&format!(r"(?i)Domain={};\s*", regex_lite::escape(&store)))
                .map(|regex| regex.replace_all(value, "").to_string())
                .unwrap_or_else(|_| value.to_string())
        })
        .collect::<Vec<_>>();
    if set_cookies.is_empty() {
        return;
    }
    headers.remove("set-cookie");
    for value in set_cookies {
        if let Ok(value) = HeaderValue::from_str(&value) {
            headers.append("set-cookie", value);
        }
    }
}

fn strip_response_headers(headers: &mut HeaderMap) {
    for header in [
        "connection",
        "keep-alive",
        "proxy-authenticate",
        "proxy-authorization",
        "te",
        "trailer",
        "transfer-encoding",
        "upgrade",
        "expect",
        "content-security-policy",
        "content-length",
        "content-encoding",
        "access-control-allow-origin",
        "access-control-allow-credentials",
        "access-control-allow-methods",
        "access-control-allow-headers",
        "access-control-expose-headers",
        "access-control-max-age",
    ] {
        headers.remove(header);
    }
}

fn localize_location(location: &str) -> Result<String, url::ParseError> {
    let mut url =
        Url::parse(location).or_else(|_| Url::parse(&format!("https://shopify.dev{location}")))?;
    if url.path().starts_with("/checkouts/") {
        return Ok(location.into());
    }
    let query_pairs = url
        .query()
        .map(|query| {
            url::form_urlencoded::parse(query.as_bytes())
                .filter(|(key, _)| key != "_fd" && key != "pb")
                .map(|(key, value)| (key.into_owned(), value.into_owned()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    url.query_pairs_mut().clear().extend_pairs(query_pairs);
    let mut path = url.path().to_string();
    if let Some(query) = url.query() {
        path.push('?');
        path.push_str(query);
    }
    Ok(path)
}

pub fn patch_html(
    html: &str,
    ctx: &DevServerContext,
    files: &BTreeMap<String, ThemeAsset>,
    extension_files: &BTreeMap<String, ThemeAsset>,
) -> String {
    let content = inject_cdn_proxy(
        html,
        &ctx.session.store_fqdn,
        files,
        extension_files,
        ctx.options.standard_events_dev_bundle,
    );
    let content = patch_base_url_attributes(&content, &local_origin(ctx));
    let content = if ctx.options.standard_events_inspector {
        inject_standard_events_inspector(&content)
    } else {
        content
    };
    inject_hot_reload_script(&content, ctx.options.live_reload)
}

pub fn inject_cdn_proxy(
    content: &str,
    store_fqdn: &str,
    files: &BTreeMap<String, ThemeAsset>,
    extension_files: &BTreeMap<String, ThemeAsset>,
    standard_events_dev_bundle: bool,
) -> String {
    let escaped_store = regex_lite::escape(store_fqdn);
    let mut output = content
        .replace(
            &format!(r#"data-shs-beacon-endpoint="https://{store_fqdn}/api/collect"#),
            r#"data-shs-beacon-endpoint="/api/collect"#,
        )
        .replace(
            &format!("data-shs-beacon-endpoint='https://{store_fqdn}/api/collect"),
            "data-shs-beacon-endpoint='/api/collect",
        );
    output = Regex::new(&format!(r#"(https?:)?//{escaped_store}/cdn/"#))
        .map(|regex| regex.replace_all(&output, "/cdn/").to_string())
        .unwrap_or(output);
    let known_assets: BTreeSet<_> = files
        .keys()
        .filter(|key| key.starts_with("assets/"))
        .cloned()
        .collect();
    let known_ext_assets: BTreeSet<_> = extension_files
        .keys()
        .filter(|key| key.starts_with("assets/"))
        .cloned()
        .collect();
    let main_cdn =
        Regex::new(r#"(?:(?:https?:)?//cdn\.shopify\.com/)(.*?/(assets/[^?#"'`>\s]+))"#).unwrap();
    output = main_cdn
        .replace_all(&output, |captures: &regex_lite::Captures<'_>| {
            let matched = captures.get(0).unwrap().as_str();
            let path = captures.get(1).unwrap().as_str();
            let asset = captures.get(2).unwrap().as_str();
            let is_image = mime_guess::from_path(asset)
                .first()
                .is_some_and(|mime| mime.type_().as_str() == "image");
            if is_image {
                return matched.to_string();
            }
            if known_ext_assets.contains(asset) && path.starts_with("extensions/") {
                return format!("{EXTENSION_CDN_PREFIX}{path}");
            }
            if known_assets.contains(asset) {
                return format!("/cdn/{path}");
            }
            matched.to_string()
        })
        .to_string();
    if standard_events_dev_bundle {
        output = rewrite_standard_events_runtime_references(&output);
    }
    output
}

fn patch_base_url_attributes(html: &str, local: &str) -> String {
    Regex::new(r#"data-base-url=["'](?:https?:)?//[^"']+["']"#)
        .map(|regex| {
            regex
                .replace_all(html, |captures: &regex_lite::Captures<'_>| {
                    let matched = captures.get(0).unwrap().as_str();
                    let quote = if matched.contains("='") { "'" } else { "\"" };
                    format!("data-base-url={quote}{local}{quote}")
                })
                .to_string()
        })
        .unwrap_or_else(|_| html.to_string())
}

pub fn inject_hot_reload_script(html: &str, mode: LiveReloadMode) -> String {
    let without = Regex::new(&format!(
        r#"<script id="{HOT_RELOAD_SCRIPT_ID}"[^>]*>[^<]*</script>"#
    ))
    .map(|regex| regex.replace_all(html, "").to_string())
    .unwrap_or_else(|_| html.to_string());
    if mode == LiveReloadMode::Off {
        return without;
    }
    let script_url = if std::env::var("SHOPIFY_CLI_LOCAL_HOT_RELOAD").is_ok() {
        LOCAL_HOT_RELOAD_SCRIPT_ENDPOINT
    } else {
        HOT_RELOAD_SCRIPT_URL
    };
    if without.contains(&format!(r#"<script id="{HOT_RELOAD_SCRIPT_ID}""#)) {
        return without;
    }
    without.replace(
        "</head>",
        &format!(
            r#"<script id="{HOT_RELOAD_SCRIPT_ID}" src="{script_url}" defer></script></head>"#
        ),
    )
}

fn inject_standard_events_inspector(html: &str) -> String {
    let inspector = format!(
        r#"<script id="{STANDARD_EVENTS_INSPECTOR_SCRIPT_ID}" src="{STANDARD_EVENTS_INSPECTOR_URL}" defer></script>"#
    );
    let existing = Regex::new(&format!(
        r#"<script\b[^>]*(?:\bid=["']{}["']|\bsrc=["']{}["'])[^>]*>"#,
        regex_lite::escape(STANDARD_EVENTS_INSPECTOR_SCRIPT_ID),
        regex_lite::escape(STANDARD_EVENTS_INSPECTOR_URL)
    ))
    .map(|regex| regex.is_match(html))
    .unwrap_or(false);
    if existing {
        return html.to_string();
    }
    Regex::new(r"(?i)<head(\s[^>]*)?>")
        .map(|regex| {
            regex
                .replace(html, |captures: &regex_lite::Captures<'_>| {
                    format!("{}{}", captures.get(0).unwrap().as_str(), inspector)
                })
                .to_string()
        })
        .unwrap_or_else(|_| html.to_string())
}

fn rewrite_standard_events_runtime_references(content: &str) -> String {
    content.replace(STANDARD_EVENTS_RUNTIME_URL, STANDARD_EVENTS_RUNTIME_DEV_URL)
}

fn is_hop_by_hop(header: &str) -> bool {
    matches!(
        header.to_ascii_lowercase().as_str(),
        "connection"
            | "keep-alive"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "te"
            | "trailer"
            | "transfer-encoding"
            | "upgrade"
            | "expect"
            | "content-security-policy"
            | "host"
    )
}

fn decode_path(path: &str) -> String {
    percent_decode_str(path).decode_utf8_lossy().to_string()
}

fn socket_addr(host: &str, port: u16) -> SocketAddr {
    host.parse::<IpAddr>()
        .map(|ip| SocketAddr::new(ip, port))
        .unwrap_or_else(|_| SocketAddr::from(([127, 0, 0, 1], port)))
}

pub async fn push_initial<A: ThemeSyncAdmin + Sync>(
    api: &A,
    theme_id: i64,
    filesystem: &ThemeFileSystem,
    options: &DevServerOptions,
) -> Result<sync::SyncReport, DevError> {
    crate::uploader::upload_theme(
        api,
        theme_id,
        filesystem,
        &SyncOptions {
            nodelete: options.nodelete,
            filters: options.filters.clone(),
        },
    )
    .await
    .map(|report| report.sync)
    .map_err(DevError::Sync)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(mode: LiveReloadMode) -> DevServerContext {
        DevServerContext {
            options: DevServerOptions {
                root: PathBuf::new(),
                host: "127.0.0.1".into(),
                port: 9292,
                explicit_port: false,
                live_reload: mode,
                error_overlay: ErrorOverlayMode::Default,
                poll: false,
                theme_editor_sync: false,
                standard_events_dev_bundle: false,
                standard_events_inspector: false,
                nodelete: false,
                filters: IgnoreFilters::default(),
                notify: None,
                store_password: None,
            },
            session: DevServerSession {
                store_fqdn: "example.myshopify.com".into(),
                admin_token: "admin".into(),
                storefront_token: Some("sfr".into()),
                theme_access_domain: None,
                session_cookies: BTreeMap::new(),
            },
            theme: DevServerTheme {
                id: 1,
                name: "Development".into(),
                role: "development".into(),
            },
            kind: DevServerKind::Theme,
        }
    }

    fn asset(key: &str, checksum: &str) -> ThemeAsset {
        ThemeAsset {
            key: key.into(),
            checksum: checksum.into(),
            attachment: None,
            value: Some("{}".into()),
            stats: None,
        }
    }

    fn value_asset(key: &str, value: &str) -> ThemeAsset {
        ThemeAsset {
            key: key.into(),
            checksum: "1".into(),
            attachment: None,
            value: Some(value.into()),
            stats: None,
        }
    }

    fn state_with_files(files: BTreeMap<String, ThemeAsset>) -> AppState {
        let ctx = Arc::new(ctx(LiveReloadMode::HotReload));
        let (reload_tx, _) = broadcast::channel(4);
        AppState {
            session: Arc::new(Mutex::new(ctx.session.clone())),
            ctx,
            files: Arc::new(Mutex::new(files)),
            watch: ThemeWatchState::default(),
            last_requested_path: Arc::new(Mutex::new(String::new())),
            reload_tx,
            client: reqwest::Client::new(),
            refresh: None,
            theme_id_mismatch_redirects: Arc::new(AtomicUsize::new(0)),
            section_names_by_file: Arc::new(Mutex::new(BTreeMap::new())),
            file_details_cache: Arc::new(Mutex::new(BTreeMap::new())),
            extension_files: Arc::new(Mutex::new(BTreeMap::new())),
            extension_unsynced: Arc::new(Mutex::new(BTreeSet::new())),
        }
    }

    fn headers(values: &[(&'static str, &'static str)]) -> HeaderMap {
        let mut headers = HeaderMap::new();
        for (key, value) in values {
            headers.insert(*key, HeaderValue::from_static(value));
        }
        headers
    }

    #[test]
    fn validates_modes() {
        assert_eq!(
            LiveReloadMode::parse("hot-reload").unwrap(),
            LiveReloadMode::HotReload
        );
        assert_eq!(
            LiveReloadMode::parse("full-page").unwrap(),
            LiveReloadMode::FullPage
        );
        assert_eq!(LiveReloadMode::parse("off").unwrap(), LiveReloadMode::Off);
        assert!(LiveReloadMode::parse("bad").is_err());
        assert_eq!(
            ErrorOverlayMode::parse("default").unwrap(),
            ErrorOverlayMode::Default
        );
        assert_eq!(
            ErrorOverlayMode::parse("silent").unwrap(),
            ErrorOverlayMode::Silent
        );
        assert!(ErrorOverlayMode::parse("loud").is_err());
    }

    #[test]
    fn validates_hosts() {
        assert_eq!(validate_host("127.0.0.1").unwrap(), "127.0.0.1");
        assert_eq!(validate_host("localhost").unwrap(), "localhost");
        assert!(validate_host("").is_err());
        assert!(validate_host("local host").is_err());
        assert!(allowed_hosts("127.0.0.1", 9292).contains("localhost:9292"));
        let wildcard = allowed_hosts("0.0.0.0", 9292);
        assert!(wildcard.contains("0.0.0.0:9292"));
        assert!(wildcard.contains("127.0.0.1:9292"));
        assert!(wildcard.contains("localhost:9292"));
        assert!(wildcard
            .iter()
            .any(|host| host.ends_with(":9292") && host != "0.0.0.0:9292"));
    }

    #[test]
    fn editor_preview_path_uses_ampersand_query_separator() {
        let urls = build_urls(&ctx(LiveReloadMode::HotReload));
        assert!(urls.editor.contains("?hr="));
        let with_path = format!(
            "{}&previewPath={}",
            urls.editor,
            url::form_urlencoded::byte_serialize(b"/products/foo").collect::<String>()
        );
        assert!(with_path.contains("editor?hr="));
        assert!(with_path.contains("&previewPath=%2Fproducts%2Ffoo"));
        assert!(!with_path.contains("?previewPath="));
    }

    #[test]
    fn classifies_proxy_requests() {
        let html = HeaderMap::from_iter([(ACCEPT, HeaderValue::from_static("text/html"))]);
        let json = HeaderMap::from_iter([(ACCEPT, HeaderValue::from_static("application/json"))]);
        assert!(!can_proxy_request(
            &Method::GET,
            &"/".parse().unwrap(),
            &html
        ));
        assert!(can_proxy_request(
            &Method::GET,
            &"/cart/add.js".parse().unwrap(),
            &html
        ));
        assert!(can_proxy_request(
            &Method::POST,
            &"/".parse().unwrap(),
            &html
        ));
        assert!(can_proxy_request(
            &Method::GET,
            &"/payments/config".parse().unwrap(),
            &json
        ));
        assert!(can_proxy_request(
            &Method::GET,
            &"/api/2024-10/graphql.json".parse().unwrap(),
            &html
        ));
        assert!(can_proxy_request(
            &Method::GET,
            &"/account".parse().unwrap(),
            &html
        ));
        assert!(!can_proxy_request(
            &Method::GET,
            &"/account/login".parse().unwrap(),
            &html
        ));
        assert!(can_proxy_request(
            &Method::GET,
            &"/account/login/multipass/token".parse().unwrap(),
            &html
        ));
    }

    #[test]
    fn builds_proxy_urls_with_dev_params_and_storefront_api_passthrough() {
        let ctx = ctx(LiveReloadMode::HotReload);
        let product = proxy_storefront_url(
            &ctx,
            "/products/snowboard",
            &BTreeMap::from([("view".into(), "quick".into())]),
        )
        .unwrap();
        assert_eq!(
            product.as_str(),
            "https://example.myshopify.com/products/snowboard?view=quick&_fd=0&pb=0"
        );

        let api =
            proxy_storefront_url(&ctx, "/api/2026-01/graphql.json", &BTreeMap::new()).unwrap();
        assert_eq!(
            api.as_str(),
            "https://example.myshopify.com/api/2026-01/graphql.json"
        );

        let non_matching_api =
            proxy_storefront_url(&ctx, "/api/2026-01/graphql.js", &BTreeMap::new()).unwrap();
        assert_eq!(
            non_matching_api.as_str(),
            "https://example.myshopify.com/api/2026-01/graphql.js?_fd=0&pb=0"
        );
    }

    #[test]
    fn builds_proxy_urls_for_theme_access_and_extension_cdn() {
        let mut ctx = ctx(LiveReloadMode::HotReload);
        ctx.session.theme_access_domain = Some("theme-access.shopifyapps.com".into());

        let theme_access =
            proxy_storefront_url(&ctx, "/products/snowboard", &BTreeMap::new()).unwrap();
        assert_eq!(
            theme_access.as_str(),
            "https://theme-access.shopifyapps.com/cli/sfr/products/snowboard?_fd=0&pb=0"
        );

        let extension = proxy_storefront_url(
            &ctx,
            "/ext/cdn/extensions/uuid/app/assets/app.js",
            &BTreeMap::new(),
        )
        .unwrap();
        assert_eq!(
            extension.as_str(),
            "https://cdn.shopify.com/extensions/uuid/app/assets/app.js?_fd=0&pb=0"
        );
    }

    #[test]
    fn rejects_proxy_urls_with_hostname_mismatch() {
        let ctx = ctx(LiveReloadMode::HotReload);

        let error =
            proxy_storefront_url(&ctx, "//evil.com/some-path", &BTreeMap::new()).unwrap_err();
        assert_eq!(
            error.to_string(),
            "Request failed: Hostname mismatch. Expected host: example.myshopify.com. Resulting URL hostname: evil.com"
        );

        let error = proxy_storefront_url(&ctx, "/ext/cdn//evil.com/some-path", &BTreeMap::new())
            .unwrap_err();
        assert_eq!(
            error.to_string(),
            "Request failed: Hostname mismatch. Expected host: cdn.shopify.com. Resulting URL hostname: evil.com"
        );
    }

    #[test]
    fn filters_proxy_headers_and_adds_forwarded_for() {
        let filtered = proxy_storefront_headers(
            &headers(&[
                ("connection", "close"),
                ("proxy-authenticate", "secret"),
                ("host", "local"),
                ("upgrade-insecure-requests", "1"),
                ("accept", "text/html"),
                ("cookie", "oreo"),
                ("user-agent", "theme-test"),
                ("x-custom", "true"),
            ]),
            Some("42"),
        );

        assert!(filtered.get("connection").is_none());
        assert!(filtered.get("proxy-authenticate").is_none());
        assert!(filtered.get("host").is_none());
        assert!(filtered.get("upgrade-insecure-requests").is_none());
        assert_eq!(filtered.get("accept").unwrap(), "text/html");
        assert_eq!(filtered.get("cookie").unwrap(), "oreo");
        assert_eq!(filtered.get("user-agent").unwrap(), "theme-test");
        assert_eq!(filtered.get("x-custom").unwrap(), "true");
        assert_eq!(filtered.get("x-forwarded-for").unwrap(), "42");
    }

    #[test]
    fn localizes_redirect_locations_and_strips_dev_params() {
        assert_eq!(
            localize_location("https://example.myshopify.com/foo?bar=1&_fd=0&pb=0").unwrap(),
            "/foo?bar=1"
        );
        assert_eq!(
            localize_location("https://example.myshopify.com/checkouts/abc?bar=1").unwrap(),
            "https://example.myshopify.com/checkouts/abc?bar=1"
        );
    }

    #[test]
    fn rewrites_known_cdn_assets_but_not_images() {
        let mut files = BTreeMap::new();
        files.insert(
            "assets/app.js".into(),
            ThemeAsset {
                key: "assets/app.js".into(),
                checksum: "1".into(),
                attachment: None,
                value: Some("".into()),
                stats: None,
            },
        );
        files.insert(
            "assets/logo.png".into(),
            ThemeAsset {
                key: "assets/logo.png".into(),
                checksum: "1".into(),
                attachment: None,
                value: Some("".into()),
                stats: None,
            },
        );
        let html = r#"https://example.myshopify.com/cdn/theme/assets/app.js //cdn.shopify.com/s/files/1/assets/app.js //cdn.shopify.com/s/files/1/assets/logo.png"#;
        let patched = inject_cdn_proxy(
            html,
            "example.myshopify.com",
            &files,
            &BTreeMap::new(),
            false,
        );
        assert!(patched.contains("/cdn/theme/assets/app.js"));
        assert!(patched.contains("/cdn/s/files/1/assets/app.js"));
        assert!(patched.contains("//cdn.shopify.com/s/files/1/assets/logo.png"));
    }

    #[test]
    fn rewrites_cdn_urls_in_javascript_link_headers_and_beacon_attributes() {
        let mut files = BTreeMap::new();
        files.insert(
            "assets/app.js".into(),
            ThemeAsset {
                key: "assets/app.js".into(),
                checksum: "1".into(),
                attachment: None,
                value: Some("".into()),
                stats: None,
            },
        );
        let content = r#"
            console.log('https://cdn.shopify.com/path/to/assets/app.js');
            const url = "https://cdn.shopify.com/path/to/assets/app.js#hash";
            <https://example.myshopify.com/cdn/shop/t/10/assets/app.js?v=1>; as="script"; rel="preload"
            <div data-shs-beacon-endpoint='https://example.myshopify.com/api/collect'></div>
        "#;
        let patched = inject_cdn_proxy(
            content,
            "example.myshopify.com",
            &files,
            &BTreeMap::new(),
            false,
        );

        assert!(patched.contains("console.log('/cdn/path/to/assets/app.js');"));
        assert!(patched.contains("\"/cdn/path/to/assets/app.js#hash\""));
        assert!(patched.contains("</cdn/shop/t/10/assets/app.js?v=1>; as=\"script\""));
        assert!(patched.contains("data-shs-beacon-endpoint='/api/collect'"));
    }

    #[test]
    fn rewrites_standard_events_runtime_only_when_dev_bundle_enabled() {
        let content =
            format!(r#""{STANDARD_EVENTS_RUNTIME_URL}" import("{STANDARD_EVENTS_RUNTIME_URL}")"#);

        let unchanged = inject_cdn_proxy(
            &content,
            "example.myshopify.com",
            &BTreeMap::new(),
            &BTreeMap::new(),
            false,
        );
        let rewritten = inject_cdn_proxy(
            &content,
            "example.myshopify.com",
            &BTreeMap::new(),
            &BTreeMap::new(),
            true,
        );

        assert!(unchanged.contains(STANDARD_EVENTS_RUNTIME_URL));
        assert!(!rewritten.contains(STANDARD_EVENTS_RUNTIME_URL));
        assert_eq!(
            rewritten.matches(STANDARD_EVENTS_RUNTIME_DEV_URL).count(),
            2
        );
    }

    #[test]
    fn local_cdn_asset_matching_skips_extension_paths() {
        assert_eq!(
            local_theme_asset_key_for_cdn_path("shop/t/12/assets/app.js?v=1").as_deref(),
            Some("assets/app.js")
        );
        assert_eq!(
            local_theme_asset_key_for_cdn_path(
                "extensions/019e1813-804f-7f97-ad2d-278904fdd92f/app/assets/app.js"
            ),
            None
        );
    }

    #[test]
    fn is_known_rendering_request_detects_section_params() {
        assert!(is_known_rendering_request(&BTreeMap::from([(
            "section_id".into(),
            "header".into()
        )])));
        assert!(is_known_rendering_request(&BTreeMap::from([(
            "sections".into(),
            "a,b".into()
        )])));
        assert!(is_known_rendering_request(&BTreeMap::from([(
            "app_block_id".into(),
            "block".into()
        )])));
        assert!(!is_known_rendering_request(&BTreeMap::from([(
            "preview_theme_id".into(),
            "1".into()
        )])));
        assert!(!is_known_rendering_request(&BTreeMap::new()));
    }

    #[test]
    fn get_error_page_contains_polaris_and_escapes_html() {
        let page = get_error_page(
            "Title <x>",
            "Header 'y'",
            &[ErrorPageError {
                message: "msg & <bad>".into(),
                code: "code \"quoted\"".into(),
            }],
        );
        assert!(page.contains(POLARIS_STYLESHEET_URL));
        assert!(page.contains("Polaris-Banner"));
        assert!(page.contains("Title &lt;x&gt;"));
        assert!(page.contains("Header &#039;y&#039;"));
        assert!(page.contains("msg &amp; &lt;bad&gt;"));
        assert!(page.contains("code &quot;quoted&quot;"));
    }

    #[tokio::test]
    async fn create_error_page_response_injects_hot_reload_script() {
        let mut context = ctx(LiveReloadMode::HotReload);
        context.options.standard_events_inspector = true;
        let response = create_error_page_response(
            &context,
            StatusCode::BAD_GATEWAY,
            "Failed",
            "Failed",
            vec![ErrorPageError {
                message: "oops".into(),
                code: "stack".into(),
            }],
        );
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let html = String::from_utf8(body.to_vec()).unwrap();
        assert!(html.contains(HOT_RELOAD_SCRIPT_ID));
        assert!(html.contains(STANDARD_EVENTS_INSPECTOR_SCRIPT_ID));
        assert!(html.contains(POLARIS_STYLESHEET_URL));
    }

    #[test]
    fn inject_cdn_proxy_rewrites_local_extension_assets() {
        let mut extension_files = BTreeMap::new();
        extension_files.insert(
            "assets/file-ext".into(),
            ThemeAsset {
                key: "assets/file-ext".into(),
                checksum: "1".into(),
                attachment: None,
                value: Some("".into()),
                stats: None,
            },
        );
        let content = r#"
            <script src="https://cdn.shopify.com/extensions/1aaaa11a-2b22-333c-4444-ee55555e55ee/0.0.0/assets/file-ext"></script>
            <link href="https://cdn.shopify.com/extensions/1aaaa11a-2b22-333c-0000-ee55555e55ee/0.1.0/file2"></link>
        "#;
        let patched = inject_cdn_proxy(
            content,
            "example.myshopify.com",
            &BTreeMap::new(),
            &extension_files,
            false,
        );
        assert!(patched.contains(
            "/ext/cdn/extensions/1aaaa11a-2b22-333c-4444-ee55555e55ee/0.0.0/assets/file-ext"
        ));
        assert!(patched.contains(
            "https://cdn.shopify.com/extensions/1aaaa11a-2b22-333c-0000-ee55555e55ee/0.1.0/file2"
        ));
    }

    #[test]
    fn local_extension_asset_key_for_cdn_path_extracts_assets_name() {
        assert_eq!(
            local_extension_asset_key_for_cdn_path(
                "extensions/019e1813-804f-7f97-ad2d-278904fdd92f/my-app/assets/app.js"
            )
            .as_deref(),
            Some("assets/app.js")
        );
        assert_eq!(
            local_extension_asset_key_for_cdn_path(
                "ext/cdn/extensions/019e1813-804f-7f97-ad2d-278904fdd92f/my-app/assets/app.js?v=1"
            )
            .as_deref(),
            Some("assets/app.js")
        );
        assert_eq!(
            local_extension_asset_key_for_cdn_path("shop/t/12/assets/app.js"),
            None
        );
    }

    #[test]
    fn should_send_storefront_bearer_only_for_theme_servers() {
        assert!(should_send_storefront_bearer(DevServerKind::Theme));
        assert!(!should_send_storefront_bearer(
            DevServerKind::ThemeExtension
        ));
    }

    #[tokio::test]
    async fn serves_text_local_assets_with_cdn_rewrites() {
        let mut files = BTreeMap::new();
        files.insert(
            "assets/app.js".into(),
            ThemeAsset {
                key: "assets/app.js".into(),
                checksum: "1".into(),
                attachment: None,
                value: Some(
                    "console.log('https://example.myshopify.com/cdn/shop/t/1/assets/app.js');"
                        .into(),
                ),
                stats: None,
            },
        );
        let response = local_asset_response(&state_with_files(files), "assets/app.js").unwrap();
        assert_eq!(
            response.headers().get(CONTENT_TYPE).unwrap(),
            "text/javascript"
        );
        assert_eq!(response.headers().get("x-local-asset").unwrap(), "true");
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();

        assert_eq!(
            std::str::from_utf8(&body).unwrap(),
            "console.log('/cdn/shop/t/1/assets/app.js');"
        );
    }

    #[tokio::test]
    async fn serves_text_local_assets_with_utf8_content_length() {
        let content = "const x = \"Hello\u{00a0}World\";";
        let mut files = BTreeMap::new();
        files.insert(
            "assets/file-with-nbsp.js".into(),
            ThemeAsset {
                key: "assets/file-with-nbsp.js".into(),
                checksum: "1".into(),
                attachment: None,
                value: Some(content.into()),
                stats: None,
            },
        );
        let response =
            local_asset_response(&state_with_files(files), "assets/file-with-nbsp.js").unwrap();

        assert_eq!(
            response.headers().get(CONTENT_LENGTH).unwrap(),
            &HeaderValue::from_str(&content.len().to_string()).unwrap()
        );
    }

    #[tokio::test]
    async fn serves_binary_local_assets_from_attachments() {
        let bytes = vec![0, 1, 2, 3];
        let mut files = BTreeMap::new();
        files.insert(
            "assets/logo.png".into(),
            ThemeAsset {
                key: "assets/logo.png".into(),
                checksum: "1".into(),
                attachment: Some(base64::Engine::encode(
                    &base64::engine::general_purpose::STANDARD,
                    &bytes,
                )),
                value: None,
                stats: None,
            },
        );
        let response = local_asset_response(&state_with_files(files), "assets/logo.png").unwrap();
        assert_eq!(response.headers().get(CONTENT_TYPE).unwrap(), "image/png");
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();

        assert_eq!(body.as_ref(), bytes.as_slice());
    }

    #[tokio::test]
    async fn serves_compiled_stylesheets_from_liquid_files() {
        let mut files = BTreeMap::new();
        files.insert(
            "sections/test-section.liquid".into(),
            ThemeAsset {
                key: "sections/test-section.liquid".into(),
                checksum: "1".into(),
                attachment: None,
                value: Some("{% stylesheet %}.section { color: red; }{% endstylesheet %}".into()),
                stats: None,
            },
        );
        files.insert(
            "blocks/test-block.liquid".into(),
            ThemeAsset {
                key: "blocks/test-block.liquid".into(),
                checksum: "1".into(),
                attachment: None,
                value: Some("{% stylesheet %}.block { color: blue; }{% endstylesheet %}".into()),
                stats: None,
            },
        );
        files.insert(
            "snippets/test-snippet.liquid".into(),
            ThemeAsset {
                key: "snippets/test-snippet.liquid".into(),
                checksum: "1".into(),
                attachment: None,
                value: Some("{% stylesheet %}.snippet { color: green; }{% endstylesheet %}".into()),
                stats: None,
            },
        );

        let response = compiled_asset_response(&state_with_files(files), "styles.css").unwrap();
        assert_eq!(response.headers().get(CONTENT_TYPE).unwrap(), "text/css");
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body = std::str::from_utf8(&body).unwrap();

        assert!(body.contains("/*** GENERATED LOCALLY ***/"));
        assert!(body.contains("/* sections/test-section.liquid */"));
        assert!(body.contains(".section { color: red; }"));
        assert!(body.contains(".block { color: blue; }"));
        assert!(body.contains(".snippet { color: green; }"));
    }

    #[tokio::test]
    async fn serves_compiled_scripts_from_liquid_files() {
        let mut files = BTreeMap::new();
        files.insert(
            "blocks/another-block.liquid".into(),
            ThemeAsset {
                key: "blocks/another-block.liquid".into(),
                checksum: "1".into(),
                attachment: None,
                value: Some("{% javascript %}console.log('another');{% endjavascript %}".into()),
                stats: None,
            },
        );
        files.insert(
            "blocks/no-js-block.liquid".into(),
            ThemeAsset {
                key: "blocks/no-js-block.liquid".into(),
                checksum: "1".into(),
                attachment: None,
                value: Some("<div>No JavaScript</div>".into()),
                stats: None,
            },
        );
        files.insert(
            "blocks/test-block.liquid".into(),
            ThemeAsset {
                key: "blocks/test-block.liquid".into(),
                checksum: "1".into(),
                attachment: None,
                value: Some("{% javascript %}console.log('test');{% endjavascript %}".into()),
                stats: None,
            },
        );

        let response = compiled_asset_response(
            &state_with_files(files),
            "path/compiled_assets/block-scripts.js",
        )
        .unwrap();
        assert_eq!(
            response.headers().get(CONTENT_TYPE).unwrap(),
            "text/javascript"
        );
        let content_length = response
            .headers()
            .get(CONTENT_LENGTH)
            .unwrap()
            .to_str()
            .unwrap()
            .parse::<usize>()
            .unwrap();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body = std::str::from_utf8(&body).unwrap();

        assert_eq!(content_length, body.len());
        assert!(body.contains("var __blocks__ = {};"));
        assert!(body.contains(r#"document.getElementById("blocks-script")"#));
        assert!(body.contains("/* blocks/another-block.liquid */"));
        assert!(body.contains("console.log('another');"));
        assert!(body.contains("/* blocks/test-block.liquid */"));
        assert!(body.contains("console.log('test');"));
        assert!(!body.contains("no-js-block"));
    }

    #[test]
    fn unknown_compiled_assets_fall_through_to_proxy() {
        assert!(compiled_asset_response(
            &state_with_files(BTreeMap::new()),
            "compiled_assets/nonexistent.js"
        )
        .is_none());
    }

    #[test]
    fn injects_and_removes_hot_reload_script() {
        let html = "<html><head></head><body></body></html>";
        let injected = inject_hot_reload_script(html, LiveReloadMode::HotReload);
        assert!(injected.contains(HOT_RELOAD_SCRIPT_ID));
        let removed = inject_hot_reload_script(&injected, LiveReloadMode::Off);
        assert!(!removed.contains(HOT_RELOAD_SCRIPT_ID));
    }

    #[test]
    fn injects_standard_events_inspector_at_head_once() {
        let html = format!(
            r#"<html><head><script>window.inspectorUrl = "{STANDARD_EVENTS_INSPECTOR_URL}"</script></head><body></body></html>"#
        );
        let injected = inject_standard_events_inspector(&html);

        assert_eq!(
            injected
                .matches(STANDARD_EVENTS_INSPECTOR_SCRIPT_ID)
                .count(),
            1
        );
        assert!(injected.contains(&format!(
            r#"<head><script id="{STANDARD_EVENTS_INSPECTOR_SCRIPT_ID}" src="{STANDARD_EVENTS_INSPECTOR_URL}" defer></script><script>"#
        )));
        assert_eq!(
            inject_standard_events_inspector(&injected)
                .matches(STANDARD_EVENTS_INSPECTOR_SCRIPT_ID)
                .count(),
            1
        );
    }

    #[test]
    fn patches_html() {
        let html = r#"<html><head></head><body data-base-url="https://example.myshopify.com"></body></html>"#;
        let patched = patch_html(
            html,
            &ctx(LiveReloadMode::HotReload),
            &BTreeMap::new(),
            &BTreeMap::new(),
        );
        assert!(patched.contains("data-base-url=\"http://127.0.0.1:9292\""));
        assert!(patched.contains(HOT_RELOAD_SCRIPT_ID));
    }

    #[test]
    fn computes_hot_reload_payload() {
        let asset = ThemeAsset {
            key: "sections/header.liquid".into(),
            checksum: "1".into(),
            attachment: None,
            value: Some("{% stylesheet %}.x{}{% endstylesheet %}<div></div>".into()),
            stats: None,
        };
        let files = BTreeMap::from([(asset.key.clone(), asset.clone())]);
        let unsynced = BTreeSet::from([asset.key.clone()]);
        let mut cache = BTreeMap::new();
        let payload = hot_reload_payload(
            &asset.key,
            Some(&asset),
            &BTreeMap::new(),
            &files,
            &unsynced,
            &mut cache,
        );
        assert!(payload.updated_file_parts.unwrap().stylesheet_tag);
        assert!(payload
            .replace_templates
            .contains_key("sections/header.liquid"));
        // Second call with same checksum should report no stylesheet change.
        let payload2 = hot_reload_payload(
            &asset.key,
            Some(&asset),
            &BTreeMap::new(),
            &files,
            &unsynced,
            &mut cache,
        );
        assert!(!payload2.updated_file_parts.unwrap().stylesheet_tag);
        assert!(requires_full_page_reload("layout/theme.liquid"));
    }

    #[test]
    fn detects_liquid_tag_content_with_dash_whitespace_variants() {
        let value = r#"
            {%- stylesheet -%}
              .foo { color: red; }
            {%- endstylesheet -%}
            {% javascript       %}
              console.log('hi');
            {%- endjavascript -%}
            {%      schema -%}
              { "name": "Test" }
            {%-       endschema -%}
        "#;

        assert_eq!(
            liquid_tag(value, "stylesheet").unwrap().trim(),
            ".foo { color: red; }"
        );
        assert_eq!(
            liquid_tag(value, "javascript").unwrap().trim(),
            "console.log('hi');"
        );
        assert_eq!(
            liquid_tag(value, "schema").unwrap().trim(),
            r#"{ "name": "Test" }"#
        );
    }

    #[test]
    fn cookies_round_trip_and_extract_set_cookie() {
        let cookies = parse_cookies("_shopify_essential=:a=b:; storefront_digest=xyz");
        assert_eq!(cookies.get("_shopify_essential").unwrap(), ":a=b:");
        assert_eq!(
            serialize_cookies(&cookies),
            "_shopify_essential=:a=b:; storefront_digest=xyz"
        );
        assert_eq!(
            cookie_from_set_cookie(
                &[
                    "other=1; Path=/".into(),
                    "_shopify_essential=abc; Path=/; HttpOnly".into()
                ],
                "_shopify_essential"
            ),
            Some("abc".into())
        );
    }

    #[test]
    fn identifies_json_reconciliation_only_for_json_files() {
        let fs = ThemeFileSystem {
            root: PathBuf::new(),
            files: BTreeMap::from([
                (
                    "templates/index.json".into(),
                    asset("templates/index.json", "local"),
                ),
                ("assets/app.css".into(), asset("assets/app.css", "local")),
                (
                    "config/local.json".into(),
                    asset("config/local.json", "local"),
                ),
            ]),
            filters: IgnoreFilters::default(),
        };
        let diff = identify_json_reconciliation(
            vec![
                Checksum {
                    key: "templates/index.json".into(),
                    checksum: "remote".into(),
                },
                Checksum {
                    key: "templates/product.json".into(),
                    checksum: "remote".into(),
                },
                Checksum {
                    key: "assets/app.css".into(),
                    checksum: "remote".into(),
                },
            ],
            &fs,
            &IgnoreFilters::default(),
        );

        assert_eq!(
            diff.local_only
                .iter()
                .map(|file| file.key.as_str())
                .collect::<Vec<_>>(),
            vec!["config/local.json"]
        );
        assert_eq!(
            diff.remote_only
                .iter()
                .map(|file| file.key.as_str())
                .collect::<Vec<_>>(),
            vec!["templates/product.json"]
        );
        assert_eq!(
            diff.conflicts
                .iter()
                .map(|file| file.key.as_str())
                .collect::<Vec<_>>(),
            vec!["templates/index.json"]
        );
    }

    #[test]
    fn builds_json_reconciliation_plan_from_choices() {
        let diff = JsonReconciliationDiff {
            local_only: vec![Checksum {
                key: "config/local.json".into(),
                checksum: "1".into(),
            }],
            remote_only: vec![Checksum {
                key: "config/remote.json".into(),
                checksum: "2".into(),
            }],
            conflicts: vec![Checksum {
                key: "templates/index.json".into(),
                checksum: "3".into(),
            }],
        };
        let plan = build_json_reconciliation_plan(
            &diff,
            false,
            Some(ReconciliationChoice::Remote),
            Some(ReconciliationChoice::Local),
            Some(ReconciliationChoice::Remote),
        )
        .unwrap();

        assert_eq!(plan.local_files_to_delete, vec!["config/local.json"]);
        assert_eq!(plan.remote_files_to_delete, vec!["config/remote.json"]);
        assert_eq!(plan.files_to_download, vec!["templates/index.json"]);
        assert!(build_json_reconciliation_plan(&diff, false, None, None, None).is_err());
    }

    #[test]
    fn aborts_when_local_and_remote_both_changed() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().to_path_buf();
        std::fs::create_dir_all(root.join("templates")).unwrap();
        std::fs::write(root.join("templates/asset.json"), r#"{"local":true}"#).unwrap();

        let filesystem = ThemeFileSystem {
            root,
            files: BTreeMap::from([(
                "templates/asset.json".into(),
                asset("templates/asset.json", "stale-in-memory"),
            )]),
            filters: IgnoreFilters::default(),
        };
        let changed = vec![Checksum {
            key: "templates/asset.json".into(),
            checksum: "remote-new".into(),
        }];

        let error = abort_if_multiple_sources_changed(&filesystem, &changed).unwrap_err();
        assert!(error
            .to_string()
            .contains("on both local and remote sources. Aborting..."));
        assert!(error.to_string().contains("templates/asset.json"));
    }

    #[test]
    fn allows_remote_change_when_local_disk_matches_memory() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().to_path_buf();
        let written = ThemeAsset {
            key: "templates/asset.json".into(),
            checksum: String::new(),
            attachment: None,
            value: Some(r#"{"ok":true}"#.into()),
            stats: None,
        };
        crate::filesystem::write_theme_asset(&root, &written).unwrap();
        let on_disk = crate::filesystem::read_theme_asset(&root, "templates/asset.json")
            .unwrap()
            .expect("asset on disk");

        let filesystem = ThemeFileSystem {
            root,
            files: BTreeMap::from([("templates/asset.json".into(), on_disk)]),
            filters: IgnoreFilters::default(),
        };
        let changed = vec![Checksum {
            key: "templates/asset.json".into(),
            checksum: "remote-new".into(),
        }];

        assert!(abort_if_multiple_sources_changed(&filesystem, &changed).is_ok());
    }

    #[test]
    fn filter_json_checksums_applies_only_ignore_and_unsynced() {
        let filesystem = ThemeFileSystem {
            root: PathBuf::new(),
            files: BTreeMap::new(),
            filters: IgnoreFilters::default(),
        };
        let checksums = vec![
            Checksum {
                key: "templates/asset.json".into(),
                checksum: "1".into(),
            },
            Checksum {
                key: "templates/other.json".into(),
                checksum: "2".into(),
            },
            Checksum {
                key: "sections/header.liquid".into(),
                checksum: "3".into(),
            },
            Checksum {
                key: "templates/unsynced.json".into(),
                checksum: "4".into(),
            },
        ];

        let only_filter = IgnoreFilters {
            only: vec!["templates/asset.json".into()],
            ..IgnoreFilters::default()
        };
        let only = filter_json_checksums(
            checksums.clone(),
            &filesystem,
            &only_filter,
            &BTreeSet::new(),
        );
        assert_eq!(
            only.iter().map(|c| c.key.as_str()).collect::<Vec<_>>(),
            vec!["templates/asset.json"]
        );

        let ignore_filter = IgnoreFilters {
            ignore: vec!["templates/asset.json".into()],
            ..IgnoreFilters::default()
        };
        let ignored = filter_json_checksums(
            checksums.clone(),
            &filesystem,
            &ignore_filter,
            &BTreeSet::new(),
        );
        assert_eq!(
            ignored.iter().map(|c| c.key.as_str()).collect::<Vec<_>>(),
            vec!["templates/other.json", "templates/unsynced.json"]
        );

        let unsynced = BTreeSet::from(["templates/unsynced.json".into()]);
        let filtered =
            filter_json_checksums(checksums, &filesystem, &IgnoreFilters::default(), &unsynced);
        assert_eq!(
            filtered.iter().map(|c| c.key.as_str()).collect::<Vec<_>>(),
            vec!["templates/asset.json", "templates/other.json"]
        );
    }

    #[tokio::test]
    async fn apply_json_reconciliation_returns_updated_checksums() {
        struct Api;

        #[async_trait::async_trait]
        impl ThemeSyncAdmin for Api {
            async fn fetch_checksums(&self, _theme_id: i64) -> Result<Vec<Checksum>, SyncError> {
                Ok(vec![Checksum {
                    key: "templates/remote.json".into(),
                    checksum: "after".into(),
                }])
            }

            async fn fetch_assets(
                &self,
                _theme_id: i64,
                keys: Vec<String>,
            ) -> Result<Vec<ThemeAsset>, SyncError> {
                Ok(keys
                    .into_iter()
                    .map(|key| ThemeAsset {
                        key: key.clone(),
                        checksum: "downloaded".into(),
                        attachment: None,
                        value: Some(r#"{"ok":true}"#.into()),
                        stats: None,
                    })
                    .collect())
            }

            async fn upload_assets(
                &self,
                _theme_id: i64,
                _assets: Vec<ThemeAsset>,
            ) -> Result<Vec<crate::sync::RemoteResult>, SyncError> {
                Ok(Vec::new())
            }

            async fn delete_assets(
                &self,
                _theme_id: i64,
                _keys: Vec<String>,
            ) -> Result<Vec<crate::sync::RemoteResult>, SyncError> {
                Ok(Vec::new())
            }
        }

        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().to_path_buf();
        let local = asset("config/local.json", "local");
        crate::filesystem::write_theme_asset(&root, &local).unwrap();
        let mut filesystem = ThemeFileSystem {
            root,
            files: BTreeMap::from([("config/local.json".into(), local)]),
            filters: IgnoreFilters::default(),
        };
        let plan = JsonReconciliationPlan {
            local_files_to_delete: vec!["config/local.json".into()],
            files_to_download: vec!["templates/remote.json".into()],
            remote_files_to_delete: vec![],
        };

        let checksums = apply_json_reconciliation(&Api, 1, &mut filesystem, plan)
            .await
            .unwrap();
        assert!(!filesystem.files.contains_key("config/local.json"));
        assert!(filesystem.files.contains_key("templates/remote.json"));
        assert_eq!(checksums[0].checksum, "after");
    }

    #[test]
    fn resolve_port_errors_when_explicit_port_is_taken() {
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let taken = listener.local_addr().unwrap().port();
        let error = resolve_port("127.0.0.1", Some(taken)).unwrap_err();
        assert!(matches!(error, DevError::PortUnavailable(port) if port == taken));
    }

    #[test]
    fn render_dev_links_include_keypress_hints() {
        let urls = DevServerUrls {
            local: "http://127.0.0.1:9292".into(),
            preview: "https://shop.myshopify.com?preview_theme_id=1".into(),
            editor: "https://shop.myshopify.com/admin/themes/1/editor?hr=9292".into(),
            gift_card: "http://127.0.0.1:9292/gift_cards/[store_id]/preview".into(),
        };
        let lines = render_dev_links(&urls);
        assert_eq!(lines.len(), 4);
        assert!(lines[0].contains("(t)"));
        assert!(lines[0].contains(&urls.local));
        assert!(lines[1].contains("(p)"));
        assert!(lines[2].contains("(e)"));
        assert!(lines[3].contains("(g)"));
    }

    #[test]
    fn identifies_json_reconciliation_respects_only_filter() {
        let fs = ThemeFileSystem {
            root: PathBuf::new(),
            files: BTreeMap::from([
                (
                    "templates/index.json".into(),
                    asset("templates/index.json", "local"),
                ),
                (
                    "config/settings_data.json".into(),
                    asset("config/settings_data.json", "local"),
                ),
            ]),
            filters: IgnoreFilters::default(),
        };
        let filters = IgnoreFilters {
            only: vec!["templates/*".into()],
            ..IgnoreFilters::default()
        };
        let diff = identify_json_reconciliation(
            vec![
                Checksum {
                    key: "templates/index.json".into(),
                    checksum: "remote".into(),
                },
                Checksum {
                    key: "config/settings_data.json".into(),
                    checksum: "remote".into(),
                },
            ],
            &fs,
            &filters,
        );
        assert_eq!(
            diff.conflicts
                .iter()
                .map(|f| f.key.as_str())
                .collect::<Vec<_>>(),
            vec!["templates/index.json"]
        );
        assert!(diff.local_only.is_empty());
        assert!(diff.remote_only.is_empty());
    }

    #[test]
    fn should_log_forwards_paths_without_ignored_prefixes_or_extensions() {
        assert!(should_log_request("/products/some-product"));
        assert!(!should_log_request("/checkouts/some-path"));
        assert!(!should_log_request("/payments/config"));
        assert!(!should_log_request("/cdn/extension/some-path"));
        assert!(!should_log_request("/ext/cdn/extensions/x/assets/file.js"));
        assert!(!should_log_request("/assets/styles.css"));
        assert!(!should_log_request("/assets/script.js?version=1.2.3"));
        assert!(should_log_request("/products/some-product?variant=123"));
    }

    #[test]
    fn theme_id_from_html_extracts_matching_theme_id() {
        let html = r#"<script>var Shopify = Shopify || {};
            Shopify.locale = "en";
            Shopify.theme = {"name":"Development","id":143509762348,"theme_store_id":null,"role":"development"};
            Shopify.theme.handle = "null";</script>"#;
        assert_eq!(theme_id_from_html(html), Some(143509762348));
        assert_eq!(theme_id_from_html("no theme here"), None);
        assert_eq!(
            theme_id_from_html(r#"Shopify.theme = {"name":"Development","id":"456"}"#),
            Some(456)
        );
        assert_eq!(
            theme_id_from_html(r#"Shopify.theme={"name":"X","id":7}"#),
            Some(7)
        );
    }

    #[test]
    fn stale_asset_query_rewrites_detected() {
        assert!(is_stale_asset_query(
            "/cdn/shop/t/img/assets/file4.js",
            "1234"
        ));
        assert!(is_stale_asset_query(
            "/ext/cdn/extensions/1a/assets/file.css",
            "5678"
        ));
        assert!(!is_stale_asset_query("/cdn/shop/t/img/assets/file4.js", ""));
        assert!(!is_stale_asset_query(
            "/cdn/shop/t/img/assets/file4.js",
            "v=123"
        ));
        assert!(!is_stale_asset_query("/products/polo", "1"));
    }

    #[test]
    fn mismatch_location_filters_sfr_params() {
        assert_eq!(
            mismatch_location("/?_ab=0&__sfr_test=true&_fd=0&_sc=1"),
            "/?_ab=0&_fd=0&_sc=1"
        );
        assert_eq!(mismatch_location("/products/polo"), "/products/polo");
        assert_eq!(
            mismatch_location("/search?q=foo&__sfr_x=1"),
            "/search?q=foo"
        );
    }

    #[tokio::test]
    async fn theme_id_mismatch_refreshes_session_and_redirects() {
        let mut state = state_with_files(BTreeMap::new());
        let session = Arc::new(Mutex::new(DevServerSession {
            store_fqdn: "example.myshopify.com".into(),
            admin_token: "admin".into(),
            storefront_token: Some("sfr".into()),
            theme_access_domain: None,
            session_cookies: BTreeMap::new(),
        }));
        state.session = session.clone();
        let refresh_session = session.clone();
        state.refresh = Some(Arc::new(move || {
            let session = refresh_session.clone();
            Box::pin(async move {
                let mut session = session.lock().expect("poisoned");
                session.admin_token = "refreshed".into();
                Ok(session.clone())
            })
        }));

        let response = handle_theme_id_mismatch(
            state.clone(),
            456,
            Some("/?__sfr_test=true&_ab=0&_fd=0&_sc=1"),
        )
        .await;

        assert_eq!(response.status(), StatusCode::FOUND);
        assert_eq!(
            response.headers().get(LOCATION).unwrap(),
            "/?_ab=0&_fd=0&_sc=1"
        );
        assert_eq!(session.lock().unwrap().admin_token, "refreshed");

        let response = handle_theme_id_mismatch(state, 456, Some("/")).await;
        assert_eq!(response.status(), StatusCode::FOUND);
    }

    #[tokio::test]
    async fn theme_id_mismatch_aborts_after_max_redirects() {
        let mut state = state_with_files(BTreeMap::new());
        let session = Arc::new(Mutex::new(DevServerSession {
            store_fqdn: "example.myshopify.com".into(),
            admin_token: "admin".into(),
            storefront_token: Some("sfr".into()),
            theme_access_domain: None,
            session_cookies: BTreeMap::new(),
        }));
        state.session = session.clone();

        let response = handle_theme_id_mismatch(state.clone(), 456, None).await;
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(state.theme_id_mismatch_redirects.load(Ordering::SeqCst), 1);

        for _ in 0..5 {
            handle_theme_id_mismatch(state.clone(), 456, None).await;
        }
        assert_eq!(state.theme_id_mismatch_redirects.load(Ordering::SeqCst), 6);
    }

    #[test]
    fn in_memory_templates_filter_by_route_and_locale() {
        let content = "{}";
        let files = BTreeMap::from([
            (
                "templates/index.json".into(),
                value_asset("templates/index.json", content),
            ),
            (
                "templates/search.json".into(),
                value_asset("templates/search.json", content),
            ),
            (
                "locales/en.default.json".into(),
                value_asset("locales/en.default.json", content),
            ),
            (
                "locales/en.default.schema.json".into(),
                value_asset("locales/en.default.schema.json", content),
            ),
            (
                "locales/es.json".into(),
                value_asset("locales/es.json", content),
            ),
            (
                "locales/es.schema.json".into(),
                value_asset("locales/es.schema.json", content),
            ),
            (
                "sections/header.liquid".into(),
                value_asset("sections/header.liquid", "markup"),
            ),
            (
                "assets/app.css".into(),
                value_asset("assets/app.css", "css"),
            ),
        ]);
        let unsynced: BTreeSet<String> = files.keys().cloned().collect();

        // All templates without a route:
        let all = get_in_memory_templates(&files, &unsynced, None, None);
        assert!(all.contains_key("templates/index.json"));
        assert!(all.contains_key("templates/search.json"));
        assert!(all.contains_key("sections/header.liquid"));
        assert!(all.contains_key("locales/en.default.json"));
        assert!(!all.contains_key("assets/app.css"));

        // Unknown route -> no filtering of templates:
        let unknown = get_in_memory_templates(&files, &unsynced, Some("/unknown"), None);
        assert!(unknown.contains_key("templates/index.json"));
        assert!(unknown.contains_key("templates/search.json"));

        // Known route -> only that template:
        let index = get_in_memory_templates(&files, &unsynced, Some("/"), None);
        assert!(index.contains_key("templates/index.json"));
        assert!(!index.contains_key("templates/search.json"));
        let index_html = get_in_memory_templates(&files, &unsynced, Some("/index.html"), None);
        assert!(index_html.contains_key("templates/index.json"));
        let search = get_in_memory_templates(&files, &unsynced, Some("/search"), None);
        assert!(search.contains_key("templates/search.json"));
        assert!(!search.contains_key("templates/index.json"));

        // Unknown locale -> default:
        let default_locale = get_in_memory_templates(&files, &unsynced, None, Some("unknown"));
        assert!(default_locale.contains_key("locales/en.default.json"));
        assert!(default_locale.contains_key("locales/en.default.schema.json"));
        assert!(!default_locale.contains_key("locales/es.json"));

        // Known locale:
        let en = get_in_memory_templates(&files, &unsynced, None, Some("en"));
        assert!(en.contains_key("locales/en.default.json"));
        assert!(!en.contains_key("locales/es.json"));
        let es = get_in_memory_templates(&files, &unsynced, None, Some("es"));
        assert!(es.contains_key("locales/es.json"));
        assert!(es.contains_key("locales/es.schema.json"));
        assert!(!es.contains_key("locales/en.default.json"));
    }

    #[test]
    fn storefront_replace_templates_params_encodes_keys_and_method() {
        let params = storefront_replace_templates_params(
            &BTreeMap::from([("sections/header.liquid".into(), "<div>hi</div>".into())]),
            "GET",
        );
        assert_eq!(
            params,
            "replace_templates%5Bsections%2Fheader.liquid%5D=%3Cdiv%3Ehi%3C%2Fdiv%3E&_method=GET"
        );
    }

    #[test]
    fn section_names_cached_from_json_and_found_by_type() {
        let template = r#"{"sections":{"first":{"type":"header"},"second":{"type":"header"}}}"#;
        let mut section_names = BTreeMap::new();
        save_sections_from_json(&mut section_names, "templates/index.json", template);
        assert_eq!(
            section_names.get("templates/index.json").unwrap(),
            &vec![
                ("header".to_string(), "first".to_string()),
                ("header".to_string(), "second".to_string())
            ]
        );

        let files = BTreeMap::from([
            (
                "sections/header.liquid".into(),
                value_asset("sections/header.liquid", ""),
            ),
            (
                "templates/index.json".into(),
                value_asset("templates/index.json", template),
            ),
        ]);
        assert_eq!(
            find_section_names_to_reload("sections/header.liquid", &files, &section_names),
            vec!["first".to_string(), "second".to_string()]
        );
        assert!(
            find_section_names_to_reload("sections/footer.liquid", &files, &section_names)
                .is_empty()
        );
    }

    #[test]
    fn hr_log_returns_no_content_and_ignores_invalid_json() {
        assert_eq!(handle_hr_log("not-json").status(), StatusCode::NO_CONTENT);
        assert_eq!(
            handle_hr_log(r#"{"type":"warn","headline":"hi","body":"details"}"#).status(),
            StatusCode::NO_CONTENT
        );
        assert_eq!(
            handle_hr_log(r#"{"type":"info","headline":"hi"}"#).status(),
            StatusCode::NO_CONTENT
        );
    }

    #[tokio::test]
    async fn section_render_returns_no_content_without_ids() {
        let state = state_with_files(BTreeMap::new());
        let request = Request::builder().uri("/").body(Body::empty()).unwrap();
        let response = render_section(state, request, BTreeMap::new()).await;
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn section_render_returns_empty_when_section_removed() {
        let state = state_with_files(BTreeMap::new());
        state.watch.mark_unsynced("sections/header.liquid");
        let request = Request::builder()
            .uri("/?section_id=123__first&section_key=sections/header.liquid")
            .body(Body::empty())
            .unwrap();
        let query = BTreeMap::from([
            ("section_id".into(), "123__first".into()),
            ("section_key".into(), "sections/header.liquid".into()),
        ]);
        let response = render_section(state, request, query).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        assert!(body.is_empty());
    }

    #[tokio::test]
    async fn section_render_includes_matching_json_templates_in_replace_templates() {
        let state = state_with_files(BTreeMap::from([
            (
                "sections/header.liquid".into(),
                value_asset("sections/header.liquid", "<header></header>"),
            ),
            (
                "templates/index.json".into(),
                value_asset(
                    "templates/index.json",
                    r#"{"sections":{"first":{"type":"header"}}}"#,
                ),
            ),
        ]));
        let unsynced: BTreeSet<String> = ["sections/header.liquid", "templates/index.json"]
            .into_iter()
            .map(String::from)
            .collect();
        save_sections_from_json(
            &mut state.section_names_by_file.lock().unwrap(),
            "templates/index.json",
            r#"{"sections":{"first":{"type":"header"}}}"#,
        );

        let replace_templates = build_section_replace_templates(
            &state,
            "sections/header.liquid",
            "123__first",
            &unsynced,
        )
        .unwrap();
        assert_eq!(
            replace_templates.get("sections/header.liquid").unwrap(),
            "<header></header>"
        );
        assert!(replace_templates.contains_key("templates/index.json"));

        // A section id not matching any cached name omits the JSON file:
        let replace_templates = build_section_replace_templates(
            &state,
            "sections/header.liquid",
            "123__other",
            &unsynced,
        )
        .unwrap();
        assert!(replace_templates.contains_key("sections/header.liquid"));
        assert!(!replace_templates.contains_key("templates/index.json"));

        // Unsynced section rendered twice, matching name yields the JSON file:
        assert!(build_section_replace_templates(
            &state,
            "sections/header.liquid",
            "123__first",
            &unsynced
        )
        .is_some());
    }

    #[test]
    fn section_render_returns_none_when_template_removed() {
        let state = state_with_files(BTreeMap::new());
        let unsynced: BTreeSet<String> = ["sections/header.liquid"]
            .into_iter()
            .map(String::from)
            .collect();
        assert_eq!(
            build_section_replace_templates(
                &state,
                "sections/header.liquid",
                "123__first",
                &unsynced
            ),
            None
        );
    }

    #[tokio::test]
    async fn local_hot_reload_script_returns_no_content_without_env() {
        let previous = std::env::var("SHOPIFY_CLI_LOCAL_HOT_RELOAD").ok();
        std::env::remove_var("SHOPIFY_CLI_LOCAL_HOT_RELOAD");
        let response = local_hot_reload_script().await;
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        if let Some(previous) = previous {
            std::env::set_var("SHOPIFY_CLI_LOCAL_HOT_RELOAD", previous);
        }
    }
}
