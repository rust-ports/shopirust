//! Schema version marker helpers for `schema.graphql`.

use crate::error::AppError;
use std::fs;
use std::path::Path;

/// Marker used in the leading comments of `schema.graphql`.
pub const SCHEMA_VERSION_MARKER_PREFIX: &str = "# api_version: ";

/// Prepends a versioned header to a schema definition.
pub fn prepend_schema_version_header(definition: &str, api_version: &str) -> String {
    format!("{SCHEMA_VERSION_MARKER_PREFIX}{api_version}\n\n{definition}")
}

/// Reads the `api_version` recorded in the leading comments of a schema file.
pub fn read_schema_api_version(file_path: &Path) -> Result<Option<String>, AppError> {
    if !file_path.is_file() {
        return Ok(None);
    }
    let contents = fs::read_to_string(file_path)?;
    let first_line = contents.lines().next().unwrap_or("");
    if let Some(rest) = first_line.strip_prefix(SCHEMA_VERSION_MARKER_PREFIX) {
        return Ok(Some(rest.trim().to_string()));
    }
    Ok(None)
}

/// Validates that `<extension>/schema.graphql` matches the declared `api_version`.
pub fn validate_schema_api_version(
    directory: &Path,
    local_identifier: &str,
    api_version: &str,
) -> Result<(), AppError> {
    let schema_path = directory.join("schema.graphql");
    let Some(version_from_schema) = read_schema_api_version(&schema_path)? else {
        return Ok(());
    };
    if version_from_schema == api_version {
        return Ok(());
    }
    Err(AppError::message(format!(
        "The schema.graphql file for {local_identifier} was generated for api_version {version_from_schema} \
         but your function is now on api_version {api_version}.\n\
         Run `shopify app function schema` to refresh it."
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn prepends_marker() {
        let result = prepend_schema_version_header("type Query { id: ID }", "2025-10");
        assert!(result.starts_with(&format!("{SCHEMA_VERSION_MARKER_PREFIX}2025-10\n")));
        assert!(result.ends_with("type Query { id: ID }"));
    }

    #[test]
    fn read_version_when_present() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("schema.graphql");
        fs::write(
            &path,
            prepend_schema_version_header("type Query { id: ID }", "2025-10"),
        )
        .unwrap();
        assert_eq!(
            read_schema_api_version(&path).unwrap().as_deref(),
            Some("2025-10")
        );
    }

    #[test]
    fn read_none_without_marker() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("schema.graphql");
        fs::write(&path, "# some other comment\ntype Query { id: ID }").unwrap();
        assert!(read_schema_api_version(&path).unwrap().is_none());
    }

    #[test]
    fn read_none_missing_file() {
        let dir = tempdir().unwrap();
        assert!(read_schema_api_version(&dir.path().join("missing.graphql"))
            .unwrap()
            .is_none());
    }

    #[test]
    fn read_ignores_buried_marker() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("schema.graphql");
        fs::write(
            &path,
            format!("type Query {{ id: ID }}\n{SCHEMA_VERSION_MARKER_PREFIX}2025-10\n"),
        )
        .unwrap();
        assert!(read_schema_api_version(&path).unwrap().is_none());
    }

    #[test]
    fn read_trims_whitespace() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("schema.graphql");
        fs::write(
            &path,
            format!("{SCHEMA_VERSION_MARKER_PREFIX}  2025-10  \ntype Query {{ id: ID }}"),
        )
        .unwrap();
        assert_eq!(
            read_schema_api_version(&path).unwrap().as_deref(),
            Some("2025-10")
        );
    }

    #[test]
    fn validate_noop_missing_or_unmarked() {
        let dir = tempdir().unwrap();
        validate_schema_api_version(dir.path(), "my-function", "2025-10").unwrap();
        fs::write(dir.path().join("schema.graphql"), "type Query { id: ID }").unwrap();
        validate_schema_api_version(dir.path(), "my-function", "2025-10").unwrap();
    }

    #[test]
    fn validate_ok_when_matching() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("schema.graphql"),
            prepend_schema_version_header("type Query { id: ID }", "2025-10"),
        )
        .unwrap();
        validate_schema_api_version(dir.path(), "my-function", "2025-10").unwrap();
    }

    #[test]
    fn validate_errors_when_stale() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("schema.graphql"),
            prepend_schema_version_header("type Query { id: ID }", "2025-07"),
        )
        .unwrap();
        let err = validate_schema_api_version(dir.path(), "my-function", "2025-10").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("2025-07"));
        assert!(msg.contains("2025-10"));
    }
}
