use crate::filesystem::is_valid_theme_file_key;
use serde_json::Value;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageMetadata {
    pub theme_name: String,
    pub theme_version: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum PackageError {
    #[error("Could not find config/settings_schema.json")]
    MissingSchema,
    #[error("config/settings_schema.json contains invalid JSON: {0}")]
    InvalidJson(String),
    #[error("config/settings_schema.json must contain theme_info.theme_name")]
    MissingThemeName,
    #[error("Unable to package theme: {0}")]
    Io(#[from] io::Error),
}

pub fn extract_metadata(root: impl AsRef<Path>) -> Result<PackageMetadata, PackageError> {
    let path = root.as_ref().join("config/settings_schema.json");
    let content = fs::read_to_string(path).map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            PackageError::MissingSchema
        } else {
            PackageError::Io(error)
        }
    })?;
    let value: Value = serde_json::from_str(&strip_json_comments(&content))
        .map_err(|error| PackageError::InvalidJson(error.to_string()))?;
    let entries = value.as_array().ok_or(PackageError::MissingThemeName)?;
    let info = entries
        .iter()
        .find_map(|entry| entry.get("theme_info"))
        .or_else(|| {
            entries
                .iter()
                .find(|entry| entry.get("name").and_then(Value::as_str) == Some("theme_info"))
        });
    let name = info
        .and_then(|info| info.get("theme_name"))
        .and_then(Value::as_str)
        .filter(|name| !name.is_empty())
        .ok_or(PackageError::MissingThemeName)?;
    let version = info
        .and_then(|info| info.get("theme_version"))
        .and_then(Value::as_str)
        .filter(|version| !version.is_empty())
        .map(str::to_owned);
    Ok(PackageMetadata {
        theme_name: name.into(),
        theme_version: version,
    })
}

pub fn package_theme(root: impl AsRef<Path>) -> Result<PathBuf, PackageError> {
    let root = root.as_ref();
    let metadata = extract_metadata(root)?;
    let base = match metadata.theme_version {
        Some(version) => format!("{}-{version}", metadata.theme_name),
        None => metadata.theme_name,
    };
    let output = root.join(format!("{base}.zip"));
    let mut files = Vec::new();
    collect(root, root, &mut files)?;
    files.sort_by(|a, b| a.0.cmp(&b.0));
    write_zip(&output, &files)?;
    Ok(output)
}

fn collect(root: &Path, current: &Path, result: &mut Vec<(String, Vec<u8>)>) -> io::Result<()> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        let relative = path
            .strip_prefix(root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        if path.is_dir() {
            if relative == "listings"
                || relative.starts_with("listings/")
                || [
                    "assets",
                    "blocks",
                    "config",
                    "layout",
                    "locales",
                    "sections",
                    "snippets",
                    "templates",
                ]
                .iter()
                .any(|dir| relative == *dir || dir.starts_with(&format!("{relative}/")))
            {
                collect(root, &path, result)?;
            }
        } else if valid_package_theme_key(&relative)
            || relative.starts_with("listings/")
            || matches!(
                relative.as_str(),
                "release-notes.md" | "update_extension.json"
            )
        {
            result.push((relative, fs::read(path)?));
        }
    }
    Ok(())
}

fn valid_package_theme_key(key: &str) -> bool {
    if !is_valid_theme_file_key(key) {
        return false;
    }
    let parts = key.split('/').collect::<Vec<_>>();
    parts.len() == 2 || (parts.len() == 3 && parts[0] == "templates" && parts[1] == "customers")
}

pub fn strip_json_comments(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let chars: Vec<_> = input.chars().collect();
    let mut i = 0;
    let mut string = false;
    let mut escape = false;
    while i < chars.len() {
        let ch = chars[i];
        if string {
            out.push(ch);
            if ch == '"' && !escape {
                string = false;
            }
            escape = ch == '\\' && !escape;
            i += 1;
            continue;
        }
        if ch == '"' {
            string = true;
            out.push(ch);
            i += 1;
        } else if ch == '/' && chars.get(i + 1) == Some(&'/') {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
        } else if ch == '/' && chars.get(i + 1) == Some(&'*') {
            i += 2;
            while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                i += 1;
            }
            i = (i + 2).min(chars.len());
        } else {
            out.push(ch);
            i += 1;
        }
    }
    out
}

fn write_zip(path: &Path, files: &[(String, Vec<u8>)]) -> io::Result<()> {
    let mut out = fs::File::create(path)?;
    let mut central = Vec::new();
    let mut offset = 0u32;
    for (name, bytes) in files {
        let name = name.as_bytes();
        let crc = crc32(bytes);
        write_u32(&mut out, 0x04034b50)?;
        write_u16(&mut out, 20)?;
        write_u16(&mut out, 0)?;
        write_u16(&mut out, 0)?;
        write_u16(&mut out, 0)?;
        write_u16(&mut out, 0)?;
        write_u32(&mut out, crc)?;
        write_u32(&mut out, bytes.len() as u32)?;
        write_u32(&mut out, bytes.len() as u32)?;
        write_u16(&mut out, name.len() as u16)?;
        write_u16(&mut out, 0)?;
        out.write_all(name)?;
        out.write_all(bytes)?;
        central.push((name.to_vec(), crc, bytes.len() as u32, offset));
        offset += 30 + name.len() as u32 + bytes.len() as u32;
    }
    let central_offset = offset;
    for (name, crc, size, local_offset) in &central {
        write_u32(&mut out, 0x02014b50)?;
        write_u16(&mut out, 20)?;
        write_u16(&mut out, 20)?;
        for value in [0, 0, 0, 0] {
            write_u16(&mut out, value)?;
        }
        write_u32(&mut out, *crc)?;
        write_u32(&mut out, *size)?;
        write_u32(&mut out, *size)?;
        write_u16(&mut out, name.len() as u16)?;
        for value in [0, 0, 0, 0, 0] {
            write_u16(&mut out, value)?;
        }
        write_u32(&mut out, 0)?;
        write_u32(&mut out, *local_offset)?;
        out.write_all(name)?;
        offset += 46 + name.len() as u32;
    }
    write_u32(&mut out, 0x06054b50)?;
    write_u16(&mut out, 0)?;
    write_u16(&mut out, 0)?;
    write_u16(&mut out, central.len() as u16)?;
    write_u16(&mut out, central.len() as u16)?;
    write_u32(&mut out, offset - central_offset)?;
    write_u32(&mut out, central_offset)?;
    write_u16(&mut out, 0)
}
fn write_u16(out: &mut impl Write, value: u16) -> io::Result<()> {
    out.write_all(&value.to_le_bytes())
}
fn write_u32(out: &mut impl Write, value: u32) -> io::Result<()> {
    out.write_all(&value.to_le_bytes())
}
fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for byte in bytes {
        crc ^= *byte as u32;
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb88320 & (0u32.wrapping_sub(crc & 1)));
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_comments_without_touching_strings() {
        assert_eq!(
            strip_json_comments("/*x*/[{\"url\":\"//x\"}]"),
            "[{\"url\":\"//x\"}]"
        );
    }

    #[test]
    fn extract_metadata_reads_theme_name_and_version() {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir_all(temp.path().join("config")).unwrap();
        fs::write(
            temp.path().join("config/settings_schema.json"),
            r#"[{"name": "theme_info", "theme_name": "Dawn", "theme_version": "7.0.2"}]"#,
        )
        .unwrap();

        let metadata = extract_metadata(temp.path()).unwrap();
        assert_eq!(metadata.theme_name, "Dawn");
        assert_eq!(metadata.theme_version, Some("7.0.2".into()));
    }

    #[test]
    fn extract_metadata_supports_nested_theme_info_object() {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir_all(temp.path().join("config")).unwrap();
        fs::write(
            temp.path().join("config/settings_schema.json"),
            r#"[{"theme_info": {"theme_name": "Test", "theme_version": "1.0"}}]"#,
        )
        .unwrap();

        let metadata = extract_metadata(temp.path()).unwrap();
        assert_eq!(metadata.theme_name, "Test");
        assert_eq!(metadata.theme_version, Some("1.0".into()));
    }

    #[test]
    fn extract_metadata_fails_when_schema_is_missing() {
        let temp = tempfile::tempdir().unwrap();
        let result = extract_metadata(temp.path());
        assert!(matches!(result, Err(PackageError::MissingSchema)));
    }

    #[test]
    fn extract_metadata_fails_when_theme_name_is_missing() {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir_all(temp.path().join("config")).unwrap();
        fs::write(
            temp.path().join("config/settings_schema.json"),
            r#"[{"name": "other"}]"#,
        )
        .unwrap();

        let result = extract_metadata(temp.path());
        assert!(matches!(result, Err(PackageError::MissingThemeName)));
    }

    #[test]
    fn extract_metadata_fails_on_invalid_json() {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir_all(temp.path().join("config")).unwrap();
        fs::write(
            temp.path().join("config/settings_schema.json"),
            r#"[{"name": "theme_info", "theme_name": "Dawn", "theme_version": "7.0.2""#,
        )
        .unwrap();

        let result = extract_metadata(temp.path());
        assert!(matches!(result, Err(PackageError::InvalidJson(_))));
    }

    #[test]
    fn package_theme_creates_zip_with_correct_name() {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir_all(temp.path().join("assets")).unwrap();
        fs::write(temp.path().join("assets/base.css"), "body{}").unwrap();
        fs::create_dir_all(temp.path().join("config")).unwrap();
        fs::write(
            temp.path().join("config/settings_schema.json"),
            r#"[{"name": "theme_info", "theme_name": "Dawn", "theme_version": "7.0.2"}]"#,
        )
        .unwrap();
        fs::create_dir_all(temp.path().join("layout")).unwrap();
        fs::write(
            temp.path().join("layout/theme.liquid"),
            "{{ content_for_header }}",
        )
        .unwrap();

        let zip_path = package_theme(temp.path()).unwrap();
        assert!(zip_path.ends_with("Dawn-7.0.2.zip"));
        assert!(zip_path.exists());
    }

    #[test]
    fn package_theme_creates_zip_without_version_when_missing() {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir_all(temp.path().join("assets")).unwrap();
        fs::write(temp.path().join("assets/base.css"), "body{}").unwrap();
        fs::create_dir_all(temp.path().join("config")).unwrap();
        fs::write(
            temp.path().join("config/settings_schema.json"),
            r#"[{"name": "theme_info", "theme_name": "Dawn"}]"#,
        )
        .unwrap();

        let zip_path = package_theme(temp.path()).unwrap();
        assert!(zip_path.ends_with("Dawn.zip"));
    }

    #[test]
    fn package_theme_includes_listings_release_notes_and_update_extension() {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir_all(temp.path().join("assets")).unwrap();
        fs::write(temp.path().join("assets/base.css"), "body{}").unwrap();
        fs::create_dir_all(temp.path().join("config")).unwrap();
        fs::write(
            temp.path().join("config/settings_schema.json"),
            r#"[{"name": "theme_info", "theme_name": "Dawn", "theme_version": "1.0.0"}]"#,
        )
        .unwrap();
        fs::create_dir_all(temp.path().join("listings/b2b/templates")).unwrap();
        fs::write(temp.path().join("listings/b2b/templates/index.json"), "{}").unwrap();
        fs::write(temp.path().join("release-notes.md"), "# notes").unwrap();
        fs::write(temp.path().join("update_extension.json"), "{}").unwrap();

        let zip_path = package_theme(temp.path()).unwrap();
        let bytes = fs::read(&zip_path).unwrap();
        let as_text = String::from_utf8_lossy(&bytes);
        assert!(as_text.contains("listings/b2b/templates/index.json"));
        assert!(as_text.contains("release-notes.md"));
        assert!(as_text.contains("update_extension.json"));
        assert!(as_text.contains("assets/base.css"));
    }

    #[test]
    fn package_theme_excludes_invalid_directories() {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir_all(temp.path().join("assets")).unwrap();
        fs::write(temp.path().join("assets/base.css"), "body{}").unwrap();
        fs::create_dir_all(temp.path().join("config/unsupported_dir")).unwrap();
        fs::write(temp.path().join("config/unsupported_dir/extra.json"), "{}").unwrap();
        fs::create_dir_all(temp.path().join("config")).unwrap();
        fs::write(
            temp.path().join("config/settings_schema.json"),
            r#"[{"name": "theme_info", "theme_name": "Dawn"}]"#,
        )
        .unwrap();

        let zip_path = package_theme(temp.path()).unwrap();
        let zip_bytes = fs::read(zip_path).unwrap();
        let zip_str = String::from_utf8_lossy(&zip_bytes);
        assert!(!zip_str.contains("unsupported_dir"));
        assert!(zip_str.contains("assets/base.css"));
    }

    #[test]
    fn valid_package_theme_key_accepts_valid_keys() {
        assert!(valid_package_theme_key("assets/theme.css"));
        assert!(valid_package_theme_key("layout/theme.liquid"));
        assert!(valid_package_theme_key("config/settings_schema.json"));
        assert!(valid_package_theme_key("templates/index.json"));
        assert!(valid_package_theme_key("templates/customers/account.json"));
    }

    #[test]
    fn valid_package_theme_key_rejects_invalid_keys() {
        assert!(!valid_package_theme_key("README.md"));
        assert!(!valid_package_theme_key("assets/node_modules/package.js"));
        assert!(!valid_package_theme_key("config/extra/file.json"));
        assert!(!valid_package_theme_key(""));
    }
}
