use crate::error::StoreError;
use crate::gid::numeric_id_from_encoded_gid;
use crate::url::{extract_host, extract_search_subdomain};

use super::types::{DestinationNode, DestinationsContext, OwningOrgInternal, OwningOrgRaw};

#[async_trait::async_trait]
pub trait DestinationsSource: Send + Sync {
    async fn search_destinations(&self, search: &str) -> Result<Vec<DestinationNode>, StoreError>;
    async fn fetch_owning_org(
        &self,
        destination_public_id: &str,
    ) -> Result<Option<OwningOrgRaw>, StoreError>;
}

pub fn destination_search_term(store: &str) -> String {
    let target_host = extract_host(store).unwrap_or_else(|| store.to_lowercase());
    extract_search_subdomain(&target_host)
}

pub fn matches_store(node: &DestinationNode, target_host: &str) -> bool {
    [&node.primary_domain, &node.web_url]
        .into_iter()
        .flatten()
        .any(|value| extract_host(value).as_deref() == Some(target_host))
}

pub async fn fetch_destinations_context(
    store: &str,
    source: &dyn DestinationsSource,
) -> Result<DestinationsContext, StoreError> {
    let target_host = extract_host(store).unwrap_or_else(|| store.to_lowercase());
    let search = extract_search_subdomain(&target_host);
    let nodes = source.search_destinations(&search).await?;
    let matched = nodes.iter().find(|node| matches_store(node, &target_host));
    let Some(matched) = matched else {
        return Err(StoreError::bp_store_not_found(store));
    };
    let owning_org = resolve_owning_org(source, &matched.public_id).await;
    Ok(DestinationsContext { owning_org })
}

async fn resolve_owning_org(
    source: &dyn DestinationsSource,
    destination_public_id: &str,
) -> Option<OwningOrgInternal> {
    match source.fetch_owning_org(destination_public_id).await {
        Ok(Some(org)) => Some(OwningOrgInternal {
            id: org.id.as_deref().and_then(numeric_id_from_encoded_gid),
            name: org.name,
        }),
        Ok(None) | Err(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use std::sync::Mutex;

    struct FakeSource {
        nodes: Vec<DestinationNode>,
        org: Result<Option<OwningOrgRaw>, StoreError>,
        searches: Mutex<Vec<String>>,
        org_ids: Mutex<Vec<String>>,
    }

    #[async_trait::async_trait]
    impl DestinationsSource for FakeSource {
        async fn search_destinations(
            &self,
            search: &str,
        ) -> Result<Vec<DestinationNode>, StoreError> {
            self.searches.lock().unwrap().push(search.to_string());
            Ok(self.nodes.clone())
        }
        async fn fetch_owning_org(
            &self,
            destination_public_id: &str,
        ) -> Result<Option<OwningOrgRaw>, StoreError> {
            self.org_ids
                .lock()
                .unwrap()
                .push(destination_public_id.to_string());
            match &self.org {
                Ok(v) => Ok(v.clone()),
                Err(e) => Err(e.clone()),
            }
        }
    }

    fn node(overrides: impl FnOnce(&mut DestinationNode)) -> DestinationNode {
        let mut n = DestinationNode {
            public_id: "dest-public-1".into(),
            primary_domain: Some("https://shop.myshopify.com".into()),
            web_url: Some("https://shop.myshopify.com/admin".into()),
        };
        overrides(&mut n);
        n
    }

    #[tokio::test]
    async fn throws_when_no_destination_matches() {
        let source = FakeSource {
            nodes: vec![],
            org: Ok(None),
            searches: Mutex::new(vec![]),
            org_ids: Mutex::new(vec![]),
        };
        let err = fetch_destinations_context("shop.myshopify.com", &source)
            .await
            .unwrap_err();
        assert!(matches!(err, StoreError::BpStoreNotFound(_)));
        assert!(err.to_string().contains("shop.myshopify.com"));
    }

    #[tokio::test]
    async fn throws_when_domain_match_is_missing() {
        let source = FakeSource {
            nodes: vec![node(|n| {
                n.primary_domain = Some("https://other.myshopify.com".into());
                n.web_url = Some("https://other.myshopify.com/admin".into());
            })],
            org: Ok(None),
            searches: Mutex::new(vec![]),
            org_ids: Mutex::new(vec![]),
        };
        let err = fetch_destinations_context("shop.myshopify.com", &source)
            .await
            .unwrap_err();
        assert!(matches!(err, StoreError::BpStoreNotFound(_)));
    }

    #[tokio::test]
    async fn searches_with_subdomain() {
        let source = FakeSource {
            nodes: vec![node(|_| {})],
            org: Ok(Some(OwningOrgRaw {
                id: None,
                name: "Org".into(),
            })),
            searches: Mutex::new(vec![]),
            org_ids: Mutex::new(vec![]),
        };
        let _ = fetch_destinations_context("shop.myshopify.com", &source)
            .await
            .unwrap();
        assert_eq!(*source.searches.lock().unwrap(), vec!["shop".to_string()]);
    }

    #[tokio::test]
    async fn extracts_subdomain_for_local_dev() {
        let dev = "my-dev-store.shop.dev";
        let source = FakeSource {
            nodes: vec![node(|n| {
                n.primary_domain = Some(format!("https://{dev}"));
                n.web_url = Some(format!("https://{dev}/admin"));
            })],
            org: Ok(Some(OwningOrgRaw {
                id: None,
                name: "Org".into(),
            })),
            searches: Mutex::new(vec![]),
            org_ids: Mutex::new(vec![]),
        };
        let _ = fetch_destinations_context(dev, &source).await.unwrap();
        assert_eq!(
            *source.searches.lock().unwrap(),
            vec!["my-dev-store".to_string()]
        );
    }

    #[tokio::test]
    async fn resolves_owning_org_via_public_id() {
        let encoded =
            base64::engine::general_purpose::STANDARD.encode("gid://organization/Organization/123");
        let source = FakeSource {
            nodes: vec![node(|_| {})],
            org: Ok(Some(OwningOrgRaw {
                id: Some(encoded),
                name: "Acme Org".into(),
            })),
            searches: Mutex::new(vec![]),
            org_ids: Mutex::new(vec![]),
        };
        let ctx = fetch_destinations_context("shop.myshopify.com", &source)
            .await
            .unwrap();
        assert_eq!(
            *source.org_ids.lock().unwrap(),
            vec!["dest-public-1".to_string()]
        );
        assert_eq!(
            ctx.owning_org,
            Some(OwningOrgInternal {
                name: "Acme Org".into(),
                id: Some("123".into()),
            })
        );
    }

    #[tokio::test]
    async fn leaves_owning_org_undefined_when_org_request_throws() {
        let source = FakeSource {
            nodes: vec![node(|_| {})],
            org: Err(StoreError::message("boom")),
            searches: Mutex::new(vec![]),
            org_ids: Mutex::new(vec![]),
        };
        let ctx = fetch_destinations_context("shop.myshopify.com", &source)
            .await
            .unwrap();
        assert!(ctx.owning_org.is_none());
    }

    #[tokio::test]
    async fn leaves_owning_org_undefined_when_org_missing() {
        let source = FakeSource {
            nodes: vec![node(|_| {})],
            org: Ok(None),
            searches: Mutex::new(vec![]),
            org_ids: Mutex::new(vec![]),
        };
        let ctx = fetch_destinations_context("shop.myshopify.com", &source)
            .await
            .unwrap();
        assert!(ctx.owning_org.is_none());
    }
}
