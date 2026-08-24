pub mod bp_source;
pub mod result;
pub mod types;

use crate::error::StoreError;

pub use bp_source::{
    list_business_platform_stores, parse_accessible_shops_response, AccessibleShopNode,
    AccessibleShopsPage, StoreListBpSource, LIST_ACCESSIBLE_SHOPS_QUERY,
};
pub use result::{format_store_list, render_store_list_result, StoreListRender};
pub use types::{
    ListStoresOptions, ListStoresResult, OrganizationsAccessInfo, StoreListEntry, StoreListOrg,
    StoreListOrganization, STORE_LIST_LIMIT,
};

pub fn limit_entries<T: Clone>(entries: Vec<T>, has_more: bool) -> (Vec<T>, bool) {
    if entries.len() > STORE_LIST_LIMIT {
        return (entries.into_iter().take(STORE_LIST_LIMIT).collect(), true);
    }
    (entries, has_more)
}

pub fn list_stores(
    entries: Vec<types::StoreListEntry>,
    organization: Option<StoreListOrganization>,
    has_more: bool,
) -> ListStoresResult {
    let (stores, truncated) = limit_entries(entries, has_more);
    ListStoresResult {
        stores,
        source: "organization".into(),
        organization,
        notice: None,
        truncated,
    }
}

pub fn select_store_list_organization<'a>(
    organizations: &'a [StoreListOrg],
    organization_id: Option<&str>,
    is_tty: bool,
) -> Result<Selection<'a>, StoreError> {
    if organizations.is_empty() {
        return Err(StoreError::message("No organizations available."));
    }
    if let Some(wanted) = organization_id {
        let selected = organizations
            .iter()
            .find(|o| o.id == wanted)
            .ok_or_else(|| {
                let available = organizations
                    .iter()
                    .map(|o| format!("{} ({})", o.business_name, o.id))
                    .collect::<Vec<_>>()
                    .join(", ");
                StoreError::with_try(
                    format!("Organization with ID {wanted} not found."),
                    format!("Available organizations: {available}"),
                )
            })?;
        return Ok(Selection::Resolved(selected));
    }
    if organizations.len() == 1 {
        return Ok(Selection::Resolved(&organizations[0]));
    }
    if !is_tty {
        return Err(StoreError::with_try(
            "An organization ID is required in non-interactive mode.",
            "Provide `--organization-id`, for example `--organization-id 1234567`. Run `shopify organization list` to find IDs.",
        ));
    }
    let unique_names: std::collections::HashSet<_> = organizations
        .iter()
        .map(|o| o.business_name.as_str())
        .collect();
    let has_duplicate_names = unique_names.len() < organizations.len();
    let choices = organizations
        .iter()
        .map(|organization| OrganizationChoice {
            label: if has_duplicate_names {
                format!("{} ({})", organization.business_name, organization.id)
            } else {
                organization.business_name.clone()
            },
            value: organization.id.clone(),
        })
        .collect();
    Ok(Selection::NeedsPrompt { choices })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrganizationChoice {
    pub label: String,
    pub value: String,
}

#[derive(Debug)]
pub enum Selection<'a> {
    Resolved(&'a StoreListOrg),
    NeedsPrompt { choices: Vec<OrganizationChoice> },
}

/// Select an organization from a list, matching `--organization-id` when given.
/// Non-interactive only (no TTY prompt). Kept for older call sites.
pub fn select_organization<'a, T>(
    organizations: &'a [T],
    organization_id: Option<&str>,
    id_of: impl Fn(&T) -> &str,
) -> Result<&'a T, StoreError> {
    if organizations.is_empty() {
        return Err(StoreError::message("No organizations available."));
    }
    if let Some(wanted) = organization_id {
        return organizations
            .iter()
            .find(|o| id_of(o) == wanted)
            .ok_or_else(|| {
                StoreError::message(format!("Organization with ID {wanted} not found."))
            });
    }
    if organizations.len() == 1 {
        return Ok(&organizations[0]);
    }
    Err(StoreError::message(
        "An organization ID is required to list stores non-interactively.\nProvide `--organization-id`.",
    ))
}

#[async_trait::async_trait]
pub trait StoreListIo: Send + Sync {
    async fn fetch_organizations(&self) -> Result<OrganizationsAccessInfo, StoreError>;
    async fn list_bp_stores(
        &self,
        organization: &StoreListOrg,
    ) -> Result<bp_source::BusinessPlatformStoreListResult, StoreError>;
    async fn prompt_organization(
        &self,
        choices: &[OrganizationChoice],
    ) -> Result<String, StoreError>;
    fn is_tty(&self) -> bool;
}

pub async fn list_stores_service(
    options: ListStoresOptions,
    io: &dyn StoreListIo,
) -> Result<ListStoresResult, StoreError> {
    let organizations_result = io.fetch_organizations().await?;
    if !organizations_result.current_user_resolved {
        return Ok(ListStoresResult {
            stores: vec![],
            source: "organization".into(),
            organization: None,
            notice: Some("Couldn't resolve a Shopify account for the current CLI session.".into()),
            truncated: false,
        });
    }
    if organizations_result.organizations.is_empty() {
        return Ok(ListStoresResult {
            stores: vec![],
            source: "organization".into(),
            organization: None,
            notice: None,
            truncated: false,
        });
    }

    let selected = match select_store_list_organization(
        &organizations_result.organizations,
        options.organization_id.as_deref(),
        io.is_tty(),
    )? {
        Selection::Resolved(org) => org.clone(),
        Selection::NeedsPrompt { choices } => {
            let selected_id = io.prompt_organization(&choices).await?;
            organizations_result
                .organizations
                .iter()
                .find(|o| o.id == selected_id)
                .cloned()
                .ok_or_else(|| StoreError::message("Selected organization was not found."))?
        }
    };

    let result = io.list_bp_stores(&selected).await?;
    let (stores, truncated) = limit_entries(result.entries, result.has_more);
    Ok(ListStoresResult {
        stores,
        source: "organization".into(),
        organization: Some(StoreListOrganization {
            id: selected.id,
            name: selected.business_name,
        }),
        notice: None,
        truncated,
    })
}

/// Convenience when the BP source is already available without prompt plumbing.
pub async fn list_business_platform_stores_for_org(
    source: &dyn StoreListBpSource,
    organization: &StoreListOrg,
) -> Result<bp_source::BusinessPlatformStoreListResult, StoreError> {
    list_business_platform_stores(source, organization).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::list::bp_source::{
        AccessibleShopNode, AccessibleShopsPage, BusinessPlatformStoreListResult,
    };
    use std::sync::Mutex;

    #[derive(Default)]
    struct FakeIo {
        orgs: Mutex<Option<OrganizationsAccessInfo>>,
        bp: Mutex<Option<Result<BusinessPlatformStoreListResult, StoreError>>>,
        prompt: Mutex<Option<Result<String, StoreError>>>,
        tty: bool,
        bp_calls: Mutex<Vec<String>>,
        prompts: Mutex<u32>,
    }

    #[async_trait::async_trait]
    impl StoreListIo for FakeIo {
        async fn fetch_organizations(&self) -> Result<OrganizationsAccessInfo, StoreError> {
            self.orgs
                .lock()
                .unwrap()
                .clone()
                .ok_or_else(|| StoreError::message("orgs not configured"))
        }
        async fn list_bp_stores(
            &self,
            organization: &StoreListOrg,
        ) -> Result<BusinessPlatformStoreListResult, StoreError> {
            self.bp_calls.lock().unwrap().push(organization.id.clone());
            match self.bp.lock().unwrap().clone() {
                Some(Ok(v)) => Ok(v),
                Some(Err(e)) => Err(e),
                None => Ok(BusinessPlatformStoreListResult {
                    entries: vec![],
                    has_more: false,
                }),
            }
        }
        async fn prompt_organization(
            &self,
            _choices: &[OrganizationChoice],
        ) -> Result<String, StoreError> {
            *self.prompts.lock().unwrap() += 1;
            match self.prompt.lock().unwrap().clone() {
                Some(Ok(v)) => Ok(v),
                Some(Err(e)) => Err(e),
                None => Err(StoreError::message("prompt not configured")),
            }
        }
        fn is_tty(&self) -> bool {
            self.tty
        }
    }

    fn acme() -> StoreListOrg {
        StoreListOrg {
            id: "1234".into(),
            business_name: "Acme".into(),
        }
    }
    fn beta() -> StoreListOrg {
        StoreListOrg {
            id: "5678".into(),
            business_name: "Beta".into(),
        }
    }
    fn org_entry() -> types::StoreListEntry {
        types::StoreListEntry {
            id: Some("gid://shopify/Shop/1".into()),
            store: "shop.myshopify.com".into(),
            created_at: "2026-01-15T00:00:00Z".into(),
            organization_id: "1234".into(),
            organization_name: "Acme".into(),
            name: Some("Shop".into()),
            store_type: Some("production".into()),
        }
    }

    #[tokio::test]
    async fn returns_organization_results_for_only_org() {
        let io = FakeIo {
            orgs: Mutex::new(Some(OrganizationsAccessInfo {
                organizations: vec![acme()],
                current_user_resolved: true,
            })),
            bp: Mutex::new(Some(Ok(BusinessPlatformStoreListResult {
                entries: vec![org_entry()],
                has_more: false,
            }))),
            prompt: Mutex::new(Some(Ok("1234".into()))),
            tty: false,
            ..Default::default()
        };
        let result = list_stores_service(ListStoresOptions::default(), &io)
            .await
            .unwrap();
        assert_eq!(*io.bp_calls.lock().unwrap(), vec!["1234".to_string()]);
        assert_eq!(*io.prompts.lock().unwrap(), 0);
        assert_eq!(
            result.organization,
            Some(StoreListOrganization {
                id: "1234".into(),
                name: "Acme".into(),
            })
        );
        assert_eq!(result.stores, vec![org_entry()]);
        assert!(!result.truncated);
    }

    #[tokio::test]
    async fn uses_requested_organization_id() {
        let io = FakeIo {
            orgs: Mutex::new(Some(OrganizationsAccessInfo {
                organizations: vec![acme(), beta()],
                current_user_resolved: true,
            })),
            bp: Mutex::new(Some(Ok(BusinessPlatformStoreListResult {
                entries: vec![],
                has_more: false,
            }))),
            prompt: Mutex::new(Some(Ok("5678".into()))),
            tty: true,
            ..Default::default()
        };
        let _ = list_stores_service(
            ListStoresOptions {
                organization_id: Some("5678".into()),
            },
            &io,
        )
        .await
        .unwrap();
        assert_eq!(*io.bp_calls.lock().unwrap(), vec!["5678".to_string()]);
        assert_eq!(*io.prompts.lock().unwrap(), 0);
    }

    #[tokio::test]
    async fn prompts_when_multiple_and_tty() {
        let io = FakeIo {
            orgs: Mutex::new(Some(OrganizationsAccessInfo {
                organizations: vec![acme(), beta()],
                current_user_resolved: true,
            })),
            bp: Mutex::new(Some(Ok(BusinessPlatformStoreListResult {
                entries: vec![],
                has_more: false,
            }))),
            prompt: Mutex::new(Some(Ok("5678".into()))),
            tty: true,
            ..Default::default()
        };
        let result = list_stores_service(ListStoresOptions::default(), &io)
            .await
            .unwrap();
        assert_eq!(*io.prompts.lock().unwrap(), 1);
        assert_eq!(
            result.organization,
            Some(StoreListOrganization {
                id: "5678".into(),
                name: "Beta".into(),
            })
        );
    }

    #[tokio::test]
    async fn requires_id_non_interactively() {
        let io = FakeIo {
            orgs: Mutex::new(Some(OrganizationsAccessInfo {
                organizations: vec![acme(), beta()],
                current_user_resolved: true,
            })),
            bp: Mutex::new(Some(Ok(BusinessPlatformStoreListResult {
                entries: vec![],
                has_more: false,
            }))),
            prompt: Mutex::new(Some(Ok("5678".into()))),
            tty: false,
            ..Default::default()
        };
        let err = list_stores_service(ListStoresOptions::default(), &io)
            .await
            .unwrap_err();
        assert!(err
            .to_string()
            .contains("An organization ID is required in non-interactive mode."));
        assert!(io.bp_calls.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn notice_when_session_unresolved() {
        let io = FakeIo {
            orgs: Mutex::new(Some(OrganizationsAccessInfo {
                organizations: vec![],
                current_user_resolved: false,
            })),
            ..Default::default()
        };
        let result = list_stores_service(ListStoresOptions::default(), &io)
            .await
            .unwrap();
        assert!(result.notice.unwrap().contains("Couldn't resolve"));
        assert!(result.stores.is_empty());
    }

    #[tokio::test]
    async fn empty_when_no_organizations() {
        let io = FakeIo {
            orgs: Mutex::new(Some(OrganizationsAccessInfo {
                organizations: vec![],
                current_user_resolved: true,
            })),
            ..Default::default()
        };
        let result = list_stores_service(ListStoresOptions::default(), &io)
            .await
            .unwrap();
        assert!(result.stores.is_empty());
        assert!(result.notice.is_none());
    }

    #[tokio::test]
    async fn throws_when_org_not_found() {
        let io = FakeIo {
            orgs: Mutex::new(Some(OrganizationsAccessInfo {
                organizations: vec![acme()],
                current_user_resolved: true,
            })),
            ..Default::default()
        };
        let err = list_stores_service(
            ListStoresOptions {
                organization_id: Some("9999999".into()),
            },
            &io,
        )
        .await
        .unwrap_err();
        assert!(err
            .to_string()
            .contains("Organization with ID 9999999 not found."));
    }

    #[tokio::test]
    async fn caps_at_limit_and_flags_truncation() {
        let entries = (0..251)
            .map(|index| types::StoreListEntry {
                id: None,
                store: format!("shop-{index}.myshopify.com"),
                created_at: "2026-01-15T00:00:00Z".into(),
                organization_id: "1234".into(),
                organization_name: "Acme".into(),
                name: None,
                store_type: None,
            })
            .collect::<Vec<_>>();
        let io = FakeIo {
            orgs: Mutex::new(Some(OrganizationsAccessInfo {
                organizations: vec![acme()],
                current_user_resolved: true,
            })),
            bp: Mutex::new(Some(Ok(BusinessPlatformStoreListResult {
                entries,
                has_more: false,
            }))),
            ..Default::default()
        };
        let result = list_stores_service(ListStoresOptions::default(), &io)
            .await
            .unwrap();
        assert_eq!(result.stores.len(), 250);
        assert!(result.truncated);
    }

    #[tokio::test]
    async fn flags_truncation_when_source_has_more() {
        let io = FakeIo {
            orgs: Mutex::new(Some(OrganizationsAccessInfo {
                organizations: vec![acme()],
                current_user_resolved: true,
            })),
            bp: Mutex::new(Some(Ok(BusinessPlatformStoreListResult {
                entries: vec![org_entry()],
                has_more: true,
            }))),
            ..Default::default()
        };
        let result = list_stores_service(ListStoresOptions::default(), &io)
            .await
            .unwrap();
        assert_eq!(result.stores.len(), 1);
        assert!(result.truncated);
    }

    #[test]
    fn truncates_over_limit_helper() {
        let entries: Vec<usize> = (0..STORE_LIST_LIMIT + 3).collect();
        let (limited, truncated) = limit_entries(entries, false);
        assert_eq!(limited.len(), STORE_LIST_LIMIT);
        assert!(truncated);
    }

    #[test]
    fn select_requires_id_when_multiple() {
        let orgs = vec!["1", "2"];
        assert!(select_organization(&orgs, None, |s| *s).is_err());
        assert_eq!(*select_organization(&orgs, Some("2"), |s| *s).unwrap(), "2");
    }

    #[test]
    fn select_single() {
        let orgs = vec!["only"];
        assert_eq!(*select_organization(&orgs, None, |s| *s).unwrap(), "only");
    }

    #[test]
    fn list_result_source() {
        let result = list_stores(vec![], None, false);
        assert_eq!(result.source, "organization");
        assert!(!result.truncated);
    }

    #[allow(dead_code)]
    fn _keep_shop_node_import() {
        let _ = AccessibleShopNode::default();
        let _ = AccessibleShopsPage::default();
    }
}
