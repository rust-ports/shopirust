use crate::auth::token_client::StoreTokenResponse;
use crate::error::StoreError;

pub fn parse_store_auth_scopes(input: &str) -> Result<Vec<String>, StoreError> {
    let mut scopes = Vec::new();
    for part in input.split(|c: char| c == ',' || c.is_whitespace()) {
        if part.is_empty() {
            continue;
        }
        if !scopes.iter().any(|s| s == part) {
            scopes.push(part.to_string());
        }
    }
    if scopes.is_empty() {
        return Err(StoreError::with_try(
            "At least one scope is required.",
            "Pass --scopes as a comma-separated list.",
        ));
    }
    Ok(scopes)
}

fn expand_implied_store_scopes(scopes: &[String]) -> std::collections::HashSet<String> {
    let mut expanded: std::collections::HashSet<String> = scopes.iter().cloned().collect();
    for scope in scopes {
        if let Some(caps) = write_scope_parts(scope) {
            expanded.insert(format!("{}read_{}", caps.0, caps.1));
        }
    }
    expanded
}

fn write_scope_parts(scope: &str) -> Option<(String, String)> {
    let unauth = "unauthenticated_";
    let (prefix, rest) = if let Some(stripped) = scope.strip_prefix(unauth) {
        (unauth.to_string(), stripped)
    } else {
        (String::new(), scope)
    };
    rest.strip_prefix("write_")
        .map(|suffix| (prefix, suffix.to_string()))
}

pub fn merge_requested_and_stored_scopes(
    requested_scopes: &[String],
    stored_scopes: &[String],
) -> Vec<String> {
    let mut merged = stored_scopes.to_vec();
    let mut expanded = expand_implied_store_scopes(stored_scopes);
    for scope in requested_scopes {
        if expanded.contains(scope) {
            continue;
        }
        merged.push(scope.clone());
        for expanded_scope in expand_implied_store_scopes(std::slice::from_ref(scope)) {
            expanded.insert(expanded_scope);
        }
    }
    merged
}

pub fn resolve_granted_scopes(
    token_response: &StoreTokenResponse,
    requested_scopes: &[String],
) -> Result<Vec<String>, StoreError> {
    let Some(scope) = token_response.scope.as_deref() else {
        return Ok(requested_scopes.to_vec());
    };
    let granted_scopes = parse_store_auth_scopes(scope)?;
    let expanded = expand_implied_store_scopes(&granted_scopes);
    let missing: Vec<_> = requested_scopes
        .iter()
        .filter(|scope| !expanded.contains(*scope))
        .cloned()
        .collect();
    if !missing.is_empty() {
        return Err(StoreError::with_try(
            "Shopify granted fewer scopes than were requested.",
            format!(
                "Missing scopes: {}.\nUpdate the app or store installation scopes.\nSee https://shopify.dev/app/scopes\nRe-run shopify store auth.",
                missing.join(", ")
            ),
        ));
    }
    Ok(granted_scopes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_splits_and_deduplicates() {
        assert_eq!(
            parse_store_auth_scopes("read_products, write_products,read_products").unwrap(),
            vec!["read_products", "write_products"]
        );
    }

    #[test]
    fn parse_space_separated() {
        assert_eq!(
            parse_store_auth_scopes("read_products read_inventory").unwrap(),
            vec!["read_products", "read_inventory"]
        );
    }

    #[test]
    fn parse_mixed_delimiters() {
        assert_eq!(
            parse_store_auth_scopes("read_products, read_inventory,write_orders").unwrap(),
            vec!["read_products", "read_inventory", "write_orders"]
        );
    }

    #[test]
    fn merge_avoids_redundant_reads() {
        assert_eq!(
            merge_requested_and_stored_scopes(
                &["read_products".into()],
                &["write_products".into()]
            ),
            vec!["write_products"]
        );
    }

    #[test]
    fn merge_adds_new_scopes() {
        assert_eq!(
            merge_requested_and_stored_scopes(&["read_products".into()], &["read_orders".into()]),
            vec!["read_orders", "read_products"]
        );
    }

    fn token(scope: Option<&str>) -> StoreTokenResponse {
        StoreTokenResponse {
            access_token: "token".into(),
            scope: scope.map(str::to_string),
            ..Default::default()
        }
    }

    #[test]
    fn resolve_space_separated_granted() {
        assert_eq!(
            resolve_granted_scopes(
                &token(Some("read_products read_inventory")),
                &["read_products".into(), "read_inventory".into()]
            )
            .unwrap(),
            vec!["read_products", "read_inventory"]
        );
    }

    #[test]
    fn resolve_write_implies_read() {
        assert_eq!(
            resolve_granted_scopes(
                &token(Some("write_products")),
                &["read_products".into(), "write_products".into()]
            )
            .unwrap(),
            vec!["write_products"]
        );
    }

    #[test]
    fn resolve_fallback_when_scope_omitted() {
        assert_eq!(
            resolve_granted_scopes(&token(None), &["read_products".into()]).unwrap(),
            vec!["read_products"]
        );
    }

    #[test]
    fn resolve_rejects_missing() {
        let err = resolve_granted_scopes(
            &token(Some("read_products")),
            &["read_products".into(), "write_products".into()],
        )
        .unwrap_err();
        assert!(err
            .to_string()
            .contains("Shopify granted fewer scopes than were requested."));
    }
}
