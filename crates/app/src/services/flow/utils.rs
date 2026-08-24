//! Flow URL resolution and schema loading (upstream `services/flow/utils.ts`).

use crate::error::AppError;
use crate::models::extensions::transform::prepend_application_url;
use std::fs;
use std::path::{Path, PathBuf};

fn is_https_url(url: &str) -> bool {
    url::Url::parse(url)
        .map(|u| u.scheme().eq_ignore_ascii_case("https"))
        .unwrap_or(false)
}

/// Resolve a Flow action URL by prepending the app URL to relative paths and requiring HTTPS.
pub fn resolve_flow_action_url(
    field_name: &str,
    url: &str,
    app_url: Option<&str>,
) -> Result<String, AppError> {
    let resolved = prepend_application_url(url, app_url.unwrap_or(""));
    if resolved.starts_with('/') {
        return Err(AppError::message(format!(
            "Flow action {field_name} is a relative URL, but no application_url is configured. \
             Set application_url in your app configuration or use an absolute HTTPS URL."
        )));
    }
    if !is_https_url(&resolved) {
        return Err(AppError::message(format!(
            "Flow action {field_name} must resolve to an HTTPS URL. \
             Set application_url to an HTTPS URL or use an absolute HTTPS URL."
        )));
    }
    Ok(resolved)
}

fn expand_schema_glob(extension_path: &Path, patch_path: &str) -> Result<Vec<PathBuf>, AppError> {
    let joined = extension_path.join(patch_path);
    if !patch_path.contains('*') && !patch_path.contains('?') {
        return Ok(if joined.is_file() {
            vec![joined]
        } else {
            vec![]
        });
    }

    let parent = joined.parent().unwrap_or(extension_path).to_path_buf();
    let pattern = joined.file_name().and_then(|s| s.to_str()).unwrap_or("*");
    let mut matches = Vec::new();
    let Ok(entries) = fs::read_dir(&parent) else {
        return Ok(matches);
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if glob_match(pattern, name) {
            matches.push(path);
        }
    }
    Ok(matches)
}

fn glob_match(pattern: &str, name: &str) -> bool {
    let mut p = pattern.chars().peekable();
    let mut n = name.chars().peekable();
    while let Some(pc) = p.next() {
        match pc {
            '*' => {
                if p.peek().is_none() {
                    return true;
                }
                let rest: String = p.collect();
                let name_rest: String = n.collect();
                for (i, _) in name_rest
                    .char_indices()
                    .chain(std::iter::once((name_rest.len(), '\0')))
                {
                    if glob_match(&rest, &name_rest[i..]) {
                        return true;
                    }
                }
                return false;
            }
            '?' => {
                if n.next().is_none() {
                    return false;
                }
            }
            c => {
                if n.next() != Some(c) {
                    return false;
                }
            }
        }
    }
    n.next().is_none()
}

/// Load schema file contents from a partner-defined relative path / glob.
pub fn load_schema_from_path(
    extension_path: &Path,
    patch_path: Option<&str>,
) -> Result<String, AppError> {
    let Some(patch_path) = patch_path.filter(|p| !p.is_empty()) else {
        return Ok(String::new());
    };

    let matches = expand_schema_glob(extension_path, patch_path)?;
    if matches.len() > 1 {
        return Err(AppError::message("Multiple files found for schema path"));
    }
    if matches.is_empty() {
        return Err(AppError::message("No file found for schema path"));
    }
    Ok(fs::read_to_string(&matches[0])?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn resolve_absolute_https_unchanged() {
        assert_eq!(
            resolve_flow_action_url(
                "runtime_url",
                "https://my-prod-host.example.com/api/execute",
                Some("https://my-app.example.com"),
            )
            .unwrap(),
            "https://my-prod-host.example.com/api/execute"
        );
    }

    #[test]
    fn resolve_accepts_https_scheme_casing() {
        assert_eq!(
            resolve_flow_action_url(
                "runtime_url",
                "HTTPS://my-prod-host.example.com/api/execute",
                Some("https://my-app.example.com"),
            )
            .unwrap(),
            "HTTPS://my-prod-host.example.com/api/execute"
        );
    }

    #[test]
    fn resolve_prepends_app_url() {
        assert_eq!(
            resolve_flow_action_url(
                "runtime_url",
                "/api/execute",
                Some("https://my-app.example.com/"),
            )
            .unwrap(),
            "https://my-app.example.com/api/execute"
        );
    }

    #[test]
    fn resolve_errors() {
        let err = resolve_flow_action_url("runtime_url", "/api/execute", None).unwrap_err();
        assert!(err.to_string().contains("no application_url"));

        let err = resolve_flow_action_url(
            "runtime_url",
            "http://my-prod-host.example.com/api/execute",
            None,
        )
        .unwrap_err();
        assert!(err.to_string().contains("must resolve to an HTTPS URL"));

        let err = resolve_flow_action_url("runtime_url", "", Some("https://my-app.example.com"))
            .unwrap_err();
        assert!(err.to_string().contains("must resolve to an HTTPS URL"));

        let err = resolve_flow_action_url(
            "runtime_url",
            "/api/execute",
            Some("http://my-app.example.com"),
        )
        .unwrap_err();
        assert!(err.to_string().contains("must resolve to an HTTPS URL"));
    }

    #[test]
    fn load_schema_from_path_works() {
        let dir = tempdir().unwrap();
        let fixtures = dir.path().join("fixtures");
        fs::create_dir_all(&fixtures).unwrap();
        let mut f = fs::File::create(fixtures.join("valid-schema-patch.graphql")).unwrap();
        writeln!(f, "type Test {{ a: String }}").unwrap();
        fs::write(
            fixtures.join("valid-schema-patch2.graphql"),
            "type Other {}\n",
        )
        .unwrap();

        let content =
            load_schema_from_path(&fixtures, Some("./valid-schema-patch.graphql")).unwrap();
        assert!(content.contains("type Test"));

        assert!(load_schema_from_path(&fixtures, Some("*.graphql"))
            .unwrap_err()
            .to_string()
            .contains("Multiple files"));

        assert!(load_schema_from_path(&fixtures, Some("./missing.graphql"))
            .unwrap_err()
            .to_string()
            .contains("No file found"));

        assert_eq!(load_schema_from_path(&fixtures, Some("")).unwrap(), "");
        assert_eq!(load_schema_from_path(&fixtures, None).unwrap(), "");
    }
}
