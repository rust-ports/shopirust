use crate::generate_name::generate_theme_name;
use crate::local_storage::ThemeLocalStorage;
use crate::models::Theme;
use crate::selector::SelectorError;
use crate::services::ThemeAdmin;
use crate::utilities::theme_store::ThemeStoreError;
use thiserror::Error;

pub const DEVELOPMENT_THEME_NOT_FOUND: &str =
    "Development theme #{theme_id} could not be found. Please create a new development theme.";
pub const NO_DEVELOPMENT_THEME_ID_SET: &str =
    "No development theme ID has been set. Please create a development theme first.";

#[derive(Debug, Error)]
pub enum ManagerError {
    #[error("{0}")]
    Store(#[from] ThemeStoreError),
    #[error("{0}")]
    Service(#[from] crate::services::ThemeServiceError),
    #[error("{0}")]
    Selector(#[from] SelectorError),
    #[error("{0}")]
    Io(String),
}

pub struct DevelopmentThemeManager<A> {
    pub admin: A,
    pub theme_id: Option<String>,
    storage: ThemeLocalStorage,
}

impl<A: ThemeAdmin + Sync> DevelopmentThemeManager<A> {
    pub fn new(admin: A) -> Self {
        Self {
            admin,
            theme_id: None,
            storage: ThemeLocalStorage::new(),
        }
    }

    pub fn with_storage(mut self, storage: ThemeLocalStorage) -> Self {
        self.storage = storage;
        self
    }

    pub fn with_theme_id(mut self, theme_id: impl Into<String>) -> Self {
        self.theme_id = Some(theme_id.into());
        self
    }

    fn require_store(&self) -> Result<String, ManagerError> {
        self.storage
            .current_theme_store()
            .map_err(|error| ManagerError::Io(error.to_string()))?
            .ok_or(ManagerError::Store(ThemeStoreError::Required))
    }

    pub async fn find(&self) -> Result<Theme, ManagerError> {
        let theme_id = self.resolve_theme_id()?;
        let store = self.require_store()?;
        let themes = self.admin.fetch_themes().await?;
        let theme = crate::selector::allowed_store_themes(&store, themes)?
            .into_iter()
            .find(|t| t.id.to_string() == theme_id)
            .ok_or_else(|| {
                ManagerError::Io(DEVELOPMENT_THEME_NOT_FOUND.replace("{theme_id}", &theme_id))
            })?;
        Ok(theme)
    }

    pub async fn fetch(&self) -> Result<Option<Theme>, ManagerError> {
        let Some(theme_id) = self.theme_id.clone() else {
            return Ok(None);
        };
        let store = self.require_store()?;
        let themes = self.admin.fetch_themes().await?;
        let found = crate::selector::allowed_store_themes(&store, themes)?
            .into_iter()
            .find(|t| t.id.to_string() == theme_id);
        Ok(found)
    }

    pub async fn find_or_create(
        &self,
        context: Option<&str>,
        role: &str,
    ) -> Result<Theme, ManagerError> {
        let store = self.require_store()?;
        let themes = self.admin.fetch_themes().await?;
        let allowed = crate::selector::allowed_store_themes(&store, themes)?;

        if let Some(theme_id) = &self.theme_id {
            if let Some(theme) = allowed.iter().find(|t| t.id.to_string() == *theme_id) {
                return Ok(theme.clone());
            }
        }

        let name = context
            .map(generate_theme_name)
            .unwrap_or_else(|| generate_theme_name("Development"));
        let theme = self.admin.create_theme(name, role.to_string()).await?;

        let _ = self
            .storage
            .store_development_theme_id_for_store(&store, theme.id);

        Ok(theme)
    }

    pub async fn set_theme_id(&self, theme_id: i64) -> Result<(), ManagerError> {
        let store = self.require_store()?;
        let _ = self
            .storage
            .store_development_theme_id_for_store(&store, theme_id);
        Ok(())
    }

    pub async fn remove_theme_id(&self) -> Result<(), ManagerError> {
        let store = self.require_store()?;
        let _ = self.storage.remove_development_theme_id_for_store(&store);
        Ok(())
    }

    fn resolve_theme_id(&self) -> Result<String, ManagerError> {
        let Some(store) = self.storage.current_theme_store().ok().flatten() else {
            return Err(ManagerError::Io(NO_DEVELOPMENT_THEME_ID_SET.into()));
        };
        self.theme_id
            .clone()
            .or_else(|| {
                self.storage
                    .development_theme_id_for_store(&store)
                    .ok()
                    .flatten()
                    .map(|id| id.to_string())
            })
            .ok_or_else(|| ManagerError::Io(NO_DEVELOPMENT_THEME_ID_SET.into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn storage_with_store() -> ThemeLocalStorage {
        let storage = ThemeLocalStorage::with_path(tempfile::tempdir().unwrap().path());
        storage
            .store_current_theme_store("test.myshopify.com")
            .unwrap();
        storage
    }

    struct MockAdmin {
        themes: Vec<Theme>,
        created: Vec<Theme>,
    }

    #[async_trait::async_trait]
    impl ThemeAdmin for MockAdmin {
        async fn fetch_themes(&self) -> Result<Vec<Theme>, crate::services::ThemeServiceError> {
            Ok(self.themes.clone())
        }
        async fn create_theme(
            &self,
            name: String,
            role: String,
        ) -> Result<Theme, crate::services::ThemeServiceError> {
            let theme = Theme {
                id: (self.created.len() + 1) as i64,
                name,
                role,
                created_at_runtime: true,
                processing: false,
                src: None,
            };
            Ok(theme)
        }
        async fn delete_theme(&self, _id: i64) -> Result<(), crate::services::ThemeServiceError> {
            Ok(())
        }
        async fn duplicate_theme(
            &self,
            _id: i64,
            _name: Option<String>,
        ) -> Result<crate::services::DuplicateResult, crate::services::ThemeServiceError> {
            unimplemented!()
        }
        async fn publish_theme(
            &self,
            _id: i64,
        ) -> Result<Option<Theme>, crate::services::ThemeServiceError> {
            Ok(None)
        }
        async fn update_theme_name(
            &self,
            _id: i64,
            _name: String,
        ) -> Result<Option<Theme>, crate::services::ThemeServiceError> {
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
    async fn find_returns_theme_when_id_matches() {
        let admin = MockAdmin {
            themes: vec![theme(1, "development"), theme(2, "live")],
            created: vec![],
        };
        let manager = DevelopmentThemeManager::new(admin)
            .with_storage(storage_with_store())
            .with_theme_id("1");
        let result = manager.find().await.unwrap();
        assert_eq!(result.id, 1);
    }

    #[tokio::test]
    async fn find_returns_error_when_no_theme_id_set() {
        let admin = MockAdmin {
            themes: vec![],
            created: vec![],
        };
        let manager = DevelopmentThemeManager::new(admin).with_storage(storage_with_store());
        let result = manager.find().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn find_returns_error_when_theme_not_found() {
        let admin = MockAdmin {
            themes: vec![theme(1, "live")],
            created: vec![],
        };
        let manager = DevelopmentThemeManager::new(admin)
            .with_storage(storage_with_store())
            .with_theme_id("2");
        let result = manager.find().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn fetch_returns_none_when_no_theme_id() {
        let admin = MockAdmin {
            themes: vec![],
            created: vec![],
        };
        let manager = DevelopmentThemeManager::new(admin).with_storage(storage_with_store());
        let result = manager.fetch().await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn find_or_create_creates_new_theme_when_no_match() {
        let admin = MockAdmin {
            themes: vec![theme(1, "live")],
            created: vec![],
        };
        let manager = DevelopmentThemeManager::new(admin).with_storage(storage_with_store());
        let result = manager
            .find_or_create(Some("PR-1"), "development")
            .await
            .unwrap();
        assert_eq!(result.role, "development");
        assert!(result.created_at_runtime);
    }

    #[tokio::test]
    async fn find_or_create_reuses_existing_theme() {
        let admin = MockAdmin {
            themes: vec![theme(1, "development")],
            created: vec![],
        };
        let storage = storage_with_store();
        let manager = DevelopmentThemeManager::new(admin)
            .with_storage(storage)
            .with_theme_id("1");
        let result = manager.find_or_create(None, "development").await.unwrap();
        assert_eq!(result.id, 1);
    }
}
