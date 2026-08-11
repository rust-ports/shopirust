//! Debounced filesystem watcher for app / extension directories.

use crate::error::AppError;
use crate::models::loader::LoadedApp;
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

const DEFAULT_DEBOUNCE_MS: u64 = 200;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WatcherEventType {
    ExtensionFolderCreated,
    ExtensionFolderDeleted,
    FileCreated,
    FileUpdated,
    FileDeleted,
    ExtensionsConfigUpdated,
    AppConfigDeleted,
}

impl WatcherEventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ExtensionFolderCreated => "extension_folder_created",
            Self::ExtensionFolderDeleted => "extension_folder_deleted",
            Self::FileCreated => "file_created",
            Self::FileUpdated => "file_updated",
            Self::FileDeleted => "file_deleted",
            Self::ExtensionsConfigUpdated => "extensions_config_updated",
            Self::AppConfigDeleted => "app_config_deleted",
        }
    }
}

#[derive(Debug, Clone)]
pub struct WatcherEvent {
    pub r#type: WatcherEventType,
    pub path: PathBuf,
    pub extension_handle: Option<String>,
    pub extension_path: PathBuf,
    pub start_time: Instant,
}

type ChangeCallback = Arc<dyn Fn(Vec<WatcherEvent>) + Send + Sync>;

pub struct FileWatcher {
    app: LoadedApp,
    debounce: Duration,
    on_change: Option<ChangeCallback>,
    extension_paths: Vec<PathBuf>,
    watched_file_owners: HashMap<PathBuf, HashSet<String>>,
    pending: Arc<Mutex<Vec<WatcherEvent>>>,
}

impl FileWatcher {
    pub fn new(app: LoadedApp) -> Self {
        Self::with_debounce(app, DEFAULT_DEBOUNCE_MS)
    }

    pub fn with_debounce(app: LoadedApp, debounce_ms: u64) -> Self {
        let mut watcher = Self {
            app: app.clone(),
            debounce: Duration::from_millis(debounce_ms),
            on_change: None,
            extension_paths: vec![],
            watched_file_owners: HashMap::new(),
            pending: Arc::new(Mutex::new(Vec::new())),
        };
        watcher.update_app(app);
        watcher
    }

    pub fn on_change<F>(&mut self, cb: F)
    where
        F: Fn(Vec<WatcherEvent>) + Send + Sync + 'static,
    {
        self.on_change = Some(Arc::new(cb));
    }

    pub fn update_app(&mut self, app: LoadedApp) {
        self.app = app;
        self.extension_paths = self
            .app
            .extensions
            .iter()
            .filter(|e| !e.specification.is_app_config())
            .map(|e| e.directory.clone())
            .filter(|d| d != &self.app.directory)
            .collect();
        self.rebuild_watched_files();
    }

    fn rebuild_watched_files(&mut self) {
        self.watched_file_owners.clear();
        for ext in &self.app.extensions {
            for file in ext.watched_files() {
                self.watched_file_owners
                    .entry(file)
                    .or_default()
                    .insert(ext.handle.clone());
            }
        }
    }

    pub fn watch_paths(&self) -> Vec<PathBuf> {
        let mut paths = vec![self.app.configuration_path.clone()];
        let ext_dirs = self
            .app
            .configuration
            .extension_directories
            .clone()
            .unwrap_or_else(|| vec!["extensions".into()]);
        for d in ext_dirs {
            let full = self.app.directory.join(d.trim_end_matches('/').trim_end_matches("/**"));
            paths.push(full);
        }
        paths.extend(self.watched_file_owners.keys().cloned());
        paths.sort();
        paths.dedup();
        paths
    }

    /// Start watching. Spawns a background task; returns when cancelled or watcher errors.
    pub async fn start(&self, cancel: CancellationToken) -> Result<(), AppError> {
        for dir in self.extension_directory_roots() {
            let _ = std::fs::create_dir_all(&dir);
        }

        let (tx, rx) = mpsc::channel();
        let mut watcher = RecommendedWatcher::new(
            move |res| {
                let _ = tx.send(res);
            },
            notify::Config::default(),
        )
        .map_err(|e| AppError::message(format!("file watcher: {e}")))?;

        for path in self.watch_paths() {
            let mode = if path.is_dir() {
                RecursiveMode::Recursive
            } else {
                RecursiveMode::NonRecursive
            };
            let _ = watcher.watch(&path, mode);
        }

        let pending = self.pending.clone();
        let on_change = self.on_change.clone();
        let debounce = self.debounce;
        let app_config = self.app.configuration_path.clone();
        let extension_paths = self.extension_paths.clone();
        let owners = self.watched_file_owners.clone();
        let cancel_bg = cancel.clone();

        tokio::task::spawn_blocking(move || {
            let _watcher = watcher;
            let mut last_emit = Instant::now();
            loop {
                if cancel_bg.is_cancelled() {
                    break;
                }
                match rx.recv_timeout(debounce) {
                    Ok(Ok(event)) => {
                        let mapped = map_notify_event(
                            &event,
                            &app_config,
                            &extension_paths,
                            &owners,
                        );
                        if mapped.is_empty() {
                            continue;
                        }
                        let mut buf = pending.lock().unwrap();
                        for ev in mapped {
                            if !buf.iter().any(|e| {
                                e.path == ev.path
                                    && e.r#type == ev.r#type
                                    && e.extension_handle == ev.extension_handle
                            }) {
                                buf.push(ev);
                            }
                        }
                        // leading+trailing style: emit if enough time passed or queue after timeout
                        if last_emit.elapsed() >= debounce {
                            let events = std::mem::take(&mut *buf);
                            drop(buf);
                            if let Some(cb) = &on_change {
                                cb(events);
                            }
                            last_emit = Instant::now();
                        }
                    }
                    Ok(Err(_)) => break,
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        let mut buf = pending.lock().unwrap();
                        if !buf.is_empty() {
                            let events = std::mem::take(&mut *buf);
                            drop(buf);
                            if let Some(cb) = &on_change {
                                cb(events);
                            }
                            last_emit = Instant::now();
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
        });

        cancel.cancelled().await;
        Ok(())
    }

    /// Synchronous classify helper for tests (no notify).
    pub fn classify_path(
        &self,
        path: &Path,
        kind: WatcherEventType,
    ) -> Vec<WatcherEvent> {
        classify(
            path,
            kind,
            &self.app.configuration_path,
            &self.extension_paths,
            &self.watched_file_owners,
        )
    }

    fn extension_directory_roots(&self) -> Vec<PathBuf> {
        let ext_dirs = self
            .app
            .configuration
            .extension_directories
            .clone()
            .unwrap_or_else(|| vec!["extensions".into()]);
        ext_dirs
            .into_iter()
            .map(|d| {
                self.app
                    .directory
                    .join(d.trim_end_matches('/').trim_end_matches("/**"))
            })
            .collect()
    }
}

fn map_notify_event(
    event: &notify::Event,
    app_config: &Path,
    extension_paths: &[PathBuf],
    owners: &HashMap<PathBuf, HashSet<String>>,
) -> Vec<WatcherEvent> {
    let kind = match event.kind {
        EventKind::Create(_) => WatcherEventType::FileCreated,
        EventKind::Modify(_) => WatcherEventType::FileUpdated,
        EventKind::Remove(_) => WatcherEventType::FileDeleted,
        _ => return vec![],
    };
    let mut out = Vec::new();
    for path in &event.paths {
        if should_ignore(path) {
            continue;
        }
        out.extend(classify(path, kind.clone(), app_config, extension_paths, owners));
    }
    out
}

fn should_ignore(path: &Path) -> bool {
    path.components().any(|c| {
        let s = c.as_os_str().to_string_lossy();
        s == "node_modules" || s == ".git"
    })
}

fn classify(
    path: &Path,
    mut kind: WatcherEventType,
    app_config: &Path,
    extension_paths: &[PathBuf],
    owners: &HashMap<PathBuf, HashSet<String>>,
) -> Vec<WatcherEvent> {
    let start = Instant::now();
    let file_name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("");

    if path == app_config {
        if matches!(kind, WatcherEventType::FileDeleted) {
            return vec![WatcherEvent {
                r#type: WatcherEventType::AppConfigDeleted,
                path: path.to_path_buf(),
                extension_handle: None,
                extension_path: app_config
                    .parent()
                    .unwrap_or(Path::new(""))
                    .to_path_buf(),
                start_time: start,
            }];
        }
        kind = WatcherEventType::ExtensionsConfigUpdated;
        return vec![WatcherEvent {
            r#type: kind,
            path: path.to_path_buf(),
            extension_handle: None,
            extension_path: app_config
                .parent()
                .unwrap_or(Path::new(""))
                .to_path_buf(),
            start_time: start,
        }];
    }

    if file_name.ends_with(".toml")
        && (file_name.contains("extension") || file_name.starts_with("shopify."))
    {
        if matches!(kind, WatcherEventType::FileDeleted) {
            let ext_path = path.parent().unwrap_or(path).to_path_buf();
            return vec![WatcherEvent {
                r#type: WatcherEventType::ExtensionFolderDeleted,
                path: path.to_path_buf(),
                extension_handle: None,
                extension_path: ext_path,
                start_time: start,
            }];
        }
        if matches!(kind, WatcherEventType::FileCreated) {
            return vec![WatcherEvent {
                r#type: WatcherEventType::ExtensionFolderCreated,
                path: path.to_path_buf(),
                extension_handle: None,
                extension_path: path.parent().unwrap_or(path).to_path_buf(),
                start_time: start,
            }];
        }
        kind = WatcherEventType::ExtensionsConfigUpdated;
    }

    if let Some(handles) = owners.get(path) {
        return handles
            .iter()
            .map(|h| {
                let ext_path = extension_paths
                    .iter()
                    .find(|p| path.starts_with(p))
                    .cloned()
                    .unwrap_or_else(|| path.parent().unwrap_or(path).to_path_buf());
                WatcherEvent {
                    r#type: kind.clone(),
                    path: path.to_path_buf(),
                    extension_handle: Some(h.clone()),
                    extension_path: ext_path,
                    start_time: start,
                }
            })
            .collect();
    }

    // Ownership by directory containment
    if let Some(ext_path) = extension_paths.iter().find(|p| path.starts_with(p)) {
        return vec![WatcherEvent {
            r#type: kind,
            path: path.to_path_buf(),
            extension_handle: None,
            extension_path: ext_path.clone(),
            start_time: start,
        }];
    }

    vec![]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::app::{AppConfiguration, AppHiddenConfig};
    use crate::models::extensions::create_extension_specification;
    use crate::models::extensions::ExtensionInstance;
    use crate::models::identifiers::Identifiers;
    use std::collections::HashMap;
    use tempfile::tempdir;

    fn loaded_app(dir: &Path) -> LoadedApp {
        let spec = create_extension_specification("ui_extension").unwrap();
        let ext_dir = dir.join("extensions/my-ext");
        std::fs::create_dir_all(&ext_dir).unwrap();
        let config_path = ext_dir.join("shopify.extension.toml");
        std::fs::write(&config_path, "name = \"x\"").unwrap();
        std::fs::write(ext_dir.join("src.js"), "1").unwrap();
        let mut ext = ExtensionInstance::new(
            "my-ext",
            ext_dir.clone(),
            config_path,
            HashMap::new(),
            spec,
        );
        ext.uid = Some("1".into());
        LoadedApp {
            directory: dir.to_path_buf(),
            configuration_path: dir.join("shopify.app.toml"),
            configuration: AppConfiguration::default(),
            hidden_config: AppHiddenConfig::default(),
            extensions: vec![ext],
            webs: vec![],
            identifiers: Identifiers::new(),
            name: "app".into(),
            errors: vec![],
        }
    }

    #[test]
    fn watch_paths_include_app_toml_and_extensions() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("shopify.app.toml"), "").unwrap();
        let app = loaded_app(dir.path());
        let fw = FileWatcher::new(app);
        let paths = fw.watch_paths();
        assert!(paths.iter().any(|p| p.ends_with("shopify.app.toml")));
        assert!(paths.iter().any(|p| p.ends_with("extensions")));
    }

    #[test]
    fn classifies_file_update_with_handle() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("shopify.app.toml"), "").unwrap();
        let app = loaded_app(dir.path());
        let fw = FileWatcher::new(app);
        let src = dir.path().join("extensions/my-ext/src.js");
        let events = fw.classify_path(&src, WatcherEventType::FileUpdated);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].r#type, WatcherEventType::FileUpdated);
        assert_eq!(events[0].extension_handle.as_deref(), Some("my-ext"));
    }

    #[test]
    fn classifies_extension_toml_update() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("shopify.app.toml"), "").unwrap();
        let app = loaded_app(dir.path());
        let fw = FileWatcher::new(app);
        let toml = dir.path().join("extensions/my-ext/shopify.extension.toml");
        let events = fw.classify_path(&toml, WatcherEventType::FileUpdated);
        assert_eq!(events[0].r#type, WatcherEventType::ExtensionsConfigUpdated);
    }

    #[test]
    fn classifies_app_config_deleted() {
        let dir = tempdir().unwrap();
        let app_toml = dir.path().join("shopify.app.toml");
        std::fs::write(&app_toml, "").unwrap();
        let app = loaded_app(dir.path());
        let fw = FileWatcher::new(app);
        let events = fw.classify_path(&app_toml, WatcherEventType::FileDeleted);
        assert_eq!(events[0].r#type, WatcherEventType::AppConfigDeleted);
    }

    #[test]
    fn classifies_new_extension_toml_as_folder_created() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("shopify.app.toml"), "").unwrap();
        let app = loaded_app(dir.path());
        let fw = FileWatcher::new(app);
        let toml = dir.path().join("extensions/new-ext/shopify.extension.toml");
        let events = fw.classify_path(&toml, WatcherEventType::FileCreated);
        assert_eq!(events[0].r#type, WatcherEventType::ExtensionFolderCreated);
    }
}
