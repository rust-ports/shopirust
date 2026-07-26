use crate::output::components::prompts::select_input::Item;
use crate::output::public_api::{render_select_prompt, render_text_prompt};
use crate::session::public::session::ensure_authenticated_user_with_options;
use crate::session::store::SessionStore;
use crate::session::{AuthError, EnsureAuthenticatedOptions};
use crate::util::fqdn::identity_fqdn;

const NEW_LOGIN_VALUE: &str = "NEW_LOGIN";

#[derive(Debug, Clone, PartialEq, Eq)]
struct SessionChoice {
    label: String,
    value: String,
}

fn build_session_choices(
    sessions: &crate::session::schema::Sessions,
    fqdn: &str,
) -> Vec<SessionChoice> {
    let mut choices = Vec::new();
    if let Some(fqdn_sessions) = sessions.get(fqdn) {
        for (user_id, session) in fqdn_sessions {
            choices.push(SessionChoice {
                label: session
                    .identity
                    .alias
                    .clone()
                    .unwrap_or_else(|| user_id.clone()),
                value: user_id.clone(),
            });
        }
    }
    choices
}

async fn handle_new_login(store: &SessionStore) -> Result<String, AuthError> {
    let result = ensure_authenticated_user_with_options(EnsureAuthenticatedOptions {
        force_new_session: true,
        ..EnsureAuthenticatedOptions::default()
    })
    .await?;
    if let Some(alias) = store.get_session_alias(&result.user_id) {
        return Ok(alias);
    }

    let user_alias =
        render_text_prompt("Enter an alias for this account (e.g. your email or a nickname)")
            .map_err(|message| AuthError::Abort {
                message,
                next_steps: None,
            })?;
    store.set_session_alias(&result.user_id, &user_alias);
    Ok(user_alias)
}

fn get_all_choices(store: &SessionStore) -> Vec<SessionChoice> {
    let sessions = store.fetch();
    let fqdn = identity_fqdn(None);
    let mut choices = sessions
        .as_ref()
        .map(|sessions| build_session_choices(sessions, &fqdn))
        .unwrap_or_default();

    if !choices.is_empty() {
        choices.push(SessionChoice {
            label: "Log in with a different account".to_string(),
            value: NEW_LOGIN_VALUE.to_string(),
        });
    }

    choices
}

pub async fn prompt_session_select(alias: Option<&str>) -> Result<String, AuthError> {
    let store = SessionStore::new();
    if let Some(alias) = alias {
        if let Some(user_id) = store.find_session_by_alias(alias) {
            store.set_current_session_id(&user_id);
            return Ok(alias.to_string());
        }
    }

    let choices = get_all_choices(&store);
    let mut selected_value = NEW_LOGIN_VALUE.to_string();

    if !choices.is_empty() {
        let items = choices
            .iter()
            .map(|choice| Item::new(choice.label.clone(), choice.value.clone()))
            .collect();
        selected_value = render_select_prompt("Which account would you like to use?", items)
            .map_err(|message| AuthError::Abort {
                message,
                next_steps: None,
            })?;
    }

    if selected_value == NEW_LOGIN_VALUE {
        return handle_new_login(&store).await;
    }

    store.set_current_session_id(&selected_value);
    Ok(choices
        .iter()
        .find(|choice| choice.value == selected_value)
        .map(|choice| choice.label.clone())
        .unwrap_or(selected_value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::schema::{IdentityToken, Session};
    use chrono::Utc;
    use std::collections::HashMap;

    #[test]
    fn builds_choices_from_aliases() {
        let mut sessions = crate::session::schema::Sessions::new();
        let mut fqdn_sessions = HashMap::new();
        fqdn_sessions.insert(
            "user-1".to_string(),
            Session {
                identity: IdentityToken {
                    access_token: "access".to_string(),
                    refresh_token: "refresh".to_string(),
                    expires_at: Utc::now(),
                    scopes: vec![],
                    user_id: "user-1".to_string(),
                    alias: Some("me@example.com".to_string()),
                },
                applications: HashMap::new(),
            },
        );
        sessions.insert("accounts.shopify.com".to_string(), fqdn_sessions);

        let choices = build_session_choices(&sessions, "accounts.shopify.com");
        assert_eq!(
            choices,
            vec![SessionChoice {
                label: "me@example.com".to_string(),
                value: "user-1".to_string()
            }]
        );
    }
}
