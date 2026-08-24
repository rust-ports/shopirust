use crate::error::StoreError;
use crate::store_type::store_type_handle;
use crate::url::extract_host;

use super::types::{StoreListEntry, StoreListOrg, STORE_LIST_LIMIT};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AccessibleShopNode {
    pub shopify_shop_id: Option<String>,
    pub name: Option<String>,
    pub store_type: Option<String>,
    pub primary_domain: Option<String>,
    pub url: Option<String>,
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AccessibleShopsPage {
    pub nodes: Vec<AccessibleShopNode>,
    pub has_next_page: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BusinessPlatformStoreListResult {
    pub entries: Vec<StoreListEntry>,
    pub has_more: bool,
}

#[async_trait::async_trait]
pub trait StoreListBpSource: Send + Sync {
    async fn fetch_accessible_shops(
        &self,
        organization_id: &str,
        first: usize,
    ) -> Result<AccessibleShopsPage, StoreError>;
}

pub fn to_store_list_entry(
    node: &AccessibleShopNode,
    organization: &StoreListOrg,
) -> Option<StoreListEntry> {
    let store = node.url.as_deref().or(node.primary_domain.as_deref())?;
    let host = extract_host(store).unwrap_or_else(|| store.to_string());
    Some(StoreListEntry {
        id: node
            .shopify_shop_id
            .as_deref()
            .filter(|s| !s.is_empty())
            .map(|id| format!("gid://shopify/Shop/{id}")),
        store: host,
        created_at: node.created_at.clone().unwrap_or_default(),
        organization_id: organization.id.clone(),
        organization_name: organization.business_name.clone(),
        name: node.name.clone(),
        store_type: store_type_handle(node.store_type.as_deref()),
    })
}

pub fn by_created_at_descending(
    left: &StoreListEntry,
    right: &StoreListEntry,
) -> std::cmp::Ordering {
    match right.created_at.cmp(&left.created_at) {
        std::cmp::Ordering::Equal => left.store.cmp(&right.store),
        other => other,
    }
}

pub async fn list_business_platform_stores(
    source: &dyn StoreListBpSource,
    organization: &StoreListOrg,
) -> Result<BusinessPlatformStoreListResult, StoreError> {
    let page = source
        .fetch_accessible_shops(&organization.id, STORE_LIST_LIMIT)
        .await?;
    let mut entries: Vec<_> = page
        .nodes
        .iter()
        .filter_map(|node| to_store_list_entry(node, organization))
        .collect();
    entries.sort_by(by_created_at_descending);
    Ok(BusinessPlatformStoreListResult {
        entries,
        has_more: page.has_next_page,
    })
}

pub const LIST_ACCESSIBLE_SHOPS_QUERY: &str = r#"
query ListAccessibleShops($first: Int!) {
  organization {
    id
    name
    accessibleShops(
      first: $first
      sort: SHOP_CREATED_AT_DESC
      filters: [{field: STORE_STATUS, operator: EQUALS, value: "active"}]
    ) {
      edges {
        node {
          id
          shopifyShopId
          name
          storeType
          primaryDomain
          url
          createdAt
        }
      }
      pageInfo {
        hasNextPage
      }
    }
  }
}
"#;

pub fn parse_accessible_shops_response(value: &serde_json::Value) -> AccessibleShopsPage {
    let shops = value.pointer("/organization/accessibleShops");
    let Some(shops) = shops else {
        return AccessibleShopsPage::default();
    };
    let edges = shops
        .get("edges")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let nodes = edges
        .into_iter()
        .filter_map(|edge| {
            let n = edge.get("node")?;
            Some(AccessibleShopNode {
                shopify_shop_id: n
                    .get("shopifyShopId")
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
                name: n.get("name").and_then(|v| v.as_str()).map(str::to_string),
                store_type: n
                    .get("storeType")
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
                primary_domain: n
                    .get("primaryDomain")
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
                url: n.get("url").and_then(|v| v.as_str()).map(str::to_string),
                created_at: n.get("createdAt").map(|v| match v {
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                }),
            })
        })
        .collect();
    let has_next_page = shops
        .pointer("/pageInfo/hasNextPage")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    AccessibleShopsPage {
        nodes,
        has_next_page,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct FakeSource {
        page: Mutex<Result<AccessibleShopsPage, StoreError>>,
        calls: Mutex<Vec<(String, usize)>>,
    }

    #[async_trait::async_trait]
    impl StoreListBpSource for FakeSource {
        async fn fetch_accessible_shops(
            &self,
            organization_id: &str,
            first: usize,
        ) -> Result<AccessibleShopsPage, StoreError> {
            self.calls
                .lock()
                .unwrap()
                .push((organization_id.to_string(), first));
            match &*self.page.lock().unwrap() {
                Ok(v) => Ok(v.clone()),
                Err(e) => Err(e.clone()),
            }
        }
    }

    fn node(overrides: impl FnOnce(&mut AccessibleShopNode)) -> AccessibleShopNode {
        let mut n = AccessibleShopNode {
            shopify_shop_id: Some("1".into()),
            name: Some("Acme Production".into()),
            store_type: Some("PRODUCTION".into()),
            primary_domain: Some("acme.myshopify.com".into()),
            url: None,
            created_at: Some("2026-01-15T00:00:00Z".into()),
        };
        overrides(&mut n);
        n
    }

    #[tokio::test]
    async fn fetches_active_stores_for_organization() {
        let source = FakeSource {
            page: Mutex::new(Ok(AccessibleShopsPage {
                nodes: vec![node(|_| {})],
                has_next_page: false,
            })),
            calls: Mutex::new(vec![]),
        };
        let org = StoreListOrg {
            id: "1234".into(),
            business_name: "Acme".into(),
        };
        let result = list_business_platform_stores(&source, &org).await.unwrap();
        assert_eq!(*source.calls.lock().unwrap(), vec![("1234".into(), 250)]);
        assert_eq!(
            result.entries,
            vec![StoreListEntry {
                id: Some("gid://shopify/Shop/1".into()),
                store: "acme.myshopify.com".into(),
                created_at: "2026-01-15T00:00:00Z".into(),
                organization_id: "1234".into(),
                organization_name: "Acme".into(),
                name: Some("Acme Production".into()),
                store_type: Some("production".into()),
            }]
        );
        assert!(!result.has_more);
    }

    #[tokio::test]
    async fn uses_selected_organization_name() {
        let source = FakeSource {
            page: Mutex::new(Ok(AccessibleShopsPage {
                nodes: vec![node(|n| {
                    n.primary_domain = Some("beta.myshopify.com".into());
                })],
                has_next_page: false,
            })),
            calls: Mutex::new(vec![]),
        };
        let org = StoreListOrg {
            id: "5678".into(),
            business_name: "Beta".into(),
        };
        let result = list_business_platform_stores(&source, &org).await.unwrap();
        assert_eq!(result.entries[0].organization_name, "Beta");
    }

    #[tokio::test]
    async fn skips_shops_without_url_or_domain() {
        let source = FakeSource {
            page: Mutex::new(Ok(AccessibleShopsPage {
                nodes: vec![node(|n| {
                    n.primary_domain = None;
                    n.url = None;
                    n.name = Some("Missing Domain Shop".into());
                })],
                has_next_page: false,
            })),
            calls: Mutex::new(vec![]),
        };
        let org = StoreListOrg {
            id: "1234".into(),
            business_name: "Acme".into(),
        };
        let result = list_business_platform_stores(&source, &org).await.unwrap();
        assert!(result.entries.is_empty());
    }

    #[tokio::test]
    async fn orders_newest_first() {
        let source = FakeSource {
            page: Mutex::new(Ok(AccessibleShopsPage {
                nodes: vec![
                    node(|n| {
                        n.shopify_shop_id = Some("1".into());
                        n.name = Some("Older Shop".into());
                        n.primary_domain = Some("older.myshopify.com".into());
                        n.created_at = Some("2025-01-01T00:00:00Z".into());
                    }),
                    node(|n| {
                        n.shopify_shop_id = Some("2".into());
                        n.name = Some("Newer Shop".into());
                        n.store_type = Some("DEVELOPMENT".into());
                        n.primary_domain = Some("newer.myshopify.com".into());
                        n.created_at = Some("2026-05-01T00:00:00Z".into());
                    }),
                ],
                has_next_page: false,
            })),
            calls: Mutex::new(vec![]),
        };
        let org = StoreListOrg {
            id: "1234".into(),
            business_name: "Acme".into(),
        };
        let result = list_business_platform_stores(&source, &org).await.unwrap();
        assert_eq!(
            result
                .entries
                .iter()
                .map(|e| e.store.as_str())
                .collect::<Vec<_>>(),
            vec!["newer.myshopify.com", "older.myshopify.com"]
        );
    }

    #[tokio::test]
    async fn sorts_matching_dates_by_host() {
        let source = FakeSource {
            page: Mutex::new(Ok(AccessibleShopsPage {
                nodes: vec![
                    node(|n| {
                        n.shopify_shop_id = Some("2".into());
                        n.primary_domain = Some("b-shop.myshopify.com".into());
                        n.created_at = Some("2026-05-01T00:00:00Z".into());
                    }),
                    node(|n| {
                        n.shopify_shop_id = Some("1".into());
                        n.primary_domain = Some("a-shop.myshopify.com".into());
                        n.created_at = Some("2026-05-01T00:00:00Z".into());
                    }),
                ],
                has_next_page: false,
            })),
            calls: Mutex::new(vec![]),
        };
        let org = StoreListOrg {
            id: "1234".into(),
            business_name: "Acme".into(),
        };
        let result = list_business_platform_stores(&source, &org).await.unwrap();
        assert_eq!(
            result
                .entries
                .iter()
                .map(|e| e.store.as_str())
                .collect::<Vec<_>>(),
            vec!["a-shop.myshopify.com", "b-shop.myshopify.com"]
        );
    }

    #[tokio::test]
    async fn reports_has_more() {
        let source = FakeSource {
            page: Mutex::new(Ok(AccessibleShopsPage {
                nodes: vec![node(|_| {})],
                has_next_page: true,
            })),
            calls: Mutex::new(vec![]),
        };
        let org = StoreListOrg {
            id: "1234".into(),
            business_name: "Acme".into(),
        };
        let result = list_business_platform_stores(&source, &org).await.unwrap();
        assert!(result.has_more);
    }

    #[test]
    fn query_filters_active_status() {
        assert!(LIST_ACCESSIBLE_SHOPS_QUERY.contains("STORE_STATUS"));
        assert!(LIST_ACCESSIBLE_SHOPS_QUERY.contains("active"));
        assert!(LIST_ACCESSIBLE_SHOPS_QUERY.contains("SHOP_CREATED_AT_DESC"));
    }
}
