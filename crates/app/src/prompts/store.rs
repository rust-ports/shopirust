//! Store selection prompts (upstream `prompts/dev.ts` store bits).

use super::{PromptItem, Prompter};
use crate::error::AppError;
use cli_api::OrganizationStore;

/// Pick a development store. A single store is auto-selected.
pub fn select_store_prompt(
    prompter: &dyn Prompter,
    stores: &[OrganizationStore],
    show_domain: bool,
) -> Result<Option<OrganizationStore>, AppError> {
    if stores.is_empty() {
        return Ok(None);
    }
    if stores.len() == 1 {
        return Ok(Some(stores[0].clone()));
    }
    let items: Vec<_> = stores
        .iter()
        .map(|s| {
            let label = if show_domain {
                format!("{} ({})", s.shop_name, s.shop_domain)
            } else {
                s.shop_name.clone()
            };
            PromptItem::new(label, s.shop_id.clone())
        })
        .collect();
    let id = prompter.autocomplete("Which store is this for?", &items)?;
    Ok(stores.iter().find(|s| s.shop_id == id).cloned())
}

pub fn reload_store_list_prompt(prompter: &dyn Prompter, org_name: &str) -> Result<bool, AppError> {
    prompter.confirm(&format!("Have you created a new store in {org_name}?"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prompts::InjectedPrompter;

    fn store(id: &str, name: &str, domain: &str) -> OrganizationStore {
        OrganizationStore {
            shop_id: id.into(),
            shop_domain: domain.into(),
            shop_name: name.into(),
            transfer_disabled: true,
            convertable_to_partner_test: true,
            provisionable: false,
            link: None,
            store_type: Some("app_development".into()),
        }
    }

    #[test]
    fn empty_returns_none() {
        let p = InjectedPrompter::new();
        assert!(select_store_prompt(&p, &[], true).unwrap().is_none());
    }

    #[test]
    fn single_auto_select() {
        let p = InjectedPrompter::new();
        let s = select_store_prompt(&p, &[store("1", "Dev", "dev.myshopify.com")], true)
            .unwrap()
            .unwrap();
        assert_eq!(s.shop_id, "1");
    }

    #[test]
    fn multi_prompt() {
        let p = InjectedPrompter::new();
        p.push_select("2");
        let s = select_store_prompt(
            &p,
            &[
                store("1", "A", "a.myshopify.com"),
                store("2", "B", "b.myshopify.com"),
            ],
            true,
        )
        .unwrap()
        .unwrap();
        assert_eq!(s.shop_name, "B");
    }

    #[test]
    fn reload_store_list_confirm() {
        let p = InjectedPrompter::new();
        p.push_confirm(true);
        assert!(reload_store_list_prompt(&p, "Acme").unwrap());
    }

    #[test]
    fn multi_without_domain_uses_name_only() {
        let p = InjectedPrompter::new();
        p.push_select("1");
        let s = select_store_prompt(
            &p,
            &[
                store("1", "A", "a.myshopify.com"),
                store("2", "B", "b.myshopify.com"),
            ],
            false,
        )
        .unwrap()
        .unwrap();
        assert_eq!(s.shop_id, "1");
    }
}
