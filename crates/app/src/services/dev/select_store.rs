//! Convert a transfer-disabled store so it can be used for `app dev`.

use crate::error::AppError;
use cli_api::{DeveloperPlatformClient, OrganizationStore};

/// When the selected store is transfer-disabled but convertible, run the Partners conversion.
pub async fn convert_dev_to_transfer_disabled_store(
    client: &dyn DeveloperPlatformClient,
    store: &OrganizationStore,
) -> Result<OrganizationStore, AppError> {
    if store.transfer_disabled {
        return Ok(store.clone());
    }
    if !store.convertable_to_partner_test {
        return Err(AppError::message(format!(
            "Store {} cannot be converted to a transfer-disabled development store.",
            store.shop_domain
        )));
    }
    let converted = client
        .convert_to_transfer_disabled_store(store.link.as_deref().unwrap_or("0"), &store.shop_id)
        .await
        .map_err(|e| AppError::message(e.to_string()))?;
    if !converted {
        return Err(AppError::message(format!(
            "Failed to convert store {} to a development store.",
            store.shop_domain
        )));
    }
    let mut out = store.clone();
    out.transfer_disabled = true;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{sample_org_app, MockClient};

    #[tokio::test]
    async fn already_disabled_is_noop() {
        let client = MockClient::with_app(sample_org_app("k"));
        let store = crate::test_support::sample_store("1", "dev.myshopify.com");
        let out = convert_dev_to_transfer_disabled_store(&client, &store)
            .await
            .unwrap();
        assert!(out.transfer_disabled);
    }

    #[tokio::test]
    async fn converts_when_allowed() {
        let client = MockClient::with_app(sample_org_app("k"));
        let mut store = crate::test_support::sample_store("1", "dev.myshopify.com");
        store.transfer_disabled = false;
        store.convertable_to_partner_test = true;
        let out = convert_dev_to_transfer_disabled_store(&client, &store)
            .await
            .unwrap();
        assert!(out.transfer_disabled);
    }

    #[tokio::test]
    async fn rejects_non_convertible() {
        let client = MockClient::with_app(sample_org_app("k"));
        let mut store = crate::test_support::sample_store("1", "live.myshopify.com");
        store.transfer_disabled = false;
        store.convertable_to_partner_test = false;
        assert!(convert_dev_to_transfer_disabled_store(&client, &store)
            .await
            .is_err());
    }
}
