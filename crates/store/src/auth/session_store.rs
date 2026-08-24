use crate::auth::config::{
    is_escaped_store_key_segment, store_auth_session_key, STORE_AUTH_APP_CLIENT_ID,
};
use fs2::FileExt;
use keyring::Entry;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::path::PathBuf;
use std::sync::Mutex;

const CREDENTIAL_SERVICE: &str = "shopify-cli-rust.store-session";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredTokenSecret {
    access_token: String,
    refresh_token: Option<String>,
}

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

    fn save(&self, map: &Map<String, Value>) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("create session directory: {error}"))?;
        }
        let data = serde_json::to_vec_pretty(map)
            .map_err(|error| format!("serialize session metadata: {error}"))?;
        let parent = self
            .path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."));
        let temp_path = parent.join(format!(
            ".{}.{}.tmp",
            self.path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("config.json"),
            std::process::id()
        ));
        let mut temp = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&temp_path)
            .map_err(|error| format!("create temporary session metadata: {error}"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            temp.set_permissions(std::fs::Permissions::from_mode(0o600))
                .map_err(|error| format!("protect session metadata: {error}"))?;
        }
        use std::io::Write;
        temp.write_all(&data)
            .map_err(|error| format!("write session metadata: {error}"))?;
        temp.sync_all()
            .map_err(|error| format!("sync session metadata: {error}"))?;
        std::fs::rename(&temp_path, &self.path)
            .map_err(|error| format!("replace session metadata: {error}"))?;
        Ok(())
    }

    fn lock_file(&self) -> Result<File, String> {
        let parent = self
            .path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."));
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("create session directory: {error}"))?;
        let lock_path = parent.join("config.json.lock");
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(lock_path)
            .map_err(|error| format!("open session lock: {error}"))?;
        file.lock_exclusive()
            .map_err(|error| format!("lock session metadata: {error}"))?;
        Ok(file)
    }

    fn credential_entry(secret_key: &str) -> Result<Entry, String> {
        Entry::new(CREDENTIAL_SERVICE, secret_key)
            .map_err(|error| format!("open operating-system credential store: {error}"))
    }

    fn write_secret(secret_key: &str, secret: &StoredTokenSecret) -> Result<(), String> {
        let encoded = serde_json::to_string(secret)
            .map_err(|error| format!("serialize token credential: {error}"))?;
        Self::credential_entry(secret_key)?
            .set_password(&encoded)
            .map_err(|error| format!("save token in operating-system credential store: {error}"))
    }

    fn read_secret(secret_key: &str) -> Result<Option<StoredTokenSecret>, String> {
        let entry = Self::credential_entry(secret_key)?;
        match entry.get_password() {
            Ok(encoded) => serde_json::from_str(&encoded)
                .map(Some)
                .map_err(|error| format!("decode token credential: {error}")),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(format!(
                "read token from operating-system credential store: {error}"
            )),
        }
    }

    fn delete_secret(secret_key: &str) {
        if let Ok(entry) = Self::credential_entry(secret_key) {
            let _ = entry.delete_credential();
        }
    }

    fn secret_key(session_key: &str, user_id: &str) -> String {
        format!("{session_key}::{user_id}")
    }

    fn strip_secrets(&self, session_key: &str, value: &Value) -> Result<Value, String> {
        let mut metadata = value.clone();
        let Some(sessions) = metadata
            .get_mut("sessionsByUserId")
            .and_then(Value::as_object_mut)
        else {
            return Ok(metadata);
        };
        for (user_id, session) in sessions {
            let Some(session) = session.as_object_mut() else {
                continue;
            };
            let access_token = session.get("accessToken").and_then(Value::as_str);
            let refresh_token = session
                .get("refreshToken")
                .and_then(Value::as_str)
                .map(str::to_owned);
            if let Some(access_token) = access_token {
                Self::write_secret(
                    &Self::secret_key(session_key, user_id),
                    &StoredTokenSecret {
                        access_token: access_token.to_owned(),
                        refresh_token,
                    },
                )?;
                session.remove("accessToken");
                session.remove("refreshToken");
            }
        }
        Ok(metadata)
    }

    fn hydrate_secrets(&self, session_key: &str, value: &Value) -> Value {
        let mut hydrated = value.clone();
        let Some(sessions) = hydrated
            .get_mut("sessionsByUserId")
            .and_then(Value::as_object_mut)
        else {
            return hydrated;
        };
        for (user_id, session) in sessions {
            let Some(session) = session.as_object_mut() else {
                continue;
            };
            if session.contains_key("accessToken") {
                continue;
            }
            match Self::read_secret(&Self::secret_key(session_key, user_id)) {
                Ok(Some(secret)) => {
                    session.insert("accessToken".into(), Value::String(secret.access_token));
                    if let Some(refresh_token) = secret.refresh_token {
                        session.insert("refreshToken".into(), Value::String(refresh_token));
                    }
                }
                Ok(None) => eprintln!(
                    "Store session credential is missing for `{user_id}`. Run `shopify store auth` again."
                ),
                Err(error) => eprintln!("Unable to load store session credential: {error}"),
            }
        }
        hydrated
    }

    fn has_plaintext_secrets(value: &Value) -> bool {
        value
            .get("sessionsByUserId")
            .and_then(Value::as_object)
            .is_some_and(|sessions| {
                sessions.values().any(|session| {
                    session
                        .as_object()
                        .is_some_and(|session| session.contains_key("accessToken"))
                })
            })
    }

    fn remove_value_secrets(&self, session_key: &str, value: Option<&Value>) {
        let Some(sessions) = value
            .and_then(|value| value.get("sessionsByUserId"))
            .and_then(Value::as_object)
        else {
            return;
        };
        for user_id in sessions.keys() {
            Self::delete_secret(&Self::secret_key(session_key, user_id));
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
        let _file_lock = match self.lock_file() {
            Ok(file) => file,
            Err(error) => {
                eprintln!("Unable to lock store session metadata: {error}");
                return None;
            }
        };
        let mut map = self.load();
        let value = map.get(key)?.clone();
        if Self::has_plaintext_secrets(&value) {
            match self.strip_secrets(key, &value) {
                Ok(metadata) => {
                    map.insert(key.to_string(), metadata.clone());
                    if let Err(error) = self.save(&map) {
                        eprintln!("Unable to migrate store session metadata: {error}");
                    }
                    return Some(self.hydrate_secrets(key, &metadata));
                }
                Err(error) => eprintln!("Unable to migrate store session credential: {error}"),
            }
        }
        Some(self.hydrate_secrets(key, &value))
    }
    fn set(&self, key: &str, value: Value) {
        let _g = self.lock.lock().unwrap();
        let Ok(_file_lock) = self.lock_file() else {
            eprintln!("Unable to lock store session metadata.");
            return;
        };
        let mut map = self.load();
        let metadata = match self.strip_secrets(key, &value) {
            Ok(metadata) => metadata,
            Err(error) => {
                eprintln!("Unable to save store session credential: {error}");
                return;
            }
        };
        map.insert(key.to_string(), metadata);
        if let Err(error) = self.save(&map) {
            eprintln!("Unable to save store session metadata: {error}");
        }
    }
    fn delete(&self, key: &str) {
        let _g = self.lock.lock().unwrap();
        let Ok(_file_lock) = self.lock_file() else {
            eprintln!("Unable to lock store session metadata.");
            return;
        };
        let mut map = self.load();
        self.remove_value_secrets(key, map.get(key));
        map.remove(key);
        if let Err(error) = self.save(&map) {
            eprintln!("Unable to save store session metadata: {error}");
        }
    }
    fn entries(&self) -> Vec<(String, Value)> {
        let _g = self.lock.lock().unwrap();
        let Ok(_file_lock) = self.lock_file() else {
            eprintln!("Unable to lock store session metadata.");
            return Vec::new();
        };
        let mut map = self.load();
        let keys: Vec<String> = map.keys().cloned().collect();
        for key in keys {
            if let Some(value) = map.get(&key).cloned() {
                if Self::has_plaintext_secrets(&value) {
                    match self.strip_secrets(&key, &value) {
                        Ok(metadata) => {
                            map.insert(key, metadata);
                        }
                        Err(error) => {
                            eprintln!("Unable to migrate store session credential: {error}")
                        }
                    }
                }
            }
        }
        if let Err(error) = self.save(&map) {
            eprintln!("Unable to migrate store session metadata: {error}");
        }
        map.into_iter()
            .map(|(key, value)| {
                let hydrated = self.hydrate_secrets(&key, &value);
                (key, hydrated)
            })
            .collect()
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
        refresh_token: obj
            .get("refreshToken")
            .and_then(is_string)
            .map(str::to_string),
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
        assert_eq!(
            got.associated_user.unwrap().first_name.as_deref(),
            Some("Merchant")
        );
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
