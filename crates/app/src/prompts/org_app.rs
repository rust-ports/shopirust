//! Organization / app selection prompts (upstream `prompts/dev.ts` app/org bits).

use super::{PromptItem, Prompter};
use crate::error::AppError;
use cli_api::{MinimalOrganizationApp, Organization};

const CREATE_NEW_VALUE: &str = "__create_new_app__";

pub fn select_organization(
    prompter: &dyn Prompter,
    orgs: &[Organization],
) -> Result<Organization, AppError> {
    if orgs.is_empty() {
        return Err(AppError::message("No organizations found for this account."));
    }
    if orgs.len() == 1 {
        return Ok(orgs[0].clone());
    }
    let items: Vec<_> = orgs
        .iter()
        .map(|o| PromptItem::new(o.business_name.clone(), o.id.clone()))
        .collect();
    let id = prompter.select("Which organization is this work for?", &items)?;
    orgs.iter()
        .find(|o| o.id == id)
        .cloned()
        .ok_or_else(|| AppError::message("Selected organization not found"))
}

/// Returns `Ok(None)` when the user chooses to create a new app.
pub fn select_app(
    prompter: &dyn Prompter,
    apps: &[MinimalOrganizationApp],
    allow_create: bool,
) -> Result<Option<MinimalOrganizationApp>, AppError> {
    if apps.is_empty() && !allow_create {
        return Err(AppError::message("No apps found in this organization."));
    }
    let mut items: Vec<_> = apps
        .iter()
        .map(|a| PromptItem::new(a.title.clone(), a.identifiers.api_key.clone()))
        .collect();
    if allow_create {
        items.push(PromptItem::new("Create a new app", CREATE_NEW_VALUE));
    }
    if items.is_empty() {
        return Ok(None);
    }
    let value = prompter.autocomplete("Which existing app is this for?", &items)?;
    if value == CREATE_NEW_VALUE {
        return Ok(None);
    }
    Ok(apps
        .iter()
        .find(|a| a.identifiers.api_key == value)
        .cloned())
}

pub fn prompt_app_name(prompter: &dyn Prompter, initial: Option<&str>) -> Result<String, AppError> {
    let name = prompter.text("App name", initial)?;
    if name.trim().is_empty() {
        return Err(AppError::message("App name cannot be empty"));
    }
    Ok(name.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prompts::InjectedPrompter;
    use cli_api::{MinimalAppIdentifiers, OrganizationSource};

    fn org(id: &str, name: &str) -> Organization {
        Organization {
            id: id.into(),
            business_name: name.into(),
            source: OrganizationSource::BusinessPlatform,
        }
    }

    fn app(key: &str, title: &str) -> MinimalOrganizationApp {
        MinimalOrganizationApp {
            identifiers: MinimalAppIdentifiers {
                api_key: key.into(),
                organization_id: "org".into(),
                id: key.into(),
            },
            title: title.into(),
        }
    }

    #[test]
    fn single_org_auto_selected() {
        let p = InjectedPrompter::new();
        let chosen = select_organization(&p, &[org("1", "Acme")]).unwrap();
        assert_eq!(chosen.id, "1");
    }

    #[test]
    fn select_org_prompt() {
        let p = InjectedPrompter::new();
        p.push_select("2");
        let chosen = select_organization(&p, &[org("1", "A"), org("2", "B")]).unwrap();
        assert_eq!(chosen.business_name, "B");
    }

    #[test]
    fn select_app_create_new() {
        let p = InjectedPrompter::new();
        p.push_select(CREATE_NEW_VALUE);
        let chosen = select_app(&p, &[app("k", "Demo")], true).unwrap();
        assert!(chosen.is_none());
    }

    #[test]
    fn select_app_existing() {
        let p = InjectedPrompter::new();
        p.push_select("k2");
        let chosen = select_app(&p, &[app("k1", "A"), app("k2", "B")], true)
            .unwrap()
            .unwrap();
        assert_eq!(chosen.title, "B");
    }

    #[test]
    fn empty_orgs_error() {
        let p = InjectedPrompter::new();
        assert!(select_organization(&p, &[]).is_err());
    }

    #[test]
    fn empty_apps_without_create_errors() {
        let p = InjectedPrompter::new();
        assert!(select_app(&p, &[], false).is_err());
    }

    #[test]
    fn empty_apps_with_create_offers_new() {
        let p = InjectedPrompter::new();
        p.push_select(CREATE_NEW_VALUE);
        assert!(select_app(&p, &[], true).unwrap().is_none());
    }

    #[test]
    fn prompt_app_name_trims() {
        let p = InjectedPrompter::new();
        p.push_text("  Demo  ");
        assert_eq!(prompt_app_name(&p, None).unwrap(), "Demo");
    }

    #[test]
    fn prompt_app_name_empty_errors() {
        let p = InjectedPrompter::new();
        p.push_text(" ");
        assert!(prompt_app_name(&p, None).is_err());
    }
}
