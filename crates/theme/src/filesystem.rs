use crate::checksum::{calculate_checksum, FileContent};
use crate::ignore::{
    apply_ignore_filters, get_patterns_from_shopify_ignore, IgnoreFilters, ThemeFileKey,
};
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::time::UNIX_EPOCH;

pub const THEME_DIRECTORIES: [&str; 9] = [
    "assets",
    "config",
    "layout",
    "locales",
    "sections",
    "blocks",
    "snippets",
    "templates",
    "templates/customers",
];

pub const DEFAULT_IGNORE_PATTERNS: [&str; 15] = [
    "**/.git",
    "**/.vscode",
    "**/.hg",
    "**/.bzr",
    "**/.svn",
    "**/_darcs",
    "**/CVS",
    "**/*.sublime-(project|workspace)",
    "**/.DS_Store",
    "**/.sass-cache",
    "**/Thumbs.db",
    "**/desktop.ini",
    "**/config.yml",
    "**/node_modules/",
    ".prettierrc.json",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThemeAssetStats {
    pub mtime: u128,
    pub size: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThemeAsset {
    pub key: String,
    pub checksum: String,
    pub attachment: Option<String>,
    pub value: Option<String>,
    pub stats: Option<ThemeAssetStats>,
}

impl ThemeFileKey for ThemeAsset {
    fn key(&self) -> &str {
        &self.key
    }
}

#[derive(Debug, Clone)]
pub struct ThemeFileSystem {
    pub root: PathBuf,
    pub files: BTreeMap<String, ThemeAsset>,
    pub filters: IgnoreFilters,
    pub listing: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ThemeFsError {
    #[error("Invalid theme file key: {0}")]
    InvalidKey(String),
    #[error("Unable to read theme file {path}: {source}")]
    Read { path: PathBuf, source: io::Error },
    #[error("Unable to write theme file {path}: {source}")]
    Write { path: PathBuf, source: io::Error },
    #[error("Unable to delete theme file {path}: {source}")]
    Delete { path: PathBuf, source: io::Error },
    #[error("Invalid base64 attachment for theme file {key}: {source}")]
    Attachment {
        key: String,
        source: base64::DecodeError,
    },
    #[error("Unable to activate theme listing: {0}")]
    Listing(String),
}

impl ThemeFileSystem {
    pub fn scan(root: impl AsRef<Path>, filters: IgnoreFilters) -> Result<Self, ThemeFsError> {
        let root = root.as_ref().to_path_buf();
        let mut all_filters = filters;
        all_filters
            .ignore_from_file
            .extend(get_patterns_from_shopify_ignore(&root).map_err(|source| {
                ThemeFsError::Read {
                    path: root.join(crate::ignore::SHOPIFY_IGNORE),
                    source,
                }
            })?);

        let mut keys = Vec::new();
        collect_theme_file_keys(&root, &root, &mut keys)?;
        keys.sort();
        keys.dedup();

        let keys = apply_ignore_filters(keys, &all_filters);
        let mut files = BTreeMap::new();
        for key in keys {
            if let Some(asset) = read_theme_asset(&root, &key)? {
                files.insert(key, asset);
            }
        }

        Ok(Self {
            root,
            files,
            filters: all_filters,
            listing: None,
        })
    }

    pub fn activate_listing(&mut self, listing: impl Into<String>) -> Result<(), ThemeFsError> {
        let listing = listing.into();
        crate::listing::apply_listing(&self.root, &listing, &mut self.files)
            .map_err(|error| ThemeFsError::Listing(error.to_string()))?;
        self.listing = Some(listing);
        Ok(())
    }

    pub fn logical_key_for_path(&self, path: &Path) -> Option<String> {
        logical_key_for_path(&self.root, path, self.listing.as_deref())
    }

    pub fn read_asset(&self, key: &str) -> Result<Option<ThemeAsset>, ThemeFsError> {
        let disk_key = self.disk_key(key);
        let Some(mut asset) = read_theme_asset(&self.root, &disk_key)? else {
            return Ok(None);
        };
        if disk_key != key {
            asset.key = key.to_string();
            asset.checksum = calculate_checksum(
                key,
                asset.value.clone().map(FileContent::Text).or_else(|| {
                    asset.attachment.as_ref().and_then(|value| {
                        BASE64_STANDARD.decode(value).ok().map(FileContent::Binary)
                    })
                }),
            );
        }
        Ok(Some(asset))
    }

    fn disk_key(&self, key: &str) -> String {
        let Some(listing) = self.listing.as_deref() else {
            return key.to_string();
        };
        if !is_listing_json_key(key) {
            return key.to_string();
        }
        let listing_key = format!("listings/{listing}/{key}");
        if self.root.join(&listing_key).is_file() {
            listing_key
        } else {
            key.to_string()
        }
    }

    pub fn read(&mut self, key: &str) -> Result<Option<FileContent>, ThemeFsError> {
        let Some(asset) = self.read_asset(key)? else {
            self.files.remove(key);
            return Ok(None);
        };

        let content = if let Some(value) = &asset.value {
            FileContent::Text(value.clone())
        } else if let Some(attachment) = &asset.attachment {
            FileContent::Binary(BASE64_STANDARD.decode(attachment).map_err(|source| {
                ThemeFsError::Attachment {
                    key: key.to_string(),
                    source,
                }
            })?)
        } else {
            FileContent::Text(String::new())
        };

        self.files.insert(key.to_string(), asset);
        Ok(Some(content))
    }

    pub fn write(&mut self, asset: &ThemeAsset) -> Result<(), ThemeFsError> {
        let mut disk_asset = asset.clone();
        disk_asset.key = self.disk_key(&asset.key);
        write_theme_asset(&self.root, &disk_asset)?;
        if let Some(asset) = self.read_asset(&asset.key)? {
            self.files.insert(asset.key.clone(), asset);
        }
        Ok(())
    }

    pub fn delete(&mut self, key: &str) -> Result<(), ThemeFsError> {
        delete_theme_asset(&self.root, key)?;
        self.files.remove(key);
        Ok(())
    }

    pub fn is_file_ignored(&self, key: &str) -> bool {
        apply_ignore_filters(vec![key], &self.filters).is_empty()
    }
}

pub fn is_listing_json_key(key: &str) -> bool {
    key.ends_with(".json") && (key.starts_with("templates/") || key.starts_with("sections/"))
}

pub fn logical_key_for_path(root: &Path, path: &Path, listing: Option<&str>) -> Option<String> {
    let relative = path.strip_prefix(root).ok()?;
    let key = relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/");
    if let Some(listing) = listing {
        let prefix = format!("listings/{listing}/");
        if let Some(logical) = key.strip_prefix(&prefix) {
            return is_listing_json_key(logical).then(|| logical.to_string());
        }
    }
    is_valid_theme_file_key(&key).then_some(key)
}

pub fn scan_theme_filesystem(
    root: impl AsRef<Path>,
    filters: IgnoreFilters,
) -> Result<ThemeFileSystem, ThemeFsError> {
    ThemeFileSystem::scan(root, filters)
}

pub fn read_theme_asset(
    root: impl AsRef<Path>,
    key: &str,
) -> Result<Option<ThemeAsset>, ThemeFsError> {
    let path = path_for_key(root.as_ref(), key)?;
    if !path.exists() {
        return Ok(None);
    }

    let metadata = fs::metadata(&path).map_err(|source| ThemeFsError::Read {
        path: path.clone(),
        source,
    })?;

    if is_text_file(key) {
        let value = fs::read_to_string(&path).map_err(|source| ThemeFsError::Read {
            path: path.clone(),
            source,
        })?;
        let checksum = calculate_checksum(key, Some(FileContent::Text(value.clone())));
        Ok(Some(ThemeAsset {
            key: key.to_string(),
            checksum,
            attachment: None,
            value: Some(value),
            stats: Some(stats_from_metadata(&metadata)),
        }))
    } else {
        let bytes = fs::read(&path).map_err(|source| ThemeFsError::Read {
            path: path.clone(),
            source,
        })?;
        let checksum = calculate_checksum(key, Some(FileContent::Binary(bytes.clone())));
        Ok(Some(ThemeAsset {
            key: key.to_string(),
            checksum,
            attachment: Some(BASE64_STANDARD.encode(bytes)),
            value: None,
            stats: Some(stats_from_metadata(&metadata)),
        }))
    }
}

pub fn write_theme_asset(root: impl AsRef<Path>, asset: &ThemeAsset) -> Result<(), ThemeFsError> {
    let path = path_for_key(root.as_ref(), &asset.key)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| ThemeFsError::Write {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    if let Some(attachment) = &asset.attachment {
        let bytes =
            BASE64_STANDARD
                .decode(attachment)
                .map_err(|source| ThemeFsError::Attachment {
                    key: asset.key.clone(),
                    source,
                })?;
        fs::write(&path, bytes).map_err(|source| ThemeFsError::Write { path, source })
    } else {
        fs::write(&path, asset.value.as_deref().unwrap_or_default())
            .map_err(|source| ThemeFsError::Write { path, source })
    }
}

pub fn delete_theme_asset(root: impl AsRef<Path>, key: &str) -> Result<(), ThemeFsError> {
    let path = path_for_key(root.as_ref(), key)?;
    if !path.exists() {
        return Ok(());
    }
    fs::remove_file(&path).map_err(|source| ThemeFsError::Delete { path, source })
}

pub fn is_text_file(path: &str) -> bool {
    matches!(
        extension(path),
        Some("css" | "js" | "json" | "liquid" | "sass" | "scss" | "svg")
    )
}

pub fn is_valid_theme_file_key(key: &str) -> bool {
    let mut parts = key.split('/');
    let Some(top_level) = parts.next() else {
        return false;
    };
    let Some(file_name) = key.rsplit('/').next() else {
        return false;
    };
    if file_name.is_empty() {
        return false;
    }

    match top_level {
        "assets" => file_name.contains('.'),
        "config" | "locales" => extension(key) == Some("json"),
        "layout" | "blocks" | "snippets" => extension(key) == Some("liquid"),
        "sections" | "templates" => matches!(extension(key), Some("liquid" | "json")),
        _ => false,
    }
}

fn collect_theme_file_keys(
    root: &Path,
    current: &Path,
    keys: &mut Vec<String>,
) -> Result<(), ThemeFsError> {
    let entries = fs::read_dir(current).map_err(|source| ThemeFsError::Read {
        path: current.to_path_buf(),
        source,
    })?;

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
            if should_descend_into(&key) {
                collect_theme_file_keys(root, &path, keys)?;
            }
        } else if metadata.is_file() && is_valid_theme_file_key(&key) {
            keys.push(key);
        }
    }

    Ok(())
}

fn should_descend_into(key: &str) -> bool {
    if key.is_empty() {
        return true;
    }

    THEME_DIRECTORIES
        .iter()
        .any(|directory| directory.starts_with(key) || key.starts_with(&format!("{directory}/")))
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

fn stats_from_metadata(metadata: &fs::Metadata) -> ThemeAssetStats {
    let mtime = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis())
        .unwrap_or_default();

    ThemeAssetStats {
        mtime,
        size: metadata.len() as usize,
    }
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

fn path_for_key(root: &Path, key: &str) -> Result<PathBuf, ThemeFsError> {
    if key.is_empty() || key.starts_with('/') || key.contains('\\') {
        return Err(ThemeFsError::InvalidKey(key.to_string()));
    }

    let mut path = root.to_path_buf();
    for component in Path::new(key).components() {
        match component {
            Component::Normal(value) => path.push(value),
            _ => return Err(ThemeFsError::InvalidKey(key.to_string())),
        }
    }

    Ok(path)
}

fn extension(path: &str) -> Option<&str> {
    path.rsplit('/')
        .next()
        .and_then(|file_name| file_name.rsplit_once('.').map(|(_, extension)| extension))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ignore::IgnoreFilters;
    use std::fs;

    fn write_file(root: &Path, key: &str, content: impl AsRef<[u8]>) {
        let path = root.join(key);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    #[test]
    fn scans_valid_theme_directories_only() {
        let temp = tempfile::tempdir().unwrap();
        write_file(temp.path(), "assets/theme.css", "body{}");
        write_file(temp.path(), "config/settings_schema.json", "[]");
        write_file(temp.path(), "layout/theme.liquid", "");
        write_file(temp.path(), "locales/en.default.json", "{}");
        write_file(temp.path(), "sections/header.json", "{}");
        write_file(temp.path(), "blocks/card.liquid", "");
        write_file(temp.path(), "snippets/icon.liquid", "");
        write_file(temp.path(), "templates/index.json", "{}");
        write_file(temp.path(), "templates/customers/account.liquid", "");
        write_file(temp.path(), "templates/customers/account.txt", "");
        write_file(temp.path(), "README.md", "");
        write_file(temp.path(), "assets/node_modules/package.js", "");
        write_file(temp.path(), ".prettierrc.json", "{}");

        let fs = scan_theme_filesystem(temp.path(), IgnoreFilters::default()).unwrap();
        let keys = fs.files.keys().cloned().collect::<Vec<_>>();

        assert_eq!(
            keys,
            vec![
                "assets/theme.css",
                "blocks/card.liquid",
                "config/settings_schema.json",
                "layout/theme.liquid",
                "locales/en.default.json",
                "sections/header.json",
                "snippets/icon.liquid",
                "templates/customers/account.liquid",
                "templates/index.json",
            ]
        );
    }

    #[test]
    fn scanner_applies_shopifyignore_and_cli_filters() {
        let temp = tempfile::tempdir().unwrap();
        write_file(temp.path(), "assets/theme.css", "");
        write_file(temp.path(), "assets/keep.css", "");
        write_file(temp.path(), "templates/index.json", "{}");
        fs::write(
            temp.path().join(".shopifyignore"),
            "assets/*\n!assets/keep.css\n",
        )
        .unwrap();

        let fs = scan_theme_filesystem(
            temp.path(),
            IgnoreFilters {
                only: vec!["templates/*.json".into(), "!templates/index.json".into()],
                ..IgnoreFilters::default()
            },
        )
        .unwrap();

        let keys = fs.files.keys().cloned().collect::<Vec<_>>();
        assert_eq!(keys, vec!["assets/keep.css"]);
    }

    #[test]
    fn default_ignore_patterns_include_upstream_entries() {
        assert!(DEFAULT_IGNORE_PATTERNS.contains(&"**/node_modules/"));
        assert!(DEFAULT_IGNORE_PATTERNS.contains(&".prettierrc.json"));
    }

    #[test]
    fn active_listing_reads_and_writes_existing_json_override_by_logical_key() {
        let temp = tempfile::tempdir().unwrap();
        write_file(temp.path(), "templates/index.json", r#"{"source":"base"}"#);
        write_file(
            temp.path(),
            "listings/summer/templates/index.json",
            r#"{"source":"listing"}"#,
        );
        let mut filesystem = ThemeFileSystem::scan(temp.path(), IgnoreFilters::default()).unwrap();
        filesystem.activate_listing("summer").unwrap();

        let asset = filesystem
            .read_asset("templates/index.json")
            .unwrap()
            .unwrap();
        assert_eq!(asset.key, "templates/index.json");
        assert_eq!(asset.value.as_deref(), Some(r#"{"source":"listing"}"#));

        filesystem
            .write(&ThemeAsset {
                key: "templates/index.json".into(),
                checksum: String::new(),
                value: Some(r#"{"source":"remote"}"#.into()),
                attachment: None,
                stats: None,
            })
            .unwrap();

        assert_eq!(
            fs::read_to_string(temp.path().join("listings/summer/templates/index.json")).unwrap(),
            r#"{"source":"remote"}"#
        );
        assert_eq!(
            fs::read_to_string(temp.path().join("templates/index.json")).unwrap(),
            r#"{"source":"base"}"#
        );
    }

    #[test]
    fn listing_path_maps_to_logical_shopify_key() {
        let root = Path::new("/theme");
        assert_eq!(
            logical_key_for_path(
                root,
                Path::new("/theme/listings/summer/sections/header.json"),
                Some("summer")
            )
            .as_deref(),
            Some("sections/header.json")
        );
        assert!(logical_key_for_path(
            root,
            Path::new("/theme/listings/summer/assets/app.css"),
            Some("summer")
        )
        .is_none());
    }

    #[test]
    fn reads_text_assets_with_value_and_binary_assets_with_attachment() {
        let temp = tempfile::tempdir().unwrap();
        write_file(temp.path(), "assets/theme.css", "body{}\r\n");
        write_file(temp.path(), "assets/logo.png", [0, 1, 2, 3]);

        let text = read_theme_asset(temp.path(), "assets/theme.css")
            .unwrap()
            .unwrap();
        let binary = read_theme_asset(temp.path(), "assets/logo.png")
            .unwrap()
            .unwrap();

        assert_eq!(text.value.as_deref(), Some("body{}\r\n"));
        assert!(text.attachment.is_none());
        assert!(binary.value.is_none());
        assert_eq!(
            BASE64_STANDARD.decode(binary.attachment.unwrap()).unwrap(),
            vec![0, 1, 2, 3]
        );
    }

    #[test]
    fn writes_and_deletes_theme_assets() {
        let temp = tempfile::tempdir().unwrap();
        let mut fs = ThemeFileSystem::scan(temp.path(), IgnoreFilters::default()).unwrap();

        fs.write(&ThemeAsset {
            key: "assets/theme.css".into(),
            checksum: String::new(),
            attachment: None,
            value: Some("body{}".into()),
            stats: None,
        })
        .unwrap();

        assert_eq!(
            fs::read_to_string(temp.path().join("assets/theme.css")).unwrap(),
            "body{}"
        );
        assert!(fs.files.contains_key("assets/theme.css"));

        fs.delete("assets/theme.css").unwrap();

        assert!(!temp.path().join("assets/theme.css").exists());
        assert!(!fs.files.contains_key("assets/theme.css"));
    }

    #[test]
    fn writes_binary_attachments() {
        let temp = tempfile::tempdir().unwrap();
        let asset = ThemeAsset {
            key: "assets/logo.png".into(),
            checksum: String::new(),
            attachment: Some(BASE64_STANDARD.encode([0, 1, 2, 3])),
            value: None,
            stats: None,
        };

        write_theme_asset(temp.path(), &asset).unwrap();

        assert_eq!(
            fs::read(temp.path().join("assets/logo.png")).unwrap(),
            vec![0, 1, 2, 3]
        );
    }

    #[test]
    fn rejects_unsafe_keys() {
        assert!(path_for_key(Path::new("/tmp/theme"), "../secret").is_err());
        assert!(path_for_key(Path::new("/tmp/theme"), "/secret").is_err());
        assert!(path_for_key(Path::new("/tmp/theme"), r"assets\secret").is_err());
    }
}
