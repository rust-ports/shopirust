use crate::session::schema::Sessions;
use crate::util::cache::CacheStore;
use std::path::PathBuf;
use std::sync::Mutex;

const SESSION_STORE_KEY: &str = "sessionStore";
const CURRENT_SESSION_ID_KEY: &str = "currentSessionId";

pub struct SessionStore {
    cache: CacheStore,
    current_id: Mutex<Option<String>>,
}

impl SessionStore {
    fn default_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("shopify-cli-kit")
            .join("config.json")
    }

    pub fn new() -> Self {
        Self {
            cache: CacheStore::with_path(Self::default_path()),
            current_id: Mutex::new(None),
        }
    }

    pub fn with_path(path: PathBuf) -> Self {
        Self {
            cache: CacheStore::with_path(path),
            current_id: Mutex::new(None),
        }
    }

    pub fn store(&self, sessions: &Sessions) {
        let json = serde_json::to_string(sessions).expect("failed to serialize sessions");
        let _ = self.cache.store(SESSION_STORE_KEY, &json);
    }

    pub fn fetch(&self) -> Option<Sessions> {
        let val = self.cache.retrieve::<String>(SESSION_STORE_KEY).ok().flatten()?;
        serde_json::from_str(&val).ok()
    }

    pub fn remove(&self) {
        let _ = self.cache.remove(SESSION_STORE_KEY);
        let _ = self.cache.remove(CURRENT_SESSION_ID_KEY);
        *self.current_id.lock().unwrap() = None;
    }

    pub fn get_current_session_id(&self) -> Option<String> {
        let cached = self.current_id.lock().unwrap().clone();
        if cached.is_some() {
            return cached;
        }
        let val: Option<String> = self.cache.retrieve(CURRENT_SESSION_ID_KEY).ok().flatten();
        if let Some(ref id) = val {
            *self.current_id.lock().unwrap() = Some(id.clone());
        }
        val
    }

    pub fn set_current_session_id(&self, id: &str) {
        let _ = self.cache.store(CURRENT_SESSION_ID_KEY, &id.to_string());
        *self.current_id.lock().unwrap() = Some(id.to_string());
    }

    pub fn remove_current_session_id(&self) {
        let _ = self.cache.remove(CURRENT_SESSION_ID_KEY);
        *self.current_id.lock().unwrap() = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::schema::{IdentityToken, Session};
    use chrono::Utc;
    use std::collections::HashMap;

    fn test_session() -> Session {
        Session {
            identity: IdentityToken {
                access_token: "tok".into(),
                refresh_token: "ref".into(),
                expires_at: Utc::now(),
                scopes: vec![],
                user_id: "user-1".into(),
                alias: None,
            },
            applications: HashMap::new(),
        }
    }

    #[test]
    fn roundtrip_session() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::with_path(dir.path().join("config.json"));
        let mut sessions = Sessions::new();
        let mut inner = HashMap::new();
        inner.insert("user-1".into(), test_session());
        sessions.insert("accounts.shopify.com".into(), inner);

        store.store(&sessions);
        let fetched = store.fetch().unwrap();
        assert_eq!(
            fetched["accounts.shopify.com"]["user-1"]
                .identity
                .access_token,
            "tok"
        );
    }

    #[test]
    fn current_session_id() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::with_path(dir.path().join("config.json"));
        assert!(store.get_current_session_id().is_none());
        store.set_current_session_id("user-1");
        assert_eq!(store.get_current_session_id().unwrap(), "user-1");
        store.remove_current_session_id();
        assert!(store.get_current_session_id().is_none());
    }
}
