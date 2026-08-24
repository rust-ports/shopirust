use crate::session::schema::Sessions;
use crate::util::cache::CacheStore;
use crate::util::fqdn::identity_fqdn;
use std::path::PathBuf;

const SESSION_STORE_KEY: &str = "sessionStore";
const CURRENT_SESSION_ID_KEY: &str = "currentSessionId";
const DEV_SESSION_STORE_KEY: &str = "devSessionStore";
const CURRENT_DEV_SESSION_ID_KEY: &str = "currentDevSessionId";

pub struct SessionStore {
    cache: CacheStore,
}

impl Default for SessionStore {
    fn default() -> Self {
        Self::new()
    }
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
        }
    }

    pub fn with_path(path: PathBuf) -> Self {
        Self {
            cache: CacheStore::with_path(path),
        }
    }

    fn session_store_key() -> &'static str {
        if crate::constants::is_local_environment(None) {
            DEV_SESSION_STORE_KEY
        } else {
            SESSION_STORE_KEY
        }
    }

    fn current_session_id_key() -> &'static str {
        if crate::constants::is_local_environment(None) {
            CURRENT_DEV_SESSION_ID_KEY
        } else {
            CURRENT_SESSION_ID_KEY
        }
    }

    pub fn store(&self, sessions: &Sessions) {
        let json = serde_json::to_string(sessions).expect("failed to serialize sessions");
        let _ = self.cache.store(Self::session_store_key(), &json);
    }

    pub fn fetch(&self) -> Option<Sessions> {
        let val = self
            .cache
            .retrieve::<String>(Self::session_store_key())
            .ok()
            .flatten()?;
        match serde_json::from_str(&val) {
            Ok(sessions) => Some(sessions),
            Err(_) => {
                self.remove();
                None
            }
        }
    }

    pub fn remove(&self) {
        let _ = self.cache.remove(Self::session_store_key());
        let _ = self.cache.remove(Self::current_session_id_key());
    }

    pub fn get_current_session_id(&self) -> Option<String> {
        self.cache
            .retrieve(Self::current_session_id_key())
            .ok()
            .flatten()
    }

    pub fn set_current_session_id(&self, id: &str) {
        let _ = self
            .cache
            .store(Self::current_session_id_key(), &id.to_string());
    }

    pub fn remove_current_session_id(&self) {
        let _ = self.cache.remove(Self::current_session_id_key());
    }

    pub fn get_session_alias(&self, user_id: &str) -> Option<String> {
        let sessions = self.fetch()?;
        let fqdn = identity_fqdn(None);
        sessions.get(&fqdn)?.get(user_id)?.identity.alias.clone()
    }

    pub fn set_session_alias(&self, user_id: &str, alias: &str) {
        let Some(mut sessions) = self.fetch() else {
            return;
        };
        let fqdn = identity_fqdn(None);
        let Some(session) = sessions
            .get_mut(&fqdn)
            .and_then(|items| items.get_mut(user_id))
        else {
            return;
        };
        session.identity.alias = Some(alias.to_string());
        self.store(&sessions);
    }

    pub fn find_session_by_alias(&self, alias: &str) -> Option<String> {
        let sessions = self.fetch()?;
        let fqdn = identity_fqdn(None);
        let fqdn_sessions = sessions.get(&fqdn)?;

        for (user_id, session) in fqdn_sessions {
            if session.identity.alias.as_deref() == Some(alias) || user_id == alias {
                return Some(user_id.clone());
            }
        }

        None
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

    #[test]
    fn session_alias_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::with_path(dir.path().join("config.json"));
        let mut sessions = Sessions::new();
        let mut inner = HashMap::new();
        inner.insert("user-1".into(), test_session());
        sessions.insert("accounts.shopify.com".into(), inner);

        store.store(&sessions);
        store.set_session_alias("user-1", "me@example.com");

        assert_eq!(
            store.get_session_alias("user-1"),
            Some("me@example.com".to_string())
        );
        assert_eq!(
            store.find_session_by_alias("me@example.com"),
            Some("user-1".to_string())
        );
        assert_eq!(
            store.find_session_by_alias("user-1"),
            Some("user-1".to_string())
        );
    }
}
