use crate::models::{role_rank, Theme, ALLOWED_ROLES};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SelectorError {
    #[error("There are no themes in the {0} store")]
    NoThemes(String),
    #[error("No themes on the store {store} match the role \"{role}\"")]
    NoRoleMatch { store: String, role: String },
    #[error("No themes on the store {store} match the ID or name \"{identifier}\"")]
    NoThemeMatch { store: String, identifier: String },
    #[error("A theme ID or name is required because prompts are not available")]
    PromptRequired,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ThemeFilter {
    pub themes: Vec<String>,
    pub theme: Option<String>,
    pub development: bool,
    pub live: bool,
    pub unpublished: bool,
}

impl ThemeFilter {
    pub fn any(&self) -> bool {
        !self.themes.is_empty()
            || self.theme.as_ref().is_some_and(|theme| !theme.is_empty())
            || self.development
            || self.live
            || self.unpublished
    }

    pub fn role(&self) -> Option<&'static str> {
        if self.live {
            Some("live")
        } else if self.unpublished {
            Some("unpublished")
        } else if self.development {
            Some("development")
        } else {
            None
        }
    }

    pub fn identifiers(&self) -> Vec<&str> {
        self.theme
            .iter()
            .map(String::as_str)
            .chain(self.themes.iter().map(String::as_str))
            .filter(|identifier| !identifier.is_empty())
            .collect()
    }
}

pub fn allowed_store_themes(
    store: &str,
    mut themes: Vec<Theme>,
) -> Result<Vec<Theme>, SelectorError> {
    themes.retain(|theme| ALLOWED_ROLES.contains(&theme.role.as_str()));
    if themes.is_empty() {
        return Err(SelectorError::NoThemes(store.to_string()));
    }
    themes.sort_by_key(|theme| role_rank(&theme.role));
    Ok(themes)
}

pub fn filter_themes(
    store: &str,
    themes: &[Theme],
    filter: &ThemeFilter,
) -> Result<Vec<Theme>, SelectorError> {
    if let Some(role) = filter.role() {
        let filtered: Vec<Theme> = themes
            .iter()
            .filter(|theme| theme.role == role)
            .cloned()
            .collect();
        if filtered.is_empty() {
            return Err(SelectorError::NoRoleMatch {
                store: store.to_string(),
                role: role.to_string(),
            });
        }
        return Ok(filtered);
    }

    let mut filtered = Vec::new();
    for identifier in filter.identifiers() {
        let matches: Vec<Theme> = themes
            .iter()
            .filter(|theme| theme_matches(theme, identifier))
            .cloned()
            .collect();
        if matches.is_empty() {
            return Err(SelectorError::NoThemeMatch {
                store: store.to_string(),
                identifier: identifier.to_string(),
            });
        }
        filtered.extend(matches);
    }

    Ok(filtered)
}

pub fn find_theme(
    store: &str,
    themes: &[Theme],
    filter: &ThemeFilter,
) -> Result<Theme, SelectorError> {
    if filter.any() {
        return filter_themes(store, themes, filter).map(|themes| themes[0].clone());
    }

    Err(SelectorError::PromptRequired)
}

fn theme_matches(theme: &Theme, identifier: &str) -> bool {
    if theme.id.to_string() == identifier {
        return true;
    }

    let name = theme.name.to_lowercase();
    let identifier = identifier.to_lowercase();
    if name == identifier {
        return true;
    }

    partial_match(&name, &identifier)
}

fn partial_match(theme_name: &str, identifier: &str) -> bool {
    let trimmed = identifier.trim_start_matches('*').trim_end_matches('*');
    match (identifier.starts_with('*'), identifier.ends_with('*')) {
        (true, true) => theme_name.contains(trimmed),
        (true, false) => theme_name.ends_with(trimmed),
        (false, true) => theme_name.starts_with(trimmed),
        (false, false) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn theme(id: i64, role: &str) -> Theme {
        Theme {
            id,
            name: format!("theme ({id})"),
            role: role.into(),
            created_at_runtime: false,
            processing: false,
            src: None,
        }
    }

    #[test]
    fn filters_allowed_themes_and_sorts_by_role() {
        let themes = allowed_store_themes(
            "shop.myshopify.com",
            vec![
                theme(1, "unpublished"),
                theme(2, "demo"),
                theme(3, "live"),
                theme(4, "development"),
            ],
        )
        .unwrap();

        assert_eq!(
            themes.into_iter().map(|theme| theme.id).collect::<Vec<_>>(),
            vec![3, 1, 4]
        );
    }

    #[test]
    fn filters_by_role() {
        let filtered = filter_themes(
            "shop.myshopify.com",
            &[theme(1, "unpublished"), theme(2, "development")],
            &ThemeFilter {
                development: true,
                ..Default::default()
            },
        )
        .unwrap();

        assert_eq!(filtered[0].id, 2);
    }

    #[test]
    fn filters_by_id_exact_name_and_wildcards() {
        let themes = vec![theme(1, "live"), theme(7, "development")];
        assert_eq!(
            filter_themes(
                "shop.myshopify.com",
                &themes,
                &ThemeFilter {
                    theme: Some("7".into()),
                    ..Default::default()
                }
            )
            .unwrap()[0]
                .id,
            7
        );
        assert_eq!(
            filter_themes(
                "shop.myshopify.com",
                &themes,
                &ThemeFilter {
                    theme: Some("*eMe (7*".into()),
                    ..Default::default()
                }
            )
            .unwrap()[0]
                .id,
            7
        );
    }

    #[test]
    fn requires_prompt_when_no_filter_is_supplied() {
        let result = find_theme(
            "shop.myshopify.com",
            &[theme(1, "live")],
            &ThemeFilter::default(),
        );

        assert_eq!(result, Err(SelectorError::PromptRequired));
    }

    #[test]
    fn theme_filter_identifiers_combines_theme_and_themes() {
        let filter = ThemeFilter {
            theme: Some("1".into()),
            themes: vec!["2".into(), "3".into()],
            ..Default::default()
        };
        assert_eq!(filter.identifiers(), vec!["1", "2", "3"]);
    }

    #[test]
    fn theme_filter_identifiers_filters_empty_strings() {
        let filter = ThemeFilter {
            theme: Some("".into()),
            themes: vec!["2".into()],
            ..Default::default()
        };
        assert_eq!(filter.identifiers(), vec!["2"]);
    }

    #[test]
    fn theme_filter_role_returns_correct_role() {
        assert_eq!(
            ThemeFilter {
                live: true,
                ..Default::default()
            }
            .role(),
            Some("live")
        );
        assert_eq!(
            ThemeFilter {
                unpublished: true,
                ..Default::default()
            }
            .role(),
            Some("unpublished")
        );
        assert_eq!(
            ThemeFilter {
                development: true,
                ..Default::default()
            }
            .role(),
            Some("development")
        );
        assert_eq!(ThemeFilter::default().role(), None);
    }

    #[test]
    fn theme_filter_any_returns_true_for_any_active_filter() {
        assert!(ThemeFilter {
            theme: Some("1".into()),
            ..Default::default()
        }
        .any());
        assert!(ThemeFilter {
            themes: vec!["1".into()],
            ..Default::default()
        }
        .any());
        assert!(ThemeFilter {
            development: true,
            ..Default::default()
        }
        .any());
        assert!(ThemeFilter {
            live: true,
            ..Default::default()
        }
        .any());
        assert!(ThemeFilter {
            unpublished: true,
            ..Default::default()
        }
        .any());
    }

    #[test]
    fn theme_filter_any_returns_false_when_empty() {
        assert!(!ThemeFilter::default().any());
        assert!(!ThemeFilter {
            themes: vec![],
            ..Default::default()
        }
        .any());
    }

    #[test]
    fn filters_by_multiple_identifiers() {
        let themes = vec![theme(1, "live"), theme(2, "live"), theme(3, "live")];
        let filtered = filter_themes(
            "shop.myshopify.com",
            &themes,
            &ThemeFilter {
                themes: vec!["theme (1)".into(), "theme (3)".into()],
                ..Default::default()
            },
        )
        .unwrap();

        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].id, 1);
        assert_eq!(filtered[1].id, 3);
    }

    #[test]
    fn filter_themes_returns_error_when_role_has_no_match() {
        let result = filter_themes(
            "shop.myshopify.com",
            &[theme(1, "live")],
            &ThemeFilter {
                development: true,
                ..Default::default()
            },
        );

        assert!(matches!(result, Err(SelectorError::NoRoleMatch { .. })));
    }

    #[test]
    fn filter_themes_returns_error_when_identifier_has_no_match() {
        let result = filter_themes(
            "shop.myshopify.com",
            &[theme(1, "live")],
            &ThemeFilter {
                theme: Some("nonexistent".into()),
                ..Default::default()
            },
        );

        assert!(matches!(result, Err(SelectorError::NoThemeMatch { .. })));
    }

    #[test]
    fn partial_match_middle_wildcard() {
        assert!(partial_match("theme (7)", "*eme (7*"));
        assert!(!partial_match("theme (1)", "*eme (7*"));
    }

    #[test]
    fn partial_match_prefix_wildcard() {
        assert!(partial_match("theme (7)", "theme (7*"));
        assert!(!partial_match("other (7)", "theme (7*"));
    }

    #[test]
    fn partial_match_suffix_wildcard() {
        assert!(partial_match("theme (7)", "*me (7)"));
        assert!(!partial_match("theme (1)", "*me (7)"));
    }

    #[test]
    fn partial_match_no_wildcard_returns_false() {
        assert!(!partial_match("theme (7)", "theme(7)"));
    }

    #[test]
    fn allowed_store_themes_errors_on_empty() {
        let result = allowed_store_themes("shop.myshopify.com", vec![]);
        assert!(matches!(result, Err(SelectorError::NoThemes(_))));
    }

    #[test]
    fn allowed_store_themes_filters_disallowed_roles() {
        let themes = allowed_store_themes(
            "shop.myshopify.com",
            vec![theme(1, "demo"), theme(2, "live")],
        )
        .unwrap();

        assert_eq!(themes.len(), 1);
        assert_eq!(themes[0].id, 2);
    }
}
