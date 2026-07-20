use crate::types::*;

/// Parse a .graphql operation file.
pub fn parse_graphql(content: &str) -> Option<GraphqlOperation> {
    let bytes = content.as_bytes();
    let mut pos = 0;

    // Skip whitespace and comments
    pos = skip_ws(bytes, pos);

    // Determine operation type: `query` or `mutation`
    let operation_type = if bytes[pos..].starts_with(b"query") {
        pos += 5;
        GraphqlOperationType::Query
    } else if bytes[pos..].starts_with(b"mutation") {
        pos += 8;
        GraphqlOperationType::Mutation
    } else {
        return None;
    };

    pos = skip_ws(bytes, pos);

    // Read operation name
    let operation_name = read_name(bytes, &mut pos);
    if operation_name.is_empty() {
        return None;
    }

    pos = skip_ws(bytes, pos);

    // Parse variables: `($name: String!, $source: URL!, ...)`
    let variables = if pos < bytes.len() && bytes[pos] == b'(' {
        pos += 1;
        parse_variables(bytes, &mut pos)
    } else {
        Vec::new()
    };

    pos = skip_ws(bytes, pos);

    // Capture the raw query: everything from `{` to the last `}`
    if pos >= bytes.len() || bytes[pos] != b'{' {
        return None;
    }

    // Find the matching closing brace
    let query_start = pos;
    let mut depth = 0;
    let mut query_end = pos;
    while query_end < bytes.len() {
        if bytes[query_end] == b'{' {
            depth += 1;
        } else if bytes[query_end] == b'}' {
            depth -= 1;
            if depth == 0 {
                query_end += 1;
                break;
            }
        }
        query_end += 1;
    }
    if depth != 0 {
        return None;
    }

    let raw_query = String::from_utf8_lossy(&bytes[query_start..query_end])
        .trim()
        .to_string();

    Some(GraphqlOperation {
        operation_type,
        operation_name,
        raw_query,
        variables,
    })
}

fn parse_variables(bytes: &[u8], pos: &mut usize) -> Vec<GraphqlVariable> {
    let mut vars = Vec::new();
    loop {
        *pos = skip_ws(bytes, *pos);
        if *pos >= bytes.len() || bytes[*pos] == b')' {
            *pos += 1;
            break;
        }
        if bytes[*pos] == b'$' {
            *pos += 1;
        }
        let name = read_name(bytes, pos);
        if name.is_empty() {
            break;
        }
        *pos = skip_ws(bytes, *pos);
        if *pos >= bytes.len() || bytes[*pos] != b':' {
            break;
        }
        *pos += 1;
        *pos = skip_ws(bytes, *pos);
        // Parse type (handles both `TypeName` and `[TypeName!]!`)
        let gql_type = read_gql_type(bytes, pos);
        if gql_type.is_empty() {
            break;
        }
        let non_null = *pos < bytes.len() && bytes[*pos] == b'!';
        if non_null {
            *pos += 1;
        }
        vars.push(GraphqlVariable {
            name,
            gql_type,
            non_null,
        });
    }
    vars
}

/// Read a GraphQL type expression (including list types like `[Type!]!`).
fn read_gql_type(bytes: &[u8], pos: &mut usize) -> String {
    let _start = *pos;
    if *pos < bytes.len() && bytes[*pos] == b'[' {
        *pos += 1;
        let inner = read_gql_type(bytes, pos);
        // Skip any `!` before `]`
        if *pos < bytes.len() && bytes[*pos] == b'!' {
            *pos += 1;
        }
        if *pos < bytes.len() && bytes[*pos] == b']' {
            *pos += 1;
        }
        format!("[{inner}]")
    } else {
        read_name(bytes, pos)
    }
}

fn skip_ws(bytes: &[u8], mut pos: usize) -> usize {
    while pos < bytes.len()
        && (bytes[pos] == b' '
            || bytes[pos] == b'\t'
            || bytes[pos] == b'\n'
            || bytes[pos] == b'\r'
            || bytes[pos] == b',')
    {
        pos += 1;
    }
    // Skip comments
    while pos + 1 < bytes.len() && bytes[pos] == b'#' {
        while pos < bytes.len() && bytes[pos] != b'\n' {
            pos += 1;
        }
        pos = skip_ws(bytes, pos);
    }
    pos
}

fn read_name(bytes: &[u8], pos: &mut usize) -> String {
    let start = *pos;
    while *pos < bytes.len() && (bytes[*pos].is_ascii_alphanumeric() || bytes[*pos] == b'_') {
        *pos += 1;
    }
    String::from_utf8_lossy(&bytes[start..*pos]).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_query() {
        let input =
            "query getTheme($id: ID!) {\n  theme(id: $id) {\n    id\n    name\n    role\n  }\n}";
        let op = parse_graphql(input).unwrap();
        assert_eq!(op.operation_name, "getTheme");
        assert!(matches!(op.operation_type, GraphqlOperationType::Query));
        assert!(!op.raw_query.is_empty());
        assert!(op.raw_query.contains("theme(id: $id)"));
    }

    #[test]
    fn test_parse_mutation() {
        let input = "mutation themeCreate($name: String!, $source: URL!, $role: ThemeRole!) {\n  themeCreate(name: $name, source: $source, role: $role) {\n    theme { id name role }\n    userErrors { field message }\n  }\n}";
        let op = parse_graphql(input).unwrap();
        assert_eq!(op.operation_name, "themeCreate");
        assert!(matches!(op.operation_type, GraphqlOperationType::Mutation));
    }
}
