//! Query-flag helpers for `app execute` / bulk commands.
//!
//! Upstream `utilities/execute-command-helpers.ts` also wires `linkedAppContext` +
//! `storeContext`; those live in `services::{context, store_context}`. This module
//! covers the unit-tested query loading / validation surface.

use crate::error::AppError;
use std::path::Path;

/// Load a GraphQL operation from `--query` or `--query-file`.
pub fn resolve_graphql_query(
    query: Option<&str>,
    query_file: Option<&Path>,
) -> Result<String, AppError> {
    if let Some(q) = query {
        if q.trim().is_empty() {
            return Err(AppError::message(
                "The --query flag value is empty. Please provide a valid GraphQL query or mutation.",
            ));
        }
        return Ok(q.to_string());
    }
    if let Some(path) = query_file {
        if !path.is_file() {
            return Err(AppError::message(format!(
                "Query file not found at {}. Please check the path and try again.",
                path.display()
            )));
        }
        let contents = std::fs::read_to_string(path)?;
        if contents.trim().is_empty() {
            return Err(AppError::message(format!(
                "Query file at {} is empty. Please provide a valid GraphQL query or mutation.",
                path.display()
            )));
        }
        return Ok(contents);
    }
    Err(AppError::message(
        "Provide --query or --query-file for the GraphQL operation",
    ))
}

/// Reject documents with more than one operation (query/mutation/subscription).
pub fn validate_single_operation(query: &str) -> Result<(), AppError> {
    let mut ops = 0;
    for token in query.split_whitespace() {
        let t = token.trim_start_matches(|c: char| c == '{' || c == '(');
        if matches!(t, "query" | "mutation" | "subscription") {
            ops += 1;
        }
    }
    // Shorthand `{ shop { name } }` is a single anonymous query.
    if ops == 0 && query.trim_start().starts_with('{') {
        ops = 1;
    }
    if ops > 1 {
        return Err(AppError::message(
            "The GraphQL document must contain exactly one operation.",
        ));
    }
    if ops == 0 {
        return Err(AppError::message(
            "The GraphQL document must contain a query or mutation.",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn uses_query_flag() {
        let q = resolve_graphql_query(Some("{ shop { name } }"), None).unwrap();
        assert!(q.contains("shop"));
    }

    #[test]
    fn rejects_empty_query_flag() {
        let err = resolve_graphql_query(Some("   "), None).unwrap_err();
        assert!(err.to_string().contains("empty"));
    }

    #[test]
    fn loads_query_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("q.graphql");
        fs::write(&path, "query { shop { name } }").unwrap();
        let q = resolve_graphql_query(None, Some(&path)).unwrap();
        assert!(q.contains("shop"));
    }

    #[test]
    fn rejects_missing_query_file() {
        let err = resolve_graphql_query(None, Some(Path::new("/no/such/file.graphql"))).unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn rejects_empty_query_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("empty.graphql");
        fs::write(&path, "  \n").unwrap();
        let err = resolve_graphql_query(None, Some(&path)).unwrap_err();
        assert!(err.to_string().contains("is empty"));
    }

    #[test]
    fn requires_one_source() {
        let err = resolve_graphql_query(None, None).unwrap_err();
        assert!(err.to_string().contains("--query"));
    }

    #[test]
    fn accepts_single_named_operation() {
        validate_single_operation("query Shop { shop { name } }").unwrap();
        validate_single_operation("{ shop { name } }").unwrap();
        validate_single_operation("mutation M { shop { id } }").unwrap();
    }

    #[test]
    fn rejects_multiple_operations() {
        let err = validate_single_operation(
            "query A { shop { name } } mutation B { shop { id } }",
        )
        .unwrap_err();
        assert!(err.to_string().contains("exactly one"));
    }

    #[test]
    fn rejects_empty_document() {
        assert!(validate_single_operation("").is_err());
    }
}
