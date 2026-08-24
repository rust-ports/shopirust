//! Minimal LiquidJS-compatible helpers used by `app init` / `app generate`.
//!
//! Implements the subset exercised by Shopify CLI templates:
//! - `{{ variable }}` / `{{variable}}` interpolation (dot-path lookups)
//! - Recursive copy with `.liquid` rendering, `.raw` passthrough, and `.cli-liquid-bypass`

use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum LiquidError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Message(String),
}

/// Render a Liquid template string with a JSON object of variables.
pub fn render_liquid_template(template_content: &str, data: &Value) -> Result<String, LiquidError> {
    let mut out = String::with_capacity(template_content.len());
    let bytes = template_content.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'{' && i + 1 < bytes.len() && bytes[i + 1] == b'{' {
            if let Some(end) = find_close(bytes, i + 2) {
                let expr = template_content[i + 2..end].trim();
                let rendered = lookup_path(data, expr)
                    .map(value_to_string)
                    .unwrap_or_default();
                out.push_str(&rendered);
                i = end + 2;
                continue;
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    Ok(out)
}

fn find_close(bytes: &[u8], start: usize) -> Option<usize> {
    let mut j = start;
    while j + 1 < bytes.len() {
        if bytes[j] == b'}' && bytes[j + 1] == b'}' {
            return Some(j);
        }
        j += 1;
    }
    None
}

fn lookup_path<'a>(data: &'a Value, path: &str) -> Option<&'a Value> {
    let mut cur = data;
    for part in path.split('.').filter(|p| !p.is_empty()) {
        cur = cur.get(part)?;
    }
    Some(cur)
}

fn value_to_string(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// Copy a template tree, rendering `*.liquid` files and stripping the extension.
pub fn recursive_liquid_template_copy(
    from: &Path,
    to: &Path,
    data: &Value,
) -> Result<(), LiquidError> {
    fs::create_dir_all(to)?;
    let bypass = load_bypass_patterns(from)?;
    copy_recursive(from, from, to, data, &bypass)?;
    Ok(())
}

fn load_bypass_patterns(from: &Path) -> Result<Vec<String>, LiquidError> {
    let bypass_path = from.join(".cli-liquid-bypass");
    if !bypass_path.exists() {
        return Ok(Vec::new());
    }
    let content = fs::read_to_string(bypass_path)?;
    Ok(content
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(|l| l.trim_start_matches("./").to_string())
        .collect())
}

fn is_bypassed(rel: &str, patterns: &[String]) -> bool {
    patterns.iter().any(|p| {
        rel == p.as_str() || rel.starts_with(&format!("{p}/")) || match_glob_simple(rel, p)
    })
}

fn match_glob_simple(path: &str, pattern: &str) -> bool {
    if let Some(prefix) = pattern.strip_suffix("/**") {
        return path == prefix || path.starts_with(&format!("{prefix}/"));
    }
    if let Some(prefix) = pattern.strip_suffix("/*") {
        if let Some(rest) = path.strip_prefix(&format!("{prefix}/")) {
            return !rest.contains('/');
        }
        return false;
    }
    path == pattern
}

fn copy_recursive(
    root: &Path,
    current: &Path,
    to_root: &Path,
    data: &Value,
    bypass: &[String],
) -> Result<(), LiquidError> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");

        if rel == ".cli-liquid-bypass" {
            continue;
        }

        let rendered_rel = render_liquid_template(&rel, data)?;
        let output_path = to_root.join(&rendered_rel);
        let bypassed = is_bypassed(&rel, bypass);

        if path.is_dir() {
            fs::create_dir_all(&output_path)?;
            copy_recursive(root, &path, to_root, data, bypass)?;
            continue;
        }

        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)?;
        }

        if rel.ends_with(".liquid") && !bypassed {
            let content = fs::read_to_string(&path)?;
            let rendered = render_liquid_template(&content, data)?;
            let without_liquid = strip_suffix_path(&output_path, ".liquid");
            if let Some(parent) = without_liquid.parent() {
                fs::create_dir_all(parent)?;
            }
            // Preserve executable bit when present.
            #[cfg(unix)]
            let mode = {
                use std::os::unix::fs::PermissionsExt;
                fs::metadata(&path).ok().map(|m| m.permissions().mode())
            };
            fs::write(&without_liquid, rendered)?;
            #[cfg(unix)]
            if let Some(mode) = mode {
                if mode & 0o111 != 0 {
                    use std::os::unix::fs::PermissionsExt;
                    fs::set_permissions(&without_liquid, fs::Permissions::from_mode(mode))?;
                }
            }
        } else if rel.ends_with(".raw") {
            let without_raw = strip_suffix_path(&output_path, ".raw");
            fs::copy(&path, &without_raw)?;
        } else {
            fs::copy(&path, &output_path)?;
        }
    }
    Ok(())
}

fn strip_suffix_path(path: &Path, suffix: &str) -> PathBuf {
    let s = path.to_string_lossy();
    if let Some(stripped) = s.strip_suffix(suffix) {
        PathBuf::from(stripped)
    } else {
        path.to_path_buf()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::tempdir;

    #[test]
    fn renders_variable() {
        let got = render_liquid_template("{{variable}}", &json!({"variable": "test"})).unwrap();
        assert_eq!(got, "test");
    }

    #[test]
    fn renders_nested_path() {
        let got =
            render_liquid_template("Hi {{user.name}}", &json!({"user": {"name": "Ada"}})).unwrap();
        assert_eq!(got, "Hi Ada");
    }

    #[test]
    fn recursive_copy_renders_liquid_only() {
        let dir = tempdir().unwrap();
        let from = dir.path().join("from");
        let packages = from.join("packages");
        let to = dir.path().join("to");
        fs::create_dir_all(&packages).unwrap();
        fs::write(from.join("first.md.liquid"), "# {{variable}}").unwrap();
        fs::write(from.join("second.liquid.raw"), "# {{literal}}").unwrap();
        fs::write(packages.join("package.json"), r#"{"name":"package"}"#).unwrap();

        recursive_liquid_template_copy(&from, &to, &json!({"variable": "test"})).unwrap();

        assert_eq!(fs::read_to_string(to.join("first.md")).unwrap(), "# test");
        assert_eq!(
            fs::read_to_string(to.join("second.liquid")).unwrap(),
            "# {{literal}}"
        );
        assert_eq!(
            fs::read_to_string(to.join("packages/package.json")).unwrap(),
            r#"{"name":"package"}"#
        );
    }

    #[test]
    fn bypass_skips_liquid_rendering() {
        let dir = tempdir().unwrap();
        let from = dir.path().join("from");
        let ignored = from.join("ignored-folder");
        let to = dir.path().join("to");
        fs::create_dir_all(&ignored).unwrap();
        fs::write(
            from.join(".cli-liquid-bypass"),
            "ignored.liquid\nignored-folder\n",
        )
        .unwrap();
        fs::write(from.join("ignored.liquid"), "# {{variable}}").unwrap();
        fs::write(ignored.join("ignored2.liquid"), "# {{variable}}").unwrap();
        fs::write(from.join("processed.md.liquid"), "# {{variable}}").unwrap();

        recursive_liquid_template_copy(&from, &to, &json!({"variable": "test"})).unwrap();

        assert_eq!(
            fs::read_to_string(to.join("ignored.liquid")).unwrap(),
            "# {{variable}}"
        );
        assert_eq!(
            fs::read_to_string(to.join("ignored-folder/ignored2.liquid")).unwrap(),
            "# {{variable}}"
        );
        assert_eq!(
            fs::read_to_string(to.join("processed.md")).unwrap(),
            "# test"
        );
    }
}
