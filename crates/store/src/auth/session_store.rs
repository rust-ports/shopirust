use crate::auth::config::{
    is_escaped_store_key_segment, store_auth_session_key, STORE_AUTH_APP_CLIENT_ID,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum StoredStoreSessionKind {
    Standard,
    Preview,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct StoredPreviewStoreMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placeholder_account_uuid: Option<String>,
    pub shop_id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct StoredAssociatedUser {
    pub id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_owner: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StoredStoreAppSession {
    pub store: String,
    pub client_id: String,
    pub user_id: String,
    pub access_token: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    pub scopes: Vec<String>,
    pub acquired_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token_expires_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub associated_user: Option<StoredAssociatedUser>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<StoredStoreSessionKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview: Option<StoredPreviewStoreMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StoredStoreAppSessionBucket {
    pub current_user_id: String,
    pub sessions_by_user_id: HashMap<String, StoredStoreAppSession>,
}

pub trait StoreSessionStorage: Send + Sync {
    fn get(&self, key: &str) -> Option<Value>;
    fn set(&self, key: &str, value: Value);
    fn delete(&self, key: &str);
    fn entries(&self) -> Vec<(String, Value)>;
}

#[derive(Default)]
pub struct MemoryStoreSessionStorage {
    values: Mutex<HashMap<String, Value>>,
}

impl MemoryStoreSessionStorage {
    pub fn new() -> Self {
        Self::default()
    }
}

impl StoreSessionStorage for MemoryStoreSessionStorage {
    fn get(&self, key: &str) -> Option<Value> {
        self.values.lock().unwrap().get(key).cloned()
    }
    fn set(&self, key: &str, value: Value) {
        self.values.lock().unwrap().insert(key.to_string(), value);
    }
    fn delete(&self, key: &str) {
        self.values.lock().unwrap().remove(key);
    }
    fn entries(&self) -> Vec<(String, Value)> {
        self.values
            .lock()
            .unwrap()
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }
}

pub struct JsonFileStoreSessionStorage {
    path: PathBuf,
    lock: Mutex<()>,
}

impl JsonFileStoreSessionStorage {
    pub fn new() -> Self {
        let dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("shopify-cli-store");
        Self::with_path(dir.join("config.json"))
    }

    pub fn with_path(path: PathBuf) -> Self {
        Self {
            path,
            lock: Mutex::new(()),
        }
    }

    fn load(&self) -> Map<String, Value> {
        if !self.path.exists() {
            return Map::new();
        }
        let Ok(data) = std::fs::read_to_string(&self.path) else {
            return Map::new();
        };
        serde_json::from_str(&data).unwrap_or_default()
    }

    fn save(&self, map: &Map<String, Value>) {
        if let Some(parent) = self.path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(data) = serde_json::to_string_pretty(map) {
            let _ = std::fs::write(&self.path, data);
        }
    }
}

impl Default for JsonFileStoreSessionStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl StoreSessionStorage for JsonFileStoreSessionStorage {
    fn get(&self, key: &str) -> Option<Value> {
        let _g = self.lock.lock().unwrap();
        self.load().get(key).cloned()
    }
    fn set(&self, key: &str, value: Value) {
        let _g = self.lock.lock().unwrap();
        let mut map = self.load();
        map.insert(key.to_string(), value);
        self.save(&map);
    }
    fn delete(&self, key: &str) {
        let _g = self.lock.lock().unwrap();
        let mut map = self.load();
        map.remove(key);
        self.save(&map);
    }
    fn entries(&self) -> Vec<(String, Value)> {
        let _g = self.lock.lock().unwrap();
        self.load().into_iter().collect()
    }
}

fn is_string(value: &Value) -> Option<&str> {
    value.as_str()
}

fn sanitize_associated_user(value: &Value) -> Option<StoredAssociatedUser> {
    let obj = value.as_object()?;
    let id = obj.get("id")?.as_i64()?;
    Some(StoredAssociatedUser {
        id,
        email: obj.get("email").and_then(is_string).map(str::to_string),
        first_name: obj.get("firstName").and_then(is_string).map(str::to_string),
        last_name: obj.get("lastName").and_then(is_string).map(str::to_string),
        account_owner: obj.get("accountOwner").and_then(Value::as_bool),
    })
}

fn sanitize_preview_metadata(value: &Value) -> Option<StoredPreviewStoreMetadata> {
    let obj = value.as_object()?;
    let shop_id = obj.get("shopId").and_then(is_string)?.to_string();
    let name = obj.get("name").and_then(is_string)?.to_string();
    let created_at = obj.get("createdAt").and_then(is_string)?.to_string();
    Some(StoredPreviewStoreMetadata {
        shop_id,
        name,
        created_at,
        placeholder_account_uuid: obj
            .get("placeholderAccountUuid")
            .and_then(is_string)
            .map(str::to_string),
        country: obj.get("country").and_then(is_string).map(str::to_string),
        access_url: obj.get("accessUrl").and_then(is_string).map(str::to_string),
    })
}

fn sanitize_stored_store_app_session(value: &Value) -> Option<StoredStoreAppSession> {
    let obj = value.as_object()?;
    let store = obj.get("store").and_then(is_string)?.to_string();
    let client_id = obj.get("clientId").and_then(is_string)?.to_string();
    let user_id = obj.get("userId").and_then(is_string)?.to_string();
    let access_token = obj.get("accessToken").and_then(is_string)?.to_string();
    let scopes = obj.get("scopes")?.as_array()?;
    if !scopes.iter().all(|s| s.is_string()) {
        return None;
    }
    let scopes: Vec<String> = scopes
        .iter()
        .filter_map(|s| s.as_str().map(str::to_string))
        .collect();
    let acquired_at = obj.get("acquiredAt").and_then(is_string)?.to_string();
    let associated_user = obj.get("associatedUser").and_then(sanitize_associated_user);
    let kind = match obj.get("kind").and_then(is_string) {
        Some("preview") => StoredStoreSessionKind::Preview,
        _ => StoredStoreSessionKind::Standard,
    };
    let preview = if matches!(kind, StoredStoreSessionKind::Preview) {
        sanitize_preview_metadata(obj.get("preview")?)
    } else {
        None
    };
    if matches!(kind, StoredStoreSessionKind::Preview) && preview.is_none() {
        return None;
    }
    Some(StoredStoreAppSession {
        store,
        client_id,
        user_id,
        access_token,
        scopes,
        acquired_at,
        refresh_token: obj.get("refreshToken").and_then(is_string).map(str::to_string),
        expires_at: obj.get("expiresAt").and_then(is_string).map(str::to_string),
        refresh_token_expires_at: obj
            .get("refreshTokenExpiresAt")
            .and_then(is_string)
            .map(str::to_string),
        associated_user,
        kind: if matches!(kind, StoredStoreSessionKind::Preview) {
            Some(kind)
        } else {
            None
        },
        preview,
    })
}

fn sanitize_bucket(
    store: &str,
    stored_bucket: Option<&Value>,
    storage: &dyn StoreSessionStorage,
) -> Option<StoredStoreAppSessionBucket> {
    let value = stored_bucket?;
    let obj = value.as_object()?;
    let looks_like_bucket =
        obj.contains_key("sessionsByUserId") || obj.contains_key("currentUserId");
    if !looks_like_bucket {
        return None;
    }
    let key = store_auth_session_key(store);
    let sessions = obj.get("sessionsByUserId");
    let current_user_id = obj.get("currentUserId").and_then(is_string);
    if !sessions.map(|s| s.is_object()).unwrap_or(false) || current_user_id.is_none() {
        storage.delete(&key);
        return None;
    }
    let sessions = sessions?;
    let current_user_id = current_user_id?;
    let map = sessions.as_object()?;
    let mut sanitized = HashMap::new();
    for (user_id, session) in map {
        if let Some(s) = sanitize_stored_store_app_session(session) {
            sanitized.insert(user_id.clone(), s);
        }
    }
    if sanitized.len() != map.len() {
        if sanitized.contains_key(current_user_id) {
            let bucket = StoredStoreAppSessionBucket {
                current_user_id: current_user_id.to_string(),
                sessions_by_user_id: sanitized.clone(),
            };
            storage.set(&key, serde_json::to_value(&bucket).unwrap_or(Value::Null));
        } else {
            storage.delete(&key);
            return None;
        }
    }
    Some(StoredStoreAppSessionBucket {
        current_user_id: current_user_id.to_string(),
        sessions_by_user_id: sanitized,
    })
}

fn read_bucket(
    store: &str,
    storage: &dyn StoreSessionStorage,
) -> Option<StoredStoreAppSessionBucket> {
    let key = store_auth_session_key(store);
    let value = storage.get(&key);
    sanitize_bucket(store, value.as_ref(), storage)
}

pub fn list_current_stored_store_app_sessions(
    storage: &dyn StoreSessionStorage,
) -> Vec<StoredStoreAppSession> {
    let prefix = format!("{STORE_AUTH_APP_CLIENT_ID}::");
    let mut sessions = Vec::new();
    for (key, value) in storage.entries() {
        if !key.starts_with(&prefix) {
            continue;
        }
        let segment = &key[prefix.len()..];
        if !is_escaped_store_key_segment(segment) {
            continue;
        }
        let store = crate::auth::config::unescape_store_auth_session_key_segment(segment);
        if let Some(bucket) = sanitize_bucket(&store, Some(&value), storage) {
            if let Some(session) = bucket.sessions_by_user_id.get(&bucket.current_user_id) {
                sessions.push(session.clone());
            }
        }
    }
    sessions
}

pub fn get_current_stored_store_app_session(
    store: &str,
    storage: &dyn StoreSessionStorage,
) -> Option<StoredStoreAppSession> {
    let bucket = read_bucket(store, storage)?;
    match bucket.sessions_by_user_id.get(&bucket.current_user_id) {
        Some(session) => Some(session.clone()),
        None => {
            storage.delete(&store_auth_session_key(store));
            None
        }
    }
}

pub fn set_stored_store_app_session(
    session: StoredStoreAppSession,
    storage: &dyn StoreSessionStorage,
) {
    let key = store_auth_session_key(&session.store);
    let mut sessions = read_bucket(&session.store, storage)
        .map(|b| b.sessions_by_user_id)
        .unwrap_or_default();
    sessions.insert(session.user_id.clone(), session.clone());
    let bucket = StoredStoreAppSessionBucket {
        current_user_id: session.user_id,
        sessions_by_user_id: sessions,
    };
    storage.set(&key, serde_json::to_value(&bucket).unwrap_or(Value::Null));
}

pub fn clear_stored_store_app_session(
    store: &str,
    user_id: Option<&str>,
    storage: &dyn StoreSessionStorage,
) {
    let key = store_auth_session_key(store);
    let Some(user_id) = user_id else {
        storage.delete(&key);
        return;
    };
    let Some(existing) = read_bucket(store, storage) else {
        return;
    };
    let mut remaining = existing.sessions_by_user_id;
    remaining.remove(user_id);
    if remaining.is_empty() {
        storage.delete(&key);
        return;
    }
    let current = if existing.current_user_id == user_id {
        remaining.keys().next().cloned().unwrap_or_default()
    } else {
        existing.current_user_id
    };
    storage.set(
        &key,
        serde_json::to_value(StoredStoreAppSessionBucket {
            current_user_id: current,
            sessions_by_user_id: remaining,
        })
        .unwrap_or(Value::Null),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::config::STORE_AUTH_APP_CLIENT_ID;

    fn build_session(overrides: impl FnOnce(&mut StoredStoreAppSession)) -> StoredStoreAppSession {
        let mut session = StoredStoreAppSession {
            store: "shop.myshopify.com".into(),
            client_id: STORE_AUTH_APP_CLIENT_ID.into(),
            user_id: "42".into(),
            access_token: "token-1".into(),
            refresh_token: None,
            scopes: vec!["read_products".into()],
            acquired_at: "2026-03-27T00:00:00.000Z".into(),
            expires_at: None,
            refresh_token_expires_at: None,
            associated_user: None,
            kind: None,
            preview: None,
        };
        overrides(&mut session);
        session
    }

    #[test]
    fn returns_current_user_session() {
        let storage = MemoryStoreSessionStorage::new();
        let session = build_session(|_| {});
        set_stored_store_app_session(session.clone(), &storage);
        assert_eq!(
            get_current_stored_store_app_session("shop.myshopify.com", &storage),
            Some(session)
        );
    }

    #[test]
    fn keeps_multiple_users_returns_current() {
        let storage = MemoryStoreSessionStorage::new();
        let first = build_session(|s| {
            s.user_id = "42".into();
            s.access_token = "token-1".into();
        });
        let second = build_session(|s| {
            s.user_id = "84".into();
            s.access_token = "token-2".into();
        });
        set_stored_store_app_session(first, &storage);
        set_stored_store_app_session(second.clone(), &storage);
        assert_eq!(
            get_current_stored_store_app_session("shop.myshopify.com", &storage),
            Some(second)
        );
    }

    #[test]
    fn clears_all_sessions() {
        let storage = MemoryStoreSessionStorage::new();
        set_stored_store_app_session(build_session(|_| {}), &storage);
        clear_stored_store_app_session("shop.myshopify.com", None, &storage);
        assert!(get_current_stored_store_app_session("shop.myshopify.com", &storage).is_none());
    }

    #[test]
    fn clears_only_specified_user() {
        let storage = MemoryStoreSessionStorage::new();
        let first = build_session(|s| {
            s.user_id = "42".into();
            s.access_token = "token-1".into();
        });
        let second = build_session(|s| {
            s.user_id = "84".into();
            s.access_token = "token-2".into();
        });
        set_stored_store_app_session(first.clone(), &storage);
        set_stored_store_app_session(second, &storage);
        clear_stored_store_app_session("shop.myshopify.com", Some("84"), &storage);
        assert_eq!(
            get_current_stored_store_app_session("shop.myshopify.com", &storage),
            Some(first)
        );
    }

    #[test]
    fn missing_current_user_clears_bucket() {
        let storage = MemoryStoreSessionStorage::new();
        storage.set(
            &store_auth_session_key("shop.myshopify.com"),
            serde_json::json!({
                "currentUserId": "999",
                "sessionsByUserId": { "42": {
                    "store": "shop.myshopify.com",
                    "clientId": STORE_AUTH_APP_CLIENT_ID,
                    "userId": "42",
                    "accessToken": "token-1",
                    "scopes": ["read_products"],
                    "acquiredAt": "2026-03-27T00:00:00.000Z"
                }}
            }),
        );
        assert!(get_current_stored_store_app_session("shop.myshopify.com", &storage).is_none());
        assert!(storage
            .get(&store_auth_session_key("shop.myshopify.com"))
            .is_none());
    }

    #[test]
    fn corrupted_bucket_cleared() {
        let storage = MemoryStoreSessionStorage::new();
        storage.set(
            &store_auth_session_key("shop.myshopify.com"),
            serde_json::json!({
                "currentUserId": 42,
                "sessionsByUserId": null
            }),
        );
        assert!(get_current_stored_store_app_session("shop.myshopify.com", &storage).is_none());
        assert!(storage
            .get(&store_auth_session_key("shop.myshopify.com"))
            .is_none());
    }

    #[test]
    fn malformed_current_session_cleared() {
        let storage = MemoryStoreSessionStorage::new();
        storage.set(
            &store_auth_session_key("shop.myshopify.com"),
            serde_json::json!({
                "currentUserId": "42",
                "sessionsByUserId": { "42": { "userId": "42" } }
            }),
        );
        assert!(get_current_stored_store_app_session("shop.myshopify.com", &storage).is_none());
        assert!(storage
            .get(&store_auth_session_key("shop.myshopify.com"))
            .is_none());
    }

    #[test]
    fn drops_malformed_optional_fields() {
        let storage = MemoryStoreSessionStorage::new();
        let good = build_session(|_| {});
        storage.set(
            &store_auth_session_key("shop.myshopify.com"),
            serde_json::json!({
                "currentUserId": "42",
                "sessionsByUserId": {
                    "42": {
                        "store": "shop.myshopify.com",
                        "clientId": STORE_AUTH_APP_CLIENT_ID,
                        "userId": "42",
                        "accessToken": "token-1",
                        "scopes": ["read_products"],
                        "acquiredAt": "2026-03-27T00:00:00.000Z",
                        "refreshToken": 123,
                        "expiresAt": 456,
                        "refreshTokenExpiresAt": true,
                        "associatedUser": {
                            "id": 42,
                            "email": 123,
                            "firstName": "Merchant",
                            "lastName": false,
                            "accountOwner": "yes"
                        }
                    }
                }
            }),
        );
        let got = get_current_stored_store_app_session("shop.myshopify.com", &storage).unwrap();
        assert_eq!(got.store, good.store);
        assert_eq!(got.associated_user.unwrap().first_name.as_deref(), Some("Merchant"));
        assert!(got.refresh_token.is_none());
    }

    #[test]
    fn round_trips_preview_metadata() {
        let storage = MemoryStoreSessionStorage::new();
        let preview = build_session(|s| {
            s.user_id = "preview:placeholder-uuid".into();
            s.scopes = vec![];
            s.kind = Some(StoredStoreSessionKind::Preview);
            s.preview = Some(StoredPreviewStoreMetadata {
                placeholder_account_uuid: Some("placeholder-uuid".into()),
                shop_id: "123".into(),
                name: "Lavender Candles".into(),
                country: Some("US".into()),
                created_at: "2026-06-08T12:00:00.000Z".into(),
                access_url: None,
            });
        });
        set_stored_store_app_session(preview.clone(), &storage);
        assert_eq!(
            get_current_stored_store_app_session("shop.myshopify.com", &storage),
            Some(preview)
        );
    }

    #[test]
    fn rejects_malformed_preview_metadata() {
        let storage = MemoryStoreSessionStorage::new();
        storage.set(
            &store_auth_session_key("shop.myshopify.com"),
            serde_json::json!({
                "currentUserId": "preview:placeholder-uuid",
                "sessionsByUserId": {
                    "preview:placeholder-uuid": {
                        "store": "shop.myshopify.com",
                        "clientId": STORE_AUTH_APP_CLIENT_ID,
                        "userId": "preview:placeholder-uuid",
                        "accessToken": "token-1",
                        "scopes": ["read_products"],
                        "acquiredAt": "2026-03-27T00:00:00.000Z",
                        "kind": "preview",
                        "preview": { "placeholderAccountUuid": "placeholder-uuid" }
                    }
                }
            }),
        );
        assert!(get_current_stored_store_app_session("shop.myshopify.com", &storage).is_none());
    }

    #[test]
    fn overwrites_malformed_bucket_on_write() {
        let storage = MemoryStoreSessionStorage::new();
        storage.set(
            &store_auth_session_key("shop.myshopify.com"),
            serde_json::json!({
                "currentUserId": "42",
                "sessionsByUserId": null
            }),
        );
        let session = build_session(|_| {});
        set_stored_store_app_session(session.clone(), &storage);
        assert_eq!(
            get_current_stored_store_app_session("shop.myshopify.com", &storage),
            Some(session)
        );
    }

    #[test]
    fn clear_malformed_bucket_does_not_throw() {
        let storage = MemoryStoreSessionStorage::new();
        storage.set(
            &store_auth_session_key("shop.myshopify.com"),
            serde_json::json!({
                "currentUserId": "42",
                "sessionsByUserId": null
            }),
        );
        clear_stored_store_app_session("shop.myshopify.com", Some("42"), &storage);
        assert!(storage
            .get(&store_auth_session_key("shop.myshopify.com"))
            .is_none());
    }
}
