use crate::checksum::FileContent;
use crate::filesystem::{read_theme_asset, ThemeAsset, ThemeFsError};
use notify::Watcher;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// Glob-style whitelist used by upstream `THEME_EXT_DIRECTORY_PATTERNS`.
pub const THEME_EXT_DIRECTORY_PATTERNS: [&str; 4] = [
    "assets/**/*.*",
    "locales/**/*.json",
    "blocks/**/*.liquid",
    "snippets/**/*.liquid",
];

const THEME_EXT_DIRECTORIES: [&str; 4] = ["assets", "locales", "blocks", "snippets"];

/// Upstream clears `unsyncedFileKeys` after `sleep(5)` on file updates.
pub const UNSYNCED_CLEAR_DELAY_MS: u64 = 5;

/// Max directory depth matching upstream glob `{ deep: 3 }`.
const THEME_EXT_MAX_DEPTH: usize = 3;

type EventListener = Arc<dyn Fn(ThemeExtFsEventPayload) + Send + Sync>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ThemeExtFsEventName {
    Add,
    Change,
    Unlink,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeExtFsEventPayload {
    pub file_key: String,
    pub content: Option<String>,
}

/// In-memory theme-extension filesystem (mirrors `mountThemeExtensionFileSystem`).
#[derive(Clone)]
pub struct ThemeExtensionFileSystem {
    pub root: PathBuf,
    files: Arc<Mutex<BTreeMap<String, ThemeAsset>>>,
    unsynced_file_keys: Arc<Mutex<BTreeSet<String>>>,
    listeners: Arc<Mutex<BTreeMap<ThemeExtFsEventName, Vec<EventListener>>>>,
}

impl std::fmt::Debug for ThemeExtensionFileSystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ThemeExtensionFileSystem")
            .field("root", &self.root)
            .field("files", &self.files.lock().map(|g| g.len()).unwrap_or(0))
            .field(
                "unsynced_file_keys",
                &self.unsynced_file_keys.lock().map(|g| g.len()).unwrap_or(0),
            )
            .finish()
    }
}

impl ThemeExtensionFileSystem {
    pub fn files(&self) -> BTreeMap<String, ThemeAsset> {
        self.files.lock().expect("theme ext files poisoned").clone()
    }

    pub fn unsynced_file_keys(&self) -> BTreeSet<String> {
        self.unsynced_file_keys
            .lock()
            .expect("theme ext unsynced poisoned")
            .clone()
    }

    pub fn ready(&self) {
        // Mount loads files synchronously; retained for upstream API parity.
    }

    /// In-memory delete (does not touch disk), matching upstream `delete`.
    pub fn delete(&self, file_key: &str) {
        self.files
            .lock()
            .expect("theme ext files poisoned")
            .remove(file_key);
    }

    /// In-memory write (does not touch disk), matching upstream `write`.
    pub fn write(&self, asset: ThemeAsset) {
        self.files
            .lock()
            .expect("theme ext files poisoned")
            .insert(asset.key.clone(), asset);
    }

    /// Reads from disk into the in-memory map (mirrors upstream `read`).
    pub fn read(&self, key: &str) -> Result<Option<FileContent>, ThemeFsError> {
        let Some(asset) = read_theme_asset(&self.root, key)? else {
            self.files
                .lock()
                .expect("theme ext files poisoned")
                .remove(key);
            return Ok(None);
        };

        let content = content_from_asset(&asset)?;
        self.files
            .lock()
            .expect("theme ext files poisoned")
            .insert(key.to_string(), asset);
        Ok(Some(content))
    }

    pub fn add_event_listener(
        &self,
        event_name: ThemeExtFsEventName,
        callback: impl Fn(ThemeExtFsEventPayload) + Send + Sync + 'static,
    ) {
        self.listeners
            .lock()
            .expect("theme ext listeners poisoned")
            .entry(event_name)
            .or_default()
            .push(Arc::new(callback));
    }

    /// Handles a watcher add/change: reload, mark unsynced, clear after ~5ms, emit.
    pub fn handle_file_update(&self, event_name: ThemeExtFsEventName, file_path: &Path) {
        let Some(file_key) = key_for_path(&self.root, file_path) else {
            return;
        };
        if !is_valid_theme_ext_file_key(&file_key) {
            return;
        }

        let content = match self.read(&file_key) {
            Ok(Some(FileContent::Text(text))) => Some(text),
            Ok(Some(FileContent::Binary(bytes))) => Some(base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                bytes,
            )),
            _ => None,
        };

        {
            let mut unsynced = self
                .unsynced_file_keys
                .lock()
                .expect("theme ext unsynced poisoned");
            unsynced.insert(file_key.clone());
        }
        self.schedule_unsynced_clear(file_key.clone());

        if content.as_ref().is_some_and(|value| !value.is_empty()) {
            self.emit(event_name, ThemeExtFsEventPayload { file_key, content });
        }
    }

    /// Handles a watcher unlink: drop from maps and emit.
    pub fn handle_file_delete(&self, file_path: &Path) {
        let Some(file_key) = key_for_path(&self.root, file_path) else {
            return;
        };

        self.unsynced_file_keys
            .lock()
            .expect("theme ext unsynced poisoned")
            .remove(&file_key);
        self.files
            .lock()
            .expect("theme ext files poisoned")
            .remove(&file_key);

        self.emit(
            ThemeExtFsEventName::Unlink,
            ThemeExtFsEventPayload {
                file_key,
                content: None,
            },
        );
    }

    /// Watches the four extension top-level directories (mirrors `startWatcher`).
    pub fn start_watcher(&self) -> Result<notify::RecommendedWatcher, ThemeFsError> {
        let fs = self.clone();
        let root = self.root.clone();
        let mut watcher = notify::RecommendedWatcher::new(
            move |result: Result<notify::Event, notify::Error>| {
                let Ok(event) = result else {
                    return;
                };
                for path in event.paths {
                    match event.kind {
                        notify::EventKind::Create(_) => {
                            fs.handle_file_update(ThemeExtFsEventName::Add, &path);
                        }
                        notify::EventKind::Modify(_) => {
                            fs.handle_file_update(ThemeExtFsEventName::Change, &path);
                        }
                        notify::EventKind::Remove(_) => {
                            fs.handle_file_delete(&path);
                        }
                        _ => {}
                    }
                }
            },
            notify::Config::default(),
        )
        .map_err(|source| ThemeFsError::Read {
            path: root.clone(),
            source: std::io::Error::other(source.to_string()),
        })?;

        for directory in THEME_EXT_DIRECTORIES {
            let path = root.join(directory);
            if path.is_dir() {
                watcher
                    .watch(&path, notify::RecursiveMode::Recursive)
                    .map_err(|source| ThemeFsError::Read {
                        path: path.clone(),
                        source: std::io::Error::other(source.to_string()),
                    })?;
            }
        }

        Ok(watcher)
    }

    fn schedule_unsynced_clear(&self, file_key: String) {
        let unsynced = Arc::clone(&self.unsynced_file_keys);
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(UNSYNCED_CLEAR_DELAY_MS));
            if let Ok(mut set) = unsynced.lock() {
                set.remove(&file_key);
            }
        });
    }

    fn emit(&self, event_name: ThemeExtFsEventName, payload: ThemeExtFsEventPayload) {
        let listeners = self
            .listeners
            .lock()
            .expect("theme ext listeners poisoned")
            .get(&event_name)
            .cloned()
            .unwrap_or_default();
        for listener in listeners {
            listener(payload.clone());
        }
    }
}

/// Mounts a theme-extension filesystem from `root`.
/// Invalid / missing directories yield an empty in-memory map (upstream parity).
pub fn mount_theme_extension_file_system(root: impl AsRef<Path>) -> ThemeExtensionFileSystem {
    let root = root.as_ref().to_path_buf();
    let mut files = BTreeMap::new();

    if root.is_dir() {
        if let Ok(keys) = collect_theme_ext_file_keys(&root) {
            for key in keys {
                if let Ok(Some(asset)) = read_theme_asset(&root, &key) {
                    files.insert(key, asset);
                }
            }
        }
    }

    ThemeExtensionFileSystem {
        root,
        files: Arc::new(Mutex::new(files)),
        unsynced_file_keys: Arc::new(Mutex::new(BTreeSet::new())),
        listeners: Arc::new(Mutex::new(BTreeMap::new())),
    }
}

/// Builds `replace_extension_templates` content from unsynced extension keys
/// (mirrors `getExtensionInMemoryTemplates`).
pub fn get_extension_in_memory_templates(
    filesystem: &ThemeExtensionFileSystem,
) -> BTreeMap<String, String> {
    let files = filesystem.files();
    let unsynced = filesystem.unsynced_file_keys();
    let mut replace_ext_templates = BTreeMap::new();

    for key in unsynced {
        let Some(file) = files.get(&key) else {
            continue;
        };
        let content = file
            .value
            .as_ref()
            .or(file.attachment.as_ref())
            .filter(|value| !value.is_empty());
        if let Some(content) = content {
            replace_ext_templates.insert(key, content.clone());
        }
    }

    replace_ext_templates
}

/// Encodes `replace_extension_templates[bucket][path]=content` form fields,
/// bucketing by the first path segment (mirrors `storefrontReplaceTemplatesParams`).
pub fn replace_extension_templates_params(
    replace_extension_templates: &BTreeMap<String, String>,
) -> String {
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    for (path, content) in replace_extension_templates {
        let bucket = path.split('/').next().unwrap_or("");
        serializer.append_pair(
            &format!("replace_extension_templates[{bucket}][{path}]"),
            content,
        );
    }
    serializer.finish()
}

pub fn is_valid_theme_ext_file_key(key: &str) -> bool {
    if key.is_empty() || key.starts_with('/') || key.contains('\\') {
        return false;
    }
    if key.matches('/').count() > THEME_EXT_MAX_DEPTH {
        return false;
    }

    let mut parts = key.split('/');
    let Some(top_level) = parts.next() else {
        return false;
    };
    let Some(file_name) = key.rsplit('/').next() else {
        return false;
    };
    if file_name.is_empty() || !file_name.contains('.') {
        return false;
    }

    match top_level {
        "assets" => true,
        "locales" => extension(file_name) == Some("json"),
        "blocks" | "snippets" => extension(file_name) == Some("liquid"),
        _ => false,
    }
}

fn collect_theme_ext_file_keys(root: &Path) -> Result<Vec<String>, ThemeFsError> {
    let mut keys = Vec::new();
    for directory in THEME_EXT_DIRECTORIES {
        let path = root.join(directory);
        if path.is_dir() {
            collect_keys(root, &path, &mut keys)?;
        }
    }
    keys.sort();
    keys.dedup();
    Ok(keys)
}

fn collect_keys(root: &Path, current: &Path, keys: &mut Vec<String>) -> Result<(), ThemeFsError> {
    let entries = match fs::read_dir(current) {
        Ok(entries) => entries,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(source) => {
            return Err(ThemeFsError::Read {
                path: current.to_path_buf(),
                source,
            })
        }
    };

    for entry in entries {
        let entry = entry.map_err(|source| ThemeFsError::Read {
            path: current.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        let metadata = entry.metadata().map_err(|source| ThemeFsError::Read {
            path: path.clone(),
            source,
        })?;

        let Some(key) = key_for_path(root, &path) else {
            continue;
        };
        if is_default_ignored(&key) {
            continue;
        }

        if metadata.is_dir() {
            if key.matches('/').count() < THEME_EXT_MAX_DEPTH {
                collect_keys(root, &path, keys)?;
            }
        } else if metadata.is_file() && is_valid_theme_ext_file_key(&key) {
            keys.push(key);
        }
    }

    Ok(())
}

fn key_for_path(root: &Path, path: &Path) -> Option<String> {
    let relative = path.strip_prefix(root).ok()?;
    let key = relative
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/");
    Some(key)
}

fn is_default_ignored(key: &str) -> bool {
    let basename = key.rsplit('/').next().unwrap_or(key);
    matches!(
        basename,
        ".git"
            | ".vscode"
            | ".hg"
            | ".bzr"
            | ".svn"
            | "_darcs"
            | "CVS"
            | ".DS_Store"
            | ".sass-cache"
            | "Thumbs.db"
            | "desktop.ini"
            | "config.yml"
            | "node_modules"
            | ".prettierrc.json"
    ) || basename.starts_with(".sublime-")
        || key.split('/').any(|part| part == "node_modules")
}

fn extension(path: &str) -> Option<&str> {
    path.rsplit_once('.').map(|(_, extension)| extension)
}

fn content_from_asset(asset: &ThemeAsset) -> Result<FileContent, ThemeFsError> {
    if let Some(value) = &asset.value {
        return Ok(FileContent::Text(value.clone()));
    }
    if let Some(attachment) = &asset.attachment {
        let bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, attachment)
            .map_err(|source| ThemeFsError::Attachment {
                key: asset.key.clone(),
                source,
            })?;
        return Ok(FileContent::Binary(bytes));
    }
    Ok(FileContent::Text(String::new()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/theme_ext/fixtures")
    }

    #[test]
    fn mounts_local_theme_extension_filesystem_with_checksums() {
        let root = fixture_root();
        let theme_fs = mount_theme_extension_file_system(&root);
        theme_fs.ready();

        assert_eq!(theme_fs.root, root);
        assert_eq!(theme_fs.files().len(), 4);
        assert!(theme_fs.unsynced_file_keys().is_empty());

        let expected = [
            (
                "blocks/star_rating.liquid",
                "d8ceb73ce5faa4ac22713071d2f0a6bd",
            ),
            (
                "locales/en.default.json",
                "02054e661bbc326a68bf7be83427d7ed",
            ),
            ("assets/thumbs-up.png", "8a1dd937b2cfe9e669b26e41dc1de5e8"),
            ("snippets/stars.liquid", "28fa42561b59f04fc32e98feb3b994ac"),
        ];

        for (key, checksum) in expected {
            let file = theme_fs.files().get(key).cloned().expect(key);
            assert_eq!(file.key, key);
            assert_eq!(file.checksum, checksum);
            assert!(file.stats.as_ref().map(|s| s.size).is_some());
            assert!(file.stats.as_ref().map(|s| s.mtime).is_some());
            if key.ends_with(".png") {
                assert!(file.attachment.is_some());
                assert!(file.value.is_none());
            } else {
                assert!(file.value.is_some());
            }
        }
    }

    #[test]
    fn mounts_empty_filesystem_for_invalid_directory() {
        let root = fixture_root().join("invalid-directory");
        let theme_fs = mount_theme_extension_file_system(&root);
        theme_fs.ready();

        assert_eq!(theme_fs.root, root);
        assert!(theme_fs.files().is_empty());
        assert!(theme_fs.unsynced_file_keys().is_empty());
    }

    #[test]
    fn delete_removes_file_from_map() {
        let theme_fs = mount_theme_extension_file_system(fixture_root());
        theme_fs.ready();

        assert!(theme_fs.files().contains_key("snippets/stars.liquid"));
        theme_fs.delete("snippets/stars.liquid");
        assert!(!theme_fs.files().contains_key("snippets/stars.liquid"));
    }

    #[test]
    fn delete_is_noop_for_missing_file() {
        let theme_fs = mount_theme_extension_file_system(fixture_root());
        theme_fs.ready();
        theme_fs.delete("assets/nonexistent.css");
        assert!(!theme_fs.files().contains_key("assets/nonexistent.css"));
    }

    #[test]
    fn write_creates_file_on_map() {
        let theme_fs = mount_theme_extension_file_system(fixture_root());
        theme_fs.ready();

        assert!(!theme_fs.files().contains_key("assets/new_file.css"));
        theme_fs.write(ThemeAsset {
            key: "assets/new_file.css".into(),
            checksum: "1010".into(),
            value: Some("content".into()),
            attachment: None,
            stats: None,
        });

        let file = theme_fs
            .files()
            .get("assets/new_file.css")
            .cloned()
            .unwrap();
        assert_eq!(file.key, "assets/new_file.css");
        assert_eq!(file.checksum, "1010");
        assert_eq!(file.value.as_deref(), Some("content"));
        assert!(file.attachment.is_none());
    }

    #[test]
    fn write_creates_image_file_on_map() {
        let theme_fs = mount_theme_extension_file_system(fixture_root());
        theme_fs.ready();
        let attachment = "0x123!".to_string();

        theme_fs.write(ThemeAsset {
            key: "assets/new_image.gif".into(),
            checksum: "1010".into(),
            value: None,
            attachment: Some(attachment.clone()),
            stats: None,
        });

        let file = theme_fs
            .files()
            .get("assets/new_image.gif")
            .cloned()
            .unwrap();
        assert_eq!(file.key, "assets/new_image.gif");
        assert_eq!(file.checksum, "1010");
        assert_eq!(file.attachment.as_deref(), Some(attachment.as_str()));
        assert!(file.value.is_none());
    }

    #[test]
    fn read_returns_disk_content_and_updates_map() {
        let theme_fs = mount_theme_extension_file_system(fixture_root());
        theme_fs.ready();
        let key = "snippets/stars.liquid";

        {
            let mut files = theme_fs.files.lock().unwrap();
            if let Some(file) = files.get_mut(key) {
                file.value = None;
            }
        }

        let content = theme_fs.read(key).unwrap().unwrap();
        let FileContent::Text(text) = content else {
            panic!("expected text content");
        };

        let updated = theme_fs.files().get(key).cloned().unwrap();
        assert_eq!(updated.key, key);
        assert_eq!(updated.checksum, "28fa42561b59f04fc32e98feb3b994ac");
        assert_eq!(updated.value.as_deref(), Some(text.as_str()));
        assert!(updated.attachment.is_none());
        assert!(updated.stats.is_some());
    }

    #[test]
    fn get_extension_in_memory_templates_returns_unsynced_content() {
        let theme_fs = mount_theme_extension_file_system(fixture_root());
        theme_fs.ready();

        theme_fs
            .unsynced_file_keys
            .lock()
            .unwrap()
            .insert("snippets/stars.liquid".into());
        theme_fs
            .unsynced_file_keys
            .lock()
            .unwrap()
            .insert("assets/thumbs-up.png".into());
        theme_fs
            .unsynced_file_keys
            .lock()
            .unwrap()
            .insert("missing.liquid".into());

        let templates = get_extension_in_memory_templates(&theme_fs);
        assert!(templates.contains_key("snippets/stars.liquid"));
        assert!(templates.contains_key("assets/thumbs-up.png"));
        assert!(!templates.contains_key("missing.liquid"));
        assert!(!templates["snippets/stars.liquid"].is_empty());
        assert!(!templates["assets/thumbs-up.png"].is_empty());
    }

    #[test]
    fn replace_extension_templates_params_buckets_by_first_segment() {
        let templates = BTreeMap::from([
            ("blocks/hello.liquid".into(), "Hello".into()),
            ("snippets/world.liquid".into(), "World".into()),
        ]);
        let params = replace_extension_templates_params(&templates);
        assert!(params
            .contains("replace_extension_templates%5Bblocks%5D%5Bblocks%2Fhello.liquid%5D=Hello"));
        assert!(params.contains(
            "replace_extension_templates%5Bsnippets%5D%5Bsnippets%2Fworld.liquid%5D=World"
        ));
    }

    #[test]
    fn handle_file_update_marks_unsynced_then_clears_after_delay() {
        let temp = tempfile::tempdir().unwrap();
        let snippet_dir = temp.path().join("snippets");
        fs::create_dir_all(&snippet_dir).unwrap();
        let path = snippet_dir.join("stars.liquid");
        fs::write(&path, "★").unwrap();

        let theme_fs = mount_theme_extension_file_system(temp.path());
        theme_fs.handle_file_update(ThemeExtFsEventName::Change, &path);
        assert!(theme_fs
            .unsynced_file_keys()
            .contains("snippets/stars.liquid"));

        thread::sleep(Duration::from_millis(UNSYNCED_CLEAR_DELAY_MS + 20));
        assert!(!theme_fs
            .unsynced_file_keys()
            .contains("snippets/stars.liquid"));
    }

    #[test]
    fn validates_theme_ext_whitelist_keys() {
        assert!(is_valid_theme_ext_file_key("assets/thumbs-up.png"));
        assert!(is_valid_theme_ext_file_key("locales/en.default.json"));
        assert!(is_valid_theme_ext_file_key("blocks/star_rating.liquid"));
        assert!(is_valid_theme_ext_file_key("snippets/stars.liquid"));
        assert!(!is_valid_theme_ext_file_key("config/settings_schema.json"));
        assert!(!is_valid_theme_ext_file_key("locales/en.default.liquid"));
        assert!(!is_valid_theme_ext_file_key("assets/noext"));
    }
}
