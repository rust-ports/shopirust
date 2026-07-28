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
}
