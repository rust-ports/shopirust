use crate::error::StoreError;

fn store_auth_command(store: &str, scopes: &str) -> String {
    format!("shopify store auth --store {store} --scopes {scopes}")
}

pub fn stored_store_auth_error(store: &str) -> StoreError {
    StoreError::with_try(
        format!("No stored app authentication found for {store}."),
        format!(
            "To create stored auth for this store, run:\n{}",
            store_auth_command(store, "<comma-separated-scopes>")
        ),
    )
}

pub fn reauthenticate_store_auth_error(message: &str, store: &str, scopes: &str) -> StoreError {
    StoreError::with_try(
        message.to_string(),
        format!(
            "To re-authenticate, run:\n{}",
            store_auth_command(store, scopes)
        ),
    )
}

pub fn retry_store_auth_with_permanent_domain_error(returned_store: &str) -> StoreError {
    StoreError::with_try(
        "OAuth callback store does not match the requested store.",
        format!(
            "Shopify returned {returned_store} during authentication. Re-run using the permanent store domain:\n{}",
            store_auth_command(returned_store, "<comma-separated-scopes>")
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stored_auth_error_mentions_command() {
        let err = stored_store_auth_error("shop.myshopify.com");
        assert!(err
            .to_string()
            .contains("No stored app authentication found"));
        assert!(err
            .to_string()
            .contains("shopify store auth --store shop.myshopify.com"));
    }

    #[test]
    fn permanent_domain_error() {
        let err = retry_store_auth_with_permanent_domain_error("other-shop.myshopify.com");
        assert!(err
            .to_string()
            .contains("OAuth callback store does not match"));
        assert!(err.to_string().contains("other-shop.myshopify.com"));
    }
}
