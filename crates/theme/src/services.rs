use crate::models::{
    theme_editor_url, theme_preview_url, DuplicateJson, DuplicateJsonTheme, Theme, ThemeInfoJson,
    ThemeInfoJsonTheme, DEVELOPMENT_THEME_ROLE, LIVE_THEME_ROLE,
};
use crate::selector::{
    allowed_store_themes, filter_themes, find_theme, SelectorError, ThemeFilter,
};
use async_trait::async_trait;
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ThemeServiceError {
    #[error("{0}")]
    Selector(#[from] SelectorError),
    #[error("{0}")]
    Api(String),
    #[error("{0}")]
    User(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateResult {
    pub theme: Option<Theme>,
    pub user_errors: Vec<String>,
    pub request_id: Option<String>,
}

#[async_trait]
pub trait ThemeAdmin {
    async fn fetch_themes(&self) -> Result<Vec<Theme>, ThemeServiceError>;
    async fn delete_theme(&self, id: i64) -> Result<(), ThemeServiceError>;
    async fn duplicate_theme(
        &self,
        id: i64,
        name: Option<String>,
    ) -> Result<DuplicateResult, ThemeServiceError>;
    async fn publish_theme(&self, id: i64) -> Result<Option<Theme>, ThemeServiceError>;
    async fn update_theme_name(
        &self,
        id: i64,
        name: String,
    ) -> Result<Option<Theme>, ThemeServiceError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListOptions {
    pub role: Option<String>,
    pub name: Option<String>,
    pub id: Option<i64>,
}

pub async fn list_themes<A: ThemeAdmin + Sync>(
    api: &A,
    store: &str,
    options: &ListOptions,
) -> Result<Vec<Theme>, ThemeServiceError> {
    let mut themes = allowed_store_themes(store, api.fetch_themes().await?)?;
    let filter = ThemeFilter {
        live: options.role.as_deref() == Some(LIVE_THEME_ROLE),
        unpublished: options.role.as_deref() == Some("unpublished"),
        development: options.role.as_deref() == Some(DEVELOPMENT_THEME_ROLE),
        theme: options
            .id
            .map(|id| id.to_string())
            .or_else(|| options.name.clone()),
        ..Default::default()
    };
    if filter.any() {
        themes = filter_themes(store, &themes, &filter)?;
    }
    Ok(themes)
}

pub async fn select_theme<A: ThemeAdmin + Sync>(
    api: &A,
    store: &str,
    filter: &ThemeFilter,
) -> Result<Theme, ThemeServiceError> {
    let themes = allowed_store_themes(store, api.fetch_themes().await?)?;
    Ok(find_theme(store, &themes, filter)?)
}

pub async fn delete_themes<A: ThemeAdmin + Sync>(
    api: &A,
    store: &str,
    filter: &ThemeFilter,
) -> Result<Vec<Theme>, ThemeServiceError> {
    let themes = allowed_store_themes(store, api.fetch_themes().await?)?;
    let themes = if filter.any() {
        filter_themes(store, &themes, filter)?
    } else {
        return Err(SelectorError::PromptRequired.into());
    };

    for theme in &themes {
        api.delete_theme(theme.id).await?;
    }
    Ok(themes)
}

pub async fn duplicate_theme<A: ThemeAdmin + Sync>(
    api: &A,
    store: &str,
    theme_identifier: Option<String>,
    name: Option<String>,
) -> Result<(Theme, DuplicateResult), ThemeServiceError> {
    let identifier = theme_identifier.ok_or_else(|| {
        ThemeServiceError::User(
            "A theme ID is required to duplicate a theme, specify one with the --theme flag".into(),
        )
    })?;
    let original = select_theme(
        api,
        store,
        &ThemeFilter {
            theme: Some(identifier.clone()),
            ..Default::default()
        },
    )
    .await
    .map_err(|error| match error {
        ThemeServiceError::Selector(SelectorError::NoThemeMatch { .. }) => {
            ThemeServiceError::User(format!(
                "No theme with ID {identifier} could be found. Use shopify theme list to find a theme ID."
            ))
        }
        other => other,
    })?;

    if original.role == DEVELOPMENT_THEME_ROLE {
        return Err(ThemeServiceError::User(
            "Development themes can't be duplicated. Use shopify theme push to upload it to the store first.".into(),
        ));
    }

    let result = api.duplicate_theme(original.id, name).await?;
    Ok((original, result))
}

pub async fn publish_theme<A: ThemeAdmin + Sync>(
    api: &A,
    store: &str,
    theme: Option<String>,
) -> Result<Theme, ThemeServiceError> {
    let theme = select_theme(
        api,
        store,
        &ThemeFilter {
            theme,
            development: false,
            live: false,
            ..Default::default()
        },
    )
    .await?;
    api.publish_theme(theme.id).await?;
    Ok(theme)
}

pub async fn rename_theme<A: ThemeAdmin + Sync>(
    api: &A,
    store: &str,
    filter: &ThemeFilter,
    new_name: String,
) -> Result<Theme, ThemeServiceError> {
    let theme = select_theme(api, store, filter).await?;
    api.update_theme_name(theme.id, new_name).await?;
    Ok(theme)
}

pub fn theme_info_json(theme: &Theme, store: &str) -> ThemeInfoJson {
    ThemeInfoJson {
        theme: ThemeInfoJsonTheme {
            id: theme.id,
            name: theme.name.clone(),
            role: theme.role.clone(),
            shop: store.to_string(),
            preview_url: theme_preview_url(theme, store),
            editor_url: theme_editor_url(theme, store),
        },
    }
}

pub fn duplicate_json(theme: &Theme, store: &str) -> DuplicateJson {
    DuplicateJson {
        theme: DuplicateJsonTheme {
            id: theme.id,
            name: theme.name.clone(),
            role: theme.role.clone(),
            shop: store.to_string(),
        },
    }
}

pub fn to_pretty_json<T: Serialize>(value: &T) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| "{}".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Api {
        themes: Vec<Theme>,
    }

    #[async_trait]
    impl ThemeAdmin for Api {
        async fn fetch_themes(&self) -> Result<Vec<Theme>, ThemeServiceError> {
            Ok(self.themes.clone())
        }

        async fn delete_theme(&self, _id: i64) -> Result<(), ThemeServiceError> {
            Ok(())
        }

        async fn duplicate_theme(
            &self,
            id: i64,
            name: Option<String>,
        ) -> Result<DuplicateResult, ThemeServiceError> {
            Ok(DuplicateResult {
                theme: Some(Theme {
                    id: id + 10,
                    name: name.unwrap_or_else(|| "copy".into()),
                    role: "unpublished".into(),
                    created_at_runtime: false,
                    processing: false,
                    src: None,
                }),
                user_errors: vec![],
                request_id: None,
            })
        }

        async fn publish_theme(&self, _id: i64) -> Result<Option<Theme>, ThemeServiceError> {
            Ok(None)
        }

        async fn update_theme_name(
            &self,
            _id: i64,
            _name: String,
        ) -> Result<Option<Theme>, ThemeServiceError> {
            Ok(None)
        }
    }

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

    #[tokio::test]
    async fn lists_with_filters() {
        let api = Api {
            themes: vec![theme(1, "unpublished"), theme(2, "development")],
        };
        let themes = list_themes(
            &api,
            "shop.myshopify.com",
            &ListOptions {
                role: Some("development".into()),
                name: None,
                id: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(themes[0].id, 2);
    }
}
