use crate::error::StoreError;
use serde_json::Value;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationKind {
    Query,
    Mutation,
    Subscription,
}

#[derive(Debug, Clone)]
pub struct ParsedGraphQLOperation {
    pub kind: OperationKind,
}

#[derive(Debug, Clone)]
pub struct PreparedStoreExecuteRequest {
    pub query: String,
    pub parsed_operation: ParsedGraphQLOperation,
    pub variables: Option<Value>,
    pub requested_version: Option<String>,
}

pub fn read_query(query: Option<&str>, query_file: Option<&Path>) -> Result<String, StoreError> {
    if let Some(q) = query {
        if q.trim().is_empty() {
            return Err(StoreError::message(
                "The --query flag value is empty. Please provide a valid GraphQL query or mutation.",
            ));
        }
        return Ok(q.to_string());
    }
    if let Some(path) = query_file {
        if !path.exists() {
            return Err(StoreError::message(format!(
                "Query file not found at {}.",
                path.display()
            )));
        }
        let raw = fs::read_to_string(path)?;
        if raw.trim().is_empty() {
            return Err(StoreError::message(format!(
                "Query file at {} is empty.",
                path.display()
            )));
        }
        return Ok(raw);
    }
    Err(StoreError::message(
        "Query should have been provided via --query or --query-file flags due to exactlyOne constraint. This indicates the oclif flag validation failed.",
    ))
}

pub fn parse_variables(
    variables: Option<&str>,
    variable_file: Option<&Path>,
) -> Result<Option<Value>, StoreError> {
    if let Some(raw) = variables {
        return serde_json::from_str(raw).map(Some).map_err(|e| {
            StoreError::with_try(
                format!("Invalid JSON in --variables flag: {e}"),
                "Please provide valid JSON format.",
            )
        });
    }
    if let Some(path) = variable_file {
        if !path.exists() {
            return Err(StoreError::message(format!(
                "Variable file not found at {}.",
                path.display()
            )));
        }
        let raw = fs::read_to_string(path)?;
        return serde_json::from_str(&raw).map(Some).map_err(|e| {
            StoreError::with_try(
                format!("Invalid JSON in variable file {}: {e}", path.display()),
                "Please provide valid JSON format.",
            )
        });
    }
    Ok(None)
}

/// Lightweight GraphQL operation detector (no full parser dependency).
/// Strips comments/strings roughly and finds top-level operation keywords.
pub fn parse_graphql_operation(
    graphql_operation: &str,
) -> Result<ParsedGraphQLOperation, StoreError> {
    let stripped = strip_graphql_noise(graphql_operation);
    if stripped.trim().is_empty() {
        return Err(StoreError::message(
            "Invalid GraphQL syntax: empty document",
        ));
    }
    // Reject obviously broken brace balance as "Invalid GraphQL syntax".
    if !braces_balanced(&stripped) {
        return Err(StoreError::message(
            "Invalid GraphQL syntax: unbalanced braces",
        ));
    }

    let ops = find_top_level_operations(&stripped);
    if ops.is_empty() {
        // Anonymous query: `{ shop { name } }`
        if stripped.trim_start().starts_with('{') {
            return Ok(ParsedGraphQLOperation {
                kind: OperationKind::Query,
            });
        }
        return Err(StoreError::message(
            "Invalid GraphQL syntax: no operation found",
        ));
    }
    if ops.len() != 1 {
        return Err(StoreError::message(
            "GraphQL document must contain exactly one operation definition. Multiple operations are not supported.",
        ));
    }
    Ok(ParsedGraphQLOperation { kind: ops[0] })
}

fn strip_graphql_noise(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '#' {
            while let Some(n) = chars.peek() {
                if *n == '\n' {
                    break;
                }
                chars.next();
            }
            continue;
        }
        if c == '"' {
            out.push(c);
            while let Some(n) = chars.next() {
                out.push(n);
                if n == '\\' {
                    if let Some(escaped) = chars.next() {
                        out.push(escaped);
                    }
                    continue;
                }
                if n == '"' {
                    break;
                }
            }
            continue;
        }
        out.push(c);
    }
    out
}

fn braces_balanced(input: &str) -> bool {
    let mut depth = 0i32;
    for c in input.chars() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth < 0 {
                    return false;
                }
            }
            _ => {}
        }
    }
    depth == 0
}

fn find_top_level_operations(input: &str) -> Vec<OperationKind> {
    let mut ops = Vec::new();
    let mut depth = 0i32;
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i] as char;
        if c == '{' {
            depth += 1;
            i += 1;
            continue;
        }
        if c == '}' {
            depth -= 1;
            i += 1;
            continue;
        }
        if depth == 0 && c.is_ascii_alphabetic() {
            let start = i;
            while i < bytes.len() && (bytes[i] as char).is_ascii_alphanumeric() {
                i += 1;
            }
            let word = &input[start..i];
            match word {
                "query" => ops.push(OperationKind::Query),
                "mutation" => ops.push(OperationKind::Mutation),
                "subscription" => ops.push(OperationKind::Subscription),
                "fragment" => {}
                _ => {}
            }
            continue;
        }
        i += 1;
    }
    ops
}

fn validate_mutations_allowed(
    operation: &ParsedGraphQLOperation,
    allow_mutations: bool,
) -> Result<(), StoreError> {
    if operation.kind == OperationKind::Subscription {
        return Err(StoreError::message(
            "Subscriptions are not supported by shopify store execute.",
        ));
    }
    if operation.kind == OperationKind::Mutation && !allow_mutations {
        return Err(StoreError::with_try(
            "Mutations are disabled by default for shopify store execute.",
            "Re-run with --allow-mutations if you intend to modify store data.",
        ));
    }
    Ok(())
}

pub struct PrepareStoreExecuteInput<'a> {
    pub query: Option<&'a str>,
    pub query_file: Option<&'a Path>,
    pub variables: Option<&'a str>,
    pub variable_file: Option<&'a Path>,
    pub version: Option<&'a str>,
    pub allow_mutations: bool,
}

pub fn prepare_store_execute_request(
    input: PrepareStoreExecuteInput<'_>,
) -> Result<PreparedStoreExecuteRequest, StoreError> {
    let query = read_query(input.query, input.query_file)?;
    let parsed_operation = parse_graphql_operation(&query)?;
    validate_mutations_allowed(&parsed_operation, input.allow_mutations)?;
    let variables = parse_variables(input.variables, input.variable_file)?;
    Ok(PreparedStoreExecuteRequest {
        query,
        parsed_operation,
        variables,
        requested_version: input.version.map(str::to_string),
    })
}

/// Back-compat helper used by older call sites.
pub fn prepare_request(
    query: Option<&str>,
    query_file: Option<&Path>,
    variables: Option<&str>,
    version: Option<&str>,
) -> Result<PreparedStoreExecuteRequest, StoreError> {
    prepare_store_execute_request(PrepareStoreExecuteInput {
        query,
        query_file,
        variables,
        variable_file: None,
        version,
        allow_mutations: true,
    })
}

pub fn admin_graphql_url(shop_domain: &str, version: &str) -> String {
    let host = shop_domain.trim_end_matches('/');
    let host = host
        .strip_prefix("https://")
        .or_else(|| host.strip_prefix("http://"))
        .unwrap_or(host);
    format!("https://{host}/admin/api/{version}/graphql.json")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn returns_prepared_request_for_inline_query() {
        let request = prepare_store_execute_request(PrepareStoreExecuteInput {
            query: Some("query { shop { name } }"),
            query_file: None,
            variables: Some(r#"{"id":"gid://shopify/Shop/1"}"#),
            variable_file: None,
            version: Some("2025-07"),
            allow_mutations: false,
        })
        .unwrap();
        assert_eq!(request.query, "query { shop { name } }");
        assert_eq!(request.variables.unwrap()["id"], "gid://shopify/Shop/1");
        assert_eq!(request.requested_version.as_deref(), Some("2025-07"));
        assert_eq!(request.parsed_operation.kind, OperationKind::Query);
    }

    #[test]
    fn reads_query_from_file() {
        let mut f = NamedTempFile::new().unwrap();
        write!(f, "query {{ shop {{ name }} }}").unwrap();
        let request = prepare_store_execute_request(PrepareStoreExecuteInput {
            query: None,
            query_file: Some(f.path()),
            variables: None,
            variable_file: None,
            version: None,
            allow_mutations: false,
        })
        .unwrap();
        assert!(request.query.contains("shop"));
    }

    #[test]
    fn rejects_empty_query() {
        assert!(prepare_store_execute_request(PrepareStoreExecuteInput {
            query: Some("   "),
            query_file: None,
            variables: None,
            variable_file: None,
            version: None,
            allow_mutations: false,
        })
        .unwrap_err()
        .to_string()
        .contains("--query flag value is empty"));
    }

    #[test]
    fn rejects_missing_query() {
        assert!(prepare_store_execute_request(PrepareStoreExecuteInput {
            query: None,
            query_file: None,
            variables: None,
            variable_file: None,
            version: None,
            allow_mutations: false,
        })
        .unwrap_err()
        .to_string()
        .contains("Query should have been provided"));
    }

    #[test]
    fn rejects_invalid_syntax() {
        assert!(prepare_store_execute_request(PrepareStoreExecuteInput {
            query: Some("query {"),
            query_file: None,
            variables: None,
            variable_file: None,
            version: None,
            allow_mutations: false,
        })
        .unwrap_err()
        .to_string()
        .contains("Invalid GraphQL syntax"));
    }

    #[test]
    fn rejects_multiple_operations() {
        assert!(prepare_store_execute_request(PrepareStoreExecuteInput {
            query: Some("query First { shop { name } } query Second { shop { id } }"),
            query_file: None,
            variables: None,
            variable_file: None,
            version: None,
            allow_mutations: false,
        })
        .unwrap_err()
        .to_string()
        .contains("exactly one operation definition"));
    }

    #[test]
    fn rejects_mutation_by_default() {
        assert!(prepare_store_execute_request(PrepareStoreExecuteInput {
            query: Some(
                r#"mutation { productCreate(product: {title: "Hat"}) { product { id } } }"#
            ),
            query_file: None,
            variables: None,
            variable_file: None,
            version: None,
            allow_mutations: false,
        })
        .unwrap_err()
        .to_string()
        .contains("Mutations are disabled by default"));
    }

    #[test]
    fn allows_mutations_when_enabled() {
        let request = prepare_store_execute_request(PrepareStoreExecuteInput {
            query: Some(
                r#"mutation { productCreate(product: {title: "Hat"}) { product { id } } }"#,
            ),
            query_file: None,
            variables: None,
            variable_file: None,
            version: None,
            allow_mutations: true,
        })
        .unwrap();
        assert_eq!(request.parsed_operation.kind, OperationKind::Mutation);
    }

    #[test]
    fn rejects_invalid_variables_json() {
        assert!(prepare_store_execute_request(PrepareStoreExecuteInput {
            query: Some("query { shop { name } }"),
            query_file: None,
            variables: Some("{invalid json}"),
            variable_file: None,
            version: None,
            allow_mutations: false,
        })
        .unwrap_err()
        .to_string()
        .contains("Invalid JSON"));
    }

    #[test]
    fn reads_variables_from_file() {
        let mut f = NamedTempFile::new().unwrap();
        write!(f, r#"{{"id":"1"}}"#).unwrap();
        let request = prepare_store_execute_request(PrepareStoreExecuteInput {
            query: Some("query { shop { name } }"),
            query_file: None,
            variables: None,
            variable_file: Some(f.path()),
            version: None,
            allow_mutations: false,
        })
        .unwrap();
        assert_eq!(request.variables.unwrap()["id"], "1");
    }

    #[test]
    fn admin_url() {
        assert_eq!(
            admin_graphql_url("dev.myshopify.com", "2026-01"),
            "https://dev.myshopify.com/admin/api/2026-01/graphql.json"
        );
    }
}
