use crate::local_storage::ThemeLocalStorage;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ThemeStoreError {
    #[error("A store is required")]
    Required,
}

pub fn ensure_theme_store(flags: &ThemeStoreFlags) -> Result<String, ThemeStoreError> {
    let store = flags.store.clone().or_else(|| get_theme_store());
    match store {
        Some(store) => {
            set_theme_store(&store);
            Ok(store)
        }
        None => Err(ThemeStoreError::Required),
    }
}

pub fn get_theme_store() -> Option<String> {
    let storage = ThemeLocalStorage::new();
    storage.current_theme_store().ok().flatten()
}

pub fn set_theme_store(store: &str) {
    let storage = ThemeLocalStorage::new();
    let _ = storage.store_current_theme_store(store);
}

pub fn remove_theme_store() {
    let _ = ThemeLocalStorage::new().remove_current_theme_store();
}

#[derive(Debug, Clone, Default)]
pub struct ThemeStoreFlags {
    pub store: Option<String>,
}

impl ThemeStoreFlags {
    pub fn new(store: Option<String>) -> Self {
        Self { store }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::local_storage::ThemeLocalStorage;

    #[test]
    fn returns_store_when_provided() {
        let flags = ThemeStoreFlags::new(Some("shop.myshopify.com".into()));
        let result = ensure_theme_store(&flags).unwrap();
        assert_eq!(result, "shop.myshopify.com");
    }

    #[test]
    fn returns_stored_value_when_flag_missing() {
        let storage = ThemeLocalStorage::with_path(tempfile::tempdir().unwrap().path());
        storage
            .store_current_theme_store("stored.myshopify.com")
            .unwrap();

        let result = storage.current_theme_store().unwrap();
        assert_eq!(result, Some("stored.myshopify.com".into()));
    }

    #[test]
    fn errors_when_no_store_available() {
        let storage = ThemeLocalStorage::with_path(tempfile::tempdir().unwrap().path());
        storage.remove_current_theme_store().unwrap();
        assert_eq!(storage.current_theme_store().unwrap(), None);
    }

    #[test]
    fn set_and_get_theme_store_round_trips() {
        let storage = ThemeLocalStorage::with_path(tempfile::tempdir().unwrap().path());
        storage
            .store_current_theme_store("roundtrip.myshopify.com")
            .unwrap();
        assert_eq!(
            storage.current_theme_store().unwrap(),
            Some("roundtrip.myshopify.com".into())
        );
    }

    #[test]
    fn remove_theme_store_clears_value() {
        let storage = ThemeLocalStorage::with_path(tempfile::tempdir().unwrap().path());
        storage
            .store_current_theme_store("remove.myshopify.com")
            .unwrap();
        storage.remove_current_theme_store().unwrap();
        assert_eq!(storage.current_theme_store().unwrap(), None);
    }

    #[test]
    fn set_and_get_global_theme_store_round_trips() {
        set_theme_store("global-roundtrip.myshopify.com");
        assert_eq!(
            get_theme_store(),
            Some("global-roundtrip.myshopify.com".into())
        );
    }
}
