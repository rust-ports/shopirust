use crate::checksum::Checksum;
use crate::filesystem::{read_theme_asset, ThemeAsset, ThemeFileSystem, ThemeFsError};
use crate::ignore::{apply_ignore_filters, IgnoreFilters, ThemeFileKey};
use crate::sync::{self, FileOperation, RemoteResult, SyncError, SyncOptions, ThemeSyncAdmin};
use axum::body::Body;
use axum::extract::{Path as AxumPath, Query, Request, State};
use axum::http::header::{ACCEPT, CONTENT_TYPE, HOST, LOCATION};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, Uri};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{any, get};
use axum::Router;
use futures::Stream;
use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use percent_encoding::percent_decode_str;
use regex_lite::Regex;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::convert::Infallible;
use std::net::{IpAddr, SocketAddr, TcpListener};
use std::path::{Path, PathBuf};
use std::pin::Pin;
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
const THEME_EDITOR_POLLING_INTERVAL: Duration = Duration::from_secs(3);
const MAX_THEME_EDITOR_POLLING_FAILURES: usize = 5;

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
}

#[derive(Debug, Clone)]
pub struct DevServerHandle {
    pub urls: DevServerUrls,
}

#[derive(Debug)]
pub struct DevServerRuntime {
    pub refresh_rx: Option<mpsc::Receiver<Result<DevServerSession, String>>>,
    pub terminal_controls: bool,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThemeFileEventKind {
    CreateOrUpdate,
    Delete,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeFileEvent {
    pub key: String,
    pub kind: ThemeFileEventKind,
}

impl ThemeFileKey for ThemeFileEvent {
    fn key(&self) -> &str {
        &self.key
    }
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
    let state = AppState {
        ctx: Arc::new(ctx.clone()),
        session: session.clone(),
        files: Arc::new(Mutex::new(filesystem.files.clone())),
        unsynced_file_keys: Arc::new(Mutex::new(BTreeSet::new())),
        last_requested_path: Arc::new(Mutex::new(String::new())),
        upload_errors: Arc::new(Mutex::new(BTreeMap::new())),
        reload_tx: reload_tx.clone(),
        client: reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|error| DevError::Server(error.to_string()))?,
    };

    let listener = TokioTcpListener::bind((ctx.options.host.as_str(), ctx.options.port))
        .await
        .map_err(|error| DevError::Bind(socket_addr(&ctx.options.host, ctx.options.port), error))?;
    let app = router(state.clone());
    let (watch_tx, mut watch_rx) = mpsc::channel(256);
    let _watcher = start_watcher(&ctx.options.root, ctx.options.poll, watch_tx)?;
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
                        handle_file_event(api, &ctx, &mut filesystem, &state, event).await?;
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
    event: ThemeFileEvent,
) -> Result<(), DevError>
where
    A: ThemeSyncAdmin + Sync,
{
    match event.kind {
        ThemeFileEventKind::CreateOrUpdate => {
            let Some(asset) = read_theme_asset(&ctx.options.root, &event.key)? else {
                return Ok(());
            };
            state
                .unsynced_file_keys
                .lock()
                .expect("unsynced keys poisoned")
                .insert(event.key.clone());
            filesystem.files.insert(event.key.clone(), asset.clone());
            state
                .files
                .lock()
                .expect("file state poisoned")
                .insert(event.key.clone(), asset.clone());
            let results = api
                .upload_assets(ctx.theme.id, vec![asset.clone()])
                .await
                .map_err(DevError::Sync)?;
            remember_upload_errors(state, results, FileOperation::Upload);
            state
                .unsynced_file_keys
                .lock()
                .expect("unsynced keys poisoned")
                .remove(&event.key);
            emit_reload(ctx, &state.reload_tx, &event.key, Some(asset), false);
        }
        ThemeFileEventKind::Delete => {
            state
                .unsynced_file_keys
                .lock()
                .expect("unsynced keys poisoned")
                .insert(event.key.clone());
            filesystem.files.remove(&event.key);
            state
                .files
                .lock()
                .expect("file state poisoned")
                .remove(&event.key);
            if !ctx.options.nodelete {
                let results = api
                    .delete_assets(ctx.theme.id, vec![event.key.clone()])
                    .await
                    .map_err(DevError::Sync)?;
                remember_upload_errors(state, results, FileOperation::Delete);
            }
            state
                .unsynced_file_keys
                .lock()
                .expect("unsynced keys poisoned")
                .remove(&event.key);
            emit_reload(ctx, &state.reload_tx, &event.key, None, true);
        }
    }
    Ok(())
}

fn remember_upload_errors(state: &AppState, results: Vec<RemoteResult>, operation: FileOperation) {
    let mut errors = state.upload_errors.lock().expect("upload errors poisoned");
    for result in results {
        if result.success {
            errors.remove(&result.key);
        } else {
            errors.insert(
                result.key,
                result
                    .errors
                    .into_iter()
                    .chain(std::iter::once(format!("{operation:?} failed")))
                    .collect(),
            );
        }
    }
}

fn emit_reload(
    ctx: &DevServerContext,
    tx: &broadcast::Sender<HotReloadEvent>,
    key: &str,
    asset: Option<ThemeAsset>,
    deleted: bool,
) {
    if ctx.options.live_reload == LiveReloadMode::Off {
        return;
    }
    let theme_id = ctx.theme.id.to_string();
    if ctx.options.live_reload == LiveReloadMode::FullPage || requires_full_page_reload(key) {
        let _ = tx.send(HotReloadEvent::Full {
            version: HOT_RELOAD_VERSION.into(),
            theme_id,
            key: key.into(),
        });
        return;
    }
    let event = if deleted {
        HotReloadEvent::Delete {
            version: HOT_RELOAD_VERSION.into(),
            sync: "remote".into(),
            theme_id,
            key: key.into(),
        }
    } else {
        HotReloadEvent::Update {
            version: HOT_RELOAD_VERSION.into(),
            sync: "remote".into(),
            theme_id,
            key: key.into(),
            payload: asset
                .as_ref()
                .map(|asset| hot_reload_payload(key, asset))
                .unwrap_or_default(),
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

pub fn hot_reload_payload(key: &str, asset: &ThemeAsset) -> HotReloadPayload {
    HotReloadPayload {
        section_names: section_names_from_json(key, asset.value.as_deref()),
        replace_templates: if needs_template_update(key) {
            BTreeMap::from([(key.to_string(), asset.value.clone().unwrap_or_default())])
        } else {
            BTreeMap::new()
        },
        updated_file_parts: updated_file_parts(asset),
    }
}

fn needs_template_update(key: &str) -> bool {
    !key.starts_with("assets/") && (key.ends_with(".liquid") || key.ends_with(".json"))
}

fn section_names_from_json(key: &str, value: Option<&str>) -> Vec<String> {
    if !key.ends_with(".json") || key.starts_with("locales/") {
        return Vec::new();
    }
    let Some(value) = value else {
        return Vec::new();
    };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(value) else {
        return Vec::new();
    };
    json.get("sections")
        .and_then(|sections| sections.as_object())
        .map(|sections| sections.keys().cloned().collect())
        .unwrap_or_default()
}

fn updated_file_parts(asset: &ThemeAsset) -> Option<UpdatedFileParts> {
    let key = asset.key.as_str();
    let valid = ["sections/", "snippets/", "blocks/"]
        .iter()
        .any(|prefix| key.starts_with(prefix))
        && key.ends_with(".liquid");
    if !valid {
        return None;
    }
    let value = asset.value.as_deref().unwrap_or_default();
    Some(UpdatedFileParts {
        stylesheet_tag: liquid_tag(value, "stylesheet").is_some(),
        javascript_tag: liquid_tag(value, "javascript").is_some(),
        schema_tag: liquid_tag(value, "schema").is_some(),
        liquid: true,
    })
}

fn liquid_tag<'a>(value: &'a str, tag: &str) -> Option<&'a str> {
    let pattern = format!(r"(?s)\{{%\s*{tag}\s*%\}}(.*?)\{{%\s*end{tag}\s*%\}}");
    Regex::new(&pattern)
        .ok()
        .and_then(|regex| regex.captures(value))
        .and_then(|captures| captures.get(1))
        .map(|matched| matched.as_str())
}

fn start_watcher(
    root: &Path,
    poll: bool,
    tx: mpsc::Sender<notify::Result<notify::Event>>,
) -> Result<RecommendedWatcher, DevError> {
    let tx = move |result| {
        let _ = tx.blocking_send(result);
    };
    let config = if poll {
        Config::default().with_poll_interval(Duration::from_millis(500))
    } else {
        Config::default()
    };
    let mut watcher =
        RecommendedWatcher::new(tx, config).map_err(|error| DevError::Watch(error.to_string()))?;
    watcher
        .watch(root, RecursiveMode::Recursive)
        .map_err(|error| DevError::Watch(error.to_string()))?;
    Ok(watcher)
}

fn normalize_event(root: &Path, result: notify::Result<notify::Event>) -> Option<ThemeFileEvent> {
    let event = result.ok()?;
    let kind = match event.kind {
        EventKind::Create(_) | EventKind::Modify(_) => ThemeFileEventKind::CreateOrUpdate,
        EventKind::Remove(_) => ThemeFileEventKind::Delete,
        _ => return None,
    };
    let path = event
        .paths
        .into_iter()
        .find(|path| path.is_file() || matches!(kind, ThemeFileEventKind::Delete))?;
    let key = key_from_path(root, &path)?;
    Some(ThemeFileEvent { key, kind })
}

fn key_from_path(root: &Path, path: &Path) -> Option<String> {
    let relative = path.strip_prefix(root).ok()?;
    let key = relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/");
    crate::filesystem::is_valid_theme_file_key(&key).then_some(key)
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
    let unsynced = state
        .unsynced_file_keys
        .lock()
        .expect("unsynced keys poisoned")
        .clone();
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

fn abort_if_multiple_sources_changed(
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

fn filter_json_checksums(
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
                        Some(format!(
                            "{}?previewPath={}",
                            urls.editor,
                            url::form_urlencoded::byte_serialize(path.as_bytes())
                                .collect::<String>()
                        ))
                    }
                }
                _ => None,
            };
            if let Some(target) = target {
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

#[derive(Clone)]
struct AppState {
    ctx: Arc<DevServerContext>,
    session: Arc<Mutex<DevServerSession>>,
    files: Arc<Mutex<BTreeMap<String, ThemeAsset>>>,
    unsynced_file_keys: Arc<Mutex<BTreeSet<String>>>,
    last_requested_path: Arc<Mutex<String>>,
    upload_errors: Arc<Mutex<BTreeMap<String, Vec<String>>>>,
    reload_tx: broadcast::Sender<HotReloadEvent>,
    client: reqwest::Client,
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
        .route("/cdn/*path", get(cdn_asset_or_proxy))
        .fallback(any(proxy_or_render))
        .layer(cors)
        .with_state(state)
}

fn local_origin(ctx: &DevServerContext) -> String {
    format!("http://{}:{}", ctx.options.host, ctx.options.port)
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
    if let Some(index) = path.find("/assets/") {
        let key = format!("assets/{}", decode_path(&path[index + "/assets/".len()..]));
        if let Some(response) = local_asset_response(&state, &key) {
            return response;
        }
    }
    proxy_request(state, request).await
}

fn serve_asset(state: &AppState, key: &str) -> Response {
    local_asset_response(state, key).unwrap_or_else(|| StatusCode::NOT_FOUND.into_response())
}

fn local_asset_response(state: &AppState, key: &str) -> Option<Response> {
    let files = state.files.lock().expect("file state poisoned");
    let asset = files.get(key)?;
    let content_type = mime_guess::from_path(key).first_or_octet_stream();
    let mut response = if let Some(value) = &asset.value {
        value.clone().into_response()
    } else if let Some(attachment) = &asset.attachment {
        match base64::Engine::decode(&base64::engine::general_purpose::STANDARD, attachment) {
            Ok(bytes) => bytes.into_response(),
            Err(_) => return Some(StatusCode::INTERNAL_SERVER_ERROR.into_response()),
        }
    } else {
        String::new().into_response()
    };
    response.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_str(content_type.as_ref())
            .unwrap_or(HeaderValue::from_static("application/octet-stream")),
    );
    response
        .headers_mut()
        .insert("x-local-asset", HeaderValue::from_static("true"));
    Some(response)
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
    allowed
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
        || path.starts_with("/checkouts/")
        || path == "/account"
        || path.starts_with("/account/")
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
        let errors = state.upload_errors.lock().expect("upload errors poisoned");
        if !errors.is_empty() {
            return upload_error_page(&errors).into_response();
        }
    }
    query.insert("preview_theme_id".into(), state.ctx.theme.id.to_string());
    query.insert("_fd".into(), "0".into());
    query.insert("pb".into(), "0".into());
    let response = remote_request(
        &state,
        request.method().clone(),
        request.uri().path(),
        query,
        request.headers().clone(),
        None,
    )
    .await;
    patch_response(state, response, true).await
}

async fn proxy_request(state: AppState, request: Request) -> Response {
    let method = request.method().clone();
    let path = request.uri().path().to_string();
    let query = request
        .uri()
        .query()
        .map(|query| {
            url::form_urlencoded::parse(query.as_bytes())
                .into_owned()
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();
    let headers = request.headers().clone();
    let body = axum::body::to_bytes(request.into_body(), usize::MAX)
        .await
        .ok();
    let response = remote_request(&state, method, &path, query, headers, body).await;
    patch_response(state, response, false).await
}

async fn remote_request(
    state: &AppState,
    method: Method,
    path: &str,
    query: BTreeMap<String, String>,
    headers: HeaderMap,
    body: Option<axum::body::Bytes>,
) -> Result<reqwest::Response, reqwest::Error> {
    let session = state
        .session
        .lock()
        .expect("session state poisoned")
        .clone();
    let base = storefront_base_url(&state.ctx);
    let mut url = Url::parse(&base).expect("storefront base URL is valid");
    if state.ctx.session.theme_access_domain.is_some() {
        let path = path.trim_start_matches('/');
        url.set_path(&format!("/cli/sfr/{path}"));
    } else {
        url.set_path(path);
    }
    {
        let mut pairs = url.query_pairs_mut();
        for (key, value) in query {
            pairs.append_pair(&key, &value);
        }
    }
    let mut builder = state.client.request(method, url);
    for (key, value) in headers.iter() {
        if is_hop_by_hop(key.as_str()) {
            continue;
        }
        builder = builder.header(key, value);
    }
    builder = builder
        .header("referer", format!("https://{}", session.store_fqdn))
        .header("cookie", build_cookie_header(&session, &headers));
    if let Some(token) = &session.storefront_token {
        builder = builder.bearer_auth(token);
    }
    if let Some(domain) = &session.theme_access_domain {
        builder = builder
            .header("x-shopify-shop", &session.store_fqdn)
            .header("x-shopify-access-token", &session.admin_token)
            .header("host", domain);
    }
    if let Some(body) = body {
        builder = builder.body(body);
    }
    builder.send().await
}

fn storefront_base_url(ctx: &DevServerContext) -> String {
    if let Some(domain) = &ctx.session.theme_access_domain {
        format!("https://{domain}")
    } else {
        format!("https://{}", ctx.session.store_fqdn)
    }
}

fn upload_error_page(errors: &BTreeMap<String, Vec<String>>) -> Response {
    let items = errors
        .iter()
        .map(|(key, errors)| {
            format!(
                "<li><strong>{}</strong><pre>{}</pre></li>",
                escape_html(key),
                escape_html(&errors.join("\n"))
            )
        })
        .collect::<Vec<_>>()
        .join("");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        [(
            CONTENT_TYPE,
            HeaderValue::from_static("text/html; charset=utf-8"),
        )],
        format!(
            r#"<!doctype html><html><head><title>Failed to Upload Theme Files</title></head><body><h1>Upload Errors</h1><ul>{items}</ul></body></html>"#
        ),
    )
        .into_response()
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
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
    response: Result<reqwest::Response, reqwest::Error>,
    html: bool,
) -> Response {
    let Ok(response) = response else {
        return (StatusCode::BAD_GATEWAY, "Failed to reach storefront").into_response();
    };
    let status =
        StatusCode::from_u16(response.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let mut headers = response.headers().clone();
    update_session_cookies_from_headers(&state, &headers);
    patch_set_cookie_domains(&state, &mut headers);
    strip_response_headers(&mut headers);
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
    if html {
        let body = response.text().await.unwrap_or_default();
        let body = patch_html(
            &body,
            &state.ctx,
            &state.files.lock().expect("file state poisoned"),
        );
        let mut out = (status, body).into_response();
        *out.headers_mut() = headers;
        out.headers_mut().insert(
            CONTENT_TYPE,
            HeaderValue::from_static("text/html; charset=utf-8"),
        );
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
    let url = Url::parse(location)?;
    if url.path().starts_with("/checkouts/") {
        return Ok(location.into());
    }
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
) -> String {
    let content = inject_cdn_proxy(html, &ctx.session.store_fqdn, files);
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
) -> String {
    let escaped_store = regex_lite::escape(store_fqdn);
    let mut output = Regex::new(&format!(r#"(https?:)?//{escaped_store}/cdn/"#))
        .map(|regex| regex.replace_all(content, "/cdn/").to_string())
        .unwrap_or_else(|_| content.to_string());
    let known_assets: BTreeSet<_> = files
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
            if known_assets.contains(asset) && !is_image {
                format!("/cdn/{path}")
            } else {
                matched.to_string()
            }
        })
        .to_string();
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
    if without.contains(&format!(r#"<script id="{HOT_RELOAD_SCRIPT_ID}""#)) {
        return without;
    }
    without.replace(
        "</head>",
        &format!(r#"<script id="{HOT_RELOAD_SCRIPT_ID}" src="{HOT_RELOAD_SCRIPT_URL}" defer></script></head>"#),
    )
}

fn inject_standard_events_inspector(html: &str) -> String {
    html.replace(
        "</body>",
        r#"<script id="shopify-standard-events-inspector">window.Shopify=window.Shopify||{};window.Shopify.themeInspector=true;</script></body>"#,
    )
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
    sync::push(
        api,
        theme_id,
        filesystem,
        &SyncOptions {
            nodelete: options.nodelete,
            filters: options.filters.clone(),
        },
    )
    .await
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
        let patched = inject_cdn_proxy(html, "example.myshopify.com", &files);
        assert!(patched.contains("/cdn/theme/assets/app.js"));
        assert!(patched.contains("/cdn/s/files/1/assets/app.js"));
        assert!(patched.contains("//cdn.shopify.com/s/files/1/assets/logo.png"));
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
    fn patches_html() {
        let html = r#"<html><head></head><body data-base-url="https://example.myshopify.com"></body></html>"#;
        let patched = patch_html(html, &ctx(LiveReloadMode::HotReload), &BTreeMap::new());
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
        let payload = hot_reload_payload(&asset.key, &asset);
        assert!(payload.updated_file_parts.unwrap().stylesheet_tag);
        assert!(payload
            .replace_templates
            .contains_key("sections/header.liquid"));
        assert!(requires_full_page_reload("layout/theme.liquid"));
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
}
