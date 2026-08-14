use crate::error::StoreError;
use crate::url::extract_host;

use super::types::{OrganizationShopFields, OrganizationShopNode};

#[async_trait::async_trait]
pub trait OrganizationShopSource: Send + Sync {
    async fn search_organization_shops(
        &self,
        organization_id: &str,
        search: &str,
    ) -> Result<Vec<OrganizationShopNode>, StoreError>;
}

pub fn match_organization_shop(
    store: &str,
    nodes: &[OrganizationShopNode],
) -> Option<OrganizationShopFields> {
    let lower_store = store.to_lowercase();
    nodes.iter().find_map(|node| {
        let host = node.primary_domain.as_deref().and_then(extract_host);
        (host.as_deref() == Some(lower_store.as_str())).then(|| OrganizationShopFields {
            shopify_shop_id: node.shopify_shop_id.clone(),
            name: node.name.clone(),
            primary_domain: node.primary_domain.clone(),
            store_type: node.store_type.clone(),
            developer_preview_handle: node.developer_preview_handle.clone(),
            plan_name: node.plan_name.clone(),
            owner_name: node.owner_name.clone(),
            owner_email: node.owner_email.clone(),
        })
    })
}

pub async fn fetch_organization_shop(
    store: &str,
    organization_id: &str,
    source: &dyn OrganizationShopSource,
) -> Result<OrganizationShopFields, StoreError> {
    let nodes = source
        .search_organization_shops(organization_id, store)
        .await?;
    match_organization_shop(store, &nodes).ok_or_else(|| {
        StoreError::with_try(
            format!("Couldn't find shop {store} inside organization {organization_id}."),
            "The shop matched a global lookup but is not listed under its parent organization. This usually means the search index is stale; try again in a moment.",
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct FakeSource {
        nodes: Option<Vec<OrganizationShopNode>>,
        calls: Mutex<Vec<(String, String)>>,
    }

    #[async_trait::async_trait]
    impl OrganizationShopSource for FakeSource {
        async fn search_organization_shops(
            &self,
            organization_id: &str,
            search: &str,
        ) -> Result<Vec<OrganizationShopNode>, StoreError> {
            self.calls
                .lock()
                .unwrap()
                .push((organization_id.to_string(), search.to_string()));
            self.nodes
                .clone()
                .ok_or_else(|| StoreError::message("organization missing"))
        }
    }

    fn shop_node(overrides: impl FnOnce(&mut OrganizationShopNode)) -> OrganizationShopNode {
        let mut n = OrganizationShopNode {
            shopify_shop_id: Some("72193245184".into()),
            name: Some("My Shop".into()),
            primary_domain: Some("https://shop.myshopify.com".into()),
            store_type: Some("PRODUCTION".into()),
            developer_preview_handle: Some("extended_variants".into()),
            plan_name: Some("professional".into()),
            owner_name: Some("Jane Doe".into()),
            owner_email: Some("jane@acme.com".into()),
        };
        overrides(&mut n);
        n
    }

    #[tokio::test]
    async fn returns_the_matched_shop_node() {
        let source = FakeSource {
            nodes: Some(vec![shop_node(|_| {})]),
            calls: Mutex::new(vec![]),
        };
        let shop = fetch_organization_shop("shop.myshopify.com", "123", &source)
            .await
            .unwrap();
        assert_eq!(shop.name.as_deref(), Some("My Shop"));
        assert_eq!(
            shop.primary_domain.as_deref(),
            Some("https://shop.myshopify.com")
        );
        assert_eq!(shop.shopify_shop_id.as_deref(), Some("72193245184"));
        assert_eq!(shop.store_type.as_deref(), Some("PRODUCTION"));
        assert_eq!(
            shop.developer_preview_handle.as_deref(),
            Some("extended_variants")
        );
        assert_eq!(shop.plan_name.as_deref(), Some("professional"));
        assert_eq!(shop.owner_name.as_deref(), Some("Jane Doe"));
        assert_eq!(shop.owner_email.as_deref(), Some("jane@acme.com"));
    }

    #[tokio::test]
    async fn throws_when_no_shop_matches_the_domain() {
        let source = FakeSource {
            nodes: Some(vec![shop_node(|n| {
                n.primary_domain = Some("https://other.myshopify.com".into());
            })]),
            calls: Mutex::new(vec![]),
        };
        let err = fetch_organization_shop("shop.myshopify.com", "123", &source)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("Couldn't find shop"));
    }

    #[tokio::test]
    async fn passes_organization_id_and_search() {
        let source = FakeSource {
            nodes: Some(vec![shop_node(|_| {})]),
            calls: Mutex::new(vec![]),
        };
        let _ = fetch_organization_shop("shop.myshopify.com", "123", &source)
            .await
            .unwrap();
        assert_eq!(
            *source.calls.lock().unwrap(),
            vec![("123".into(), "shop.myshopify.com".into())]
        );
    }

    #[tokio::test]
    async fn throws_when_organization_is_missing() {
        let source = FakeSource {
            nodes: None,
            calls: Mutex::new(vec![]),
        };
        let err = fetch_organization_shop("shop.myshopify.com", "123", &source)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("organization missing"));
    }
}
