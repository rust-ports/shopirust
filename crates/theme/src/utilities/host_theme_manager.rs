//! Find-or-create host theme for theme app extensions (upstream `HostThemeManager`).

use crate::generate_name::generate_theme_name;
use crate::local_storage::ThemeLocalStorage;
use crate::models::{Theme, DEVELOPMENT_THEME_ROLE};
use crate::services::{ThemeAdmin, ThemeServiceError};
use std::time::{Duration, Instant};
use thiserror::Error;

pub const DEFAULT_THEME_ZIP: &str =
    "https://codeload.github.com/Shopify/dawn/zip/refs/tags/v15.0.0";
pub const FALLBACK_THEME_ZIP: &str =
    "https://cdn.shopify.com/theme-store/uhrdefhlndzaoyrgylhto59sx2i7.jpg";
pub const FAILED_TO_CREATE_THEME_MESSAGE: &str = "The host theme could not be created to host your theme app extension. Please try again or use the \"--theme\" flag to use an existing theme as the host theme.";

const RETRY_ATTEMPTS: usize = 3;
const DEFAULT_POLL_INTERVAL: Duration = Duration::from_secs(3);
const DEFAULT_WAIT_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const HOST_THEME_CONTEXT: &str = "App Ext. Host";

#[derive(Debug, Error)]
pub enum HostThemeError {
    #[error("{0}")]
    Api(#[from] ThemeServiceError),
    #[error("{0}")]
    Message(String),
}

pub struct HostThemeManager<A> {
    admin: A,
    store_fqdn: String,
    storage: ThemeLocalStorage,
    theme_id: Option<String>,
    poll_interval: Duration,
    wait_timeout: Duration,
}

impl<A: ThemeAdmin + Sync> HostThemeManager<A> {
    pub fn new(admin: A, store_fqdn: impl Into<String>) -> Self {
        Self {
            admin,
            store_fqdn: store_fqdn.into(),
            storage: ThemeLocalStorage::new(),
            theme_id: None,
            poll_interval: DEFAULT_POLL_INTERVAL,
            wait_timeout: DEFAULT_WAIT_TIMEOUT,
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

    pub fn with_timings(mut self, poll_interval: Duration, wait_timeout: Duration) -> Self {
        self.poll_interval = poll_interval;
        self.wait_timeout = wait_timeout;
        self
    }

    pub async fn fetch(&self) -> Result<Option<Theme>, HostThemeError> {
        let Some(theme_id) = self.resolve_theme_id() else {
            return Ok(None);
        };
        let Ok(id) = theme_id.parse::<i64>() else {
            return Ok(None);
        };
        Ok(self.admin.fetch_theme(id).await?)
    }

    pub async fn find_or_create(&self) -> Result<Theme, HostThemeError> {
        if let Some(theme) = self.fetch().await? {
            return Ok(theme);
        }
        self.create_host_theme().await
    }

    async fn create_host_theme(&self) -> Result<Theme, HostThemeError> {
        let name = generate_theme_name(HOST_THEME_CONTEXT);
        let role = DEVELOPMENT_THEME_ROLE.to_string();
        for _ in 0..RETRY_ATTEMPTS {
            match self
                .admin
                .create_theme_with_src(name.clone(), role.clone(), Some(DEFAULT_THEME_ZIP.into()))
                .await
            {
                Ok(theme) => {
                    self.persist(theme.id);
                    self.wait_for_theme_to_be_processed(theme.id).await?;
                    return self.theme_after_create(theme).await;
                }
                Err(_) => continue,
            }
        }
        let theme = self
            .admin
            .create_theme_with_src(name, role, Some(FALLBACK_THEME_ZIP.into()))
            .await
            .map_err(|_| HostThemeError::Message(FAILED_TO_CREATE_THEME_MESSAGE.into()))?;
        self.persist(theme.id);
        self.wait_for_theme_to_be_processed(theme.id).await?;
        self.theme_after_create(theme).await
    }

    async fn theme_after_create(&self, created: Theme) -> Result<Theme, HostThemeError> {
        Ok(self.admin.fetch_theme(created.id).await?.unwrap_or(created))
    }

    async fn wait_for_theme_to_be_processed(&self, theme_id: i64) -> Result<(), HostThemeError> {
        let started = Instant::now();
        loop {
            let Some(theme) = self.admin.fetch_theme(theme_id).await? else {
                return Err(HostThemeError::Message(
                    FAILED_TO_CREATE_THEME_MESSAGE.into(),
                ));
            };
            if !theme.processing {
                return Ok(());
            }
            if started.elapsed() >= self.wait_timeout {
                return Err(HostThemeError::Message(
                    FAILED_TO_CREATE_THEME_MESSAGE.into(),
                ));
            }
            tokio::time::sleep(self.poll_interval).await;
        }
    }

    fn persist(&self, theme_id: i64) {
        let _ = self.storage.store_host_theme_id(&self.store_fqdn, theme_id);
    }

    fn resolve_theme_id(&self) -> Option<String> {
        self.theme_id.clone().or_else(|| {
            self.storage
                .host_theme_id(&self.store_fqdn)
                .ok()
                .flatten()
                .map(|id| id.to_string())
        })
    }
}

/// Resolve `--theme` or find/create a Dawn-based host theme and persist its id.
pub async fn find_or_create_host_theme<A: ThemeAdmin + Sync>(
    admin: A,
    store_fqdn: &str,
    theme_flag: Option<&str>,
    storage: ThemeLocalStorage,
) -> Result<Theme, HostThemeError> {
    if let Some(flag) = theme_flag {
        if let Ok(id) = flag.parse::<i64>() {
            let theme = admin.fetch_theme(id).await?.ok_or_else(|| {
                HostThemeError::Message(
                    "Could not find or create a host theme for theme app extensions".into(),
                )
            })?;
            let _ = storage.store_host_theme_id(store_fqdn, theme.id);
            return Ok(theme);
        }
    }
    let mut manager = HostThemeManager::new(admin, store_fqdn).with_storage(storage);
    if let Some(flag) = theme_flag {
        manager = manager.with_theme_id(flag);
    }
    manager.find_or_create().await
}

/// Admin GraphQL client used by `app dev` (no cli-kit dependency).
pub struct TokenThemeAdmin {
    pub store_fqdn: String,
    pub token: String,
    pub graphql_url: String,
}

impl TokenThemeAdmin {
    pub fn new(store_fqdn: &str, token: &str) -> Self {
        let origin = storefront_origin(store_fqdn);
        Self {
            store_fqdn: store_fqdn.to_string(),
            token: token.to_string(),
            graphql_url: format!("{origin}/admin/api/unstable/graphql.json"),
        }
    }

    pub fn with_graphql_url(mut self, url: impl Into<String>) -> Self {
        self.graphql_url = url.into();
        self
    }

    async fn graphql(
        &self,
        query: &str,
        variables: serde_json::Value,
    ) -> Result<serde_json::Value, ThemeServiceError> {
        let client = reqwest::Client::new();
        let response = client
            .post(&self.graphql_url)
            .header("X-Shopify-Access-Token", &self.token)
            .header("Content-Type", "application/json")
            .json(&serde_json::json!({
                "query": query,
                "variables": variables,
            }))
            .send()
            .await
            .map_err(|error| ThemeServiceError::Api(error.to_string()))?;
        if !response.status().is_success() {
            return Err(ThemeServiceError::Api(format!(
                "Admin GraphQL failed with status {}",
                response.status()
            )));
        }
        response
            .json()
            .await
            .map_err(|error| ThemeServiceError::Api(error.to_string()))
    }
}

pub fn storefront_origin(store_fqdn: &str) -> String {
    let trimmed = store_fqdn.trim().trim_end_matches('/');
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else {
        format!("https://{trimmed}")
    }
}

fn parse_theme_gid(gid: &str) -> Option<i64> {
    gid.rsplit('/').next()?.parse().ok()
}

fn domain_role(role: &str) -> String {
    match role {
        "MAIN" => "live".into(),
        "UNPUBLISHED" => "unpublished".into(),
        "DEVELOPMENT" => "development".into(),
        other => other.to_lowercase(),
    }
}

fn graphql_role(role: &str) -> &'static str {
    match role.to_ascii_lowercase().as_str() {
        "live" | "main" => "MAIN",
        "unpublished" => "UNPUBLISHED",
        _ => "DEVELOPMENT",
    }
}

const GET_THEMES_QUERY: &str = r"query getThemes($after: String) {
  themes(first: 50, after: $after) {
    nodes { id name role processing }
    pageInfo { hasNextPage endCursor }
  }
}";

const GET_THEME_QUERY: &str = r"query getTheme($id: ID!) {
  theme(id: $id) { id name role processing }
}";

const THEME_CREATE_MUTATION: &str = r"mutation themeCreate($name: String!, $source: URL!, $role: ThemeRole!) {
  themeCreate(name: $name, source: $source, role: $role) {
    theme { id name role }
    userErrors { field message }
  }
}";

const SKELETON_THEME_CDN: &str = "https://cdn.shopify.com/static/online-store/theme-skeleton.zip";

#[async_trait::async_trait]
impl ThemeAdmin for TokenThemeAdmin {
    async fn fetch_themes(&self) -> Result<Vec<Theme>, ThemeServiceError> {
        let mut themes = Vec::new();
        let mut after: Option<String> = None;
        loop {
            let mut variables = serde_json::Map::new();
            if let Some(cursor) = &after {
                variables.insert("after".into(), serde_json::Value::String(cursor.clone()));
            }
            let json = self
                .graphql(GET_THEMES_QUERY, serde_json::Value::Object(variables))
                .await?;
            let nodes = json
                .pointer("/data/themes/nodes")
                .and_then(|value| value.as_array())
                .cloned()
                .unwrap_or_default();
            for node in nodes {
                let Some(theme) = theme_from_json(&node) else {
                    continue;
                };
                themes.push(theme);
            }
            let has_next = json
                .pointer("/data/themes/pageInfo/hasNextPage")
                .and_then(|value| value.as_bool())
                .unwrap_or(false);
            after = json
                .pointer("/data/themes/pageInfo/endCursor")
                .and_then(|value| value.as_str())
                .map(ToOwned::to_owned);
            if !has_next {
                break;
            }
        }
        Ok(themes)
    }

    async fn fetch_theme(&self, id: i64) -> Result<Option<Theme>, ThemeServiceError> {
        let json = self
            .graphql(
                GET_THEME_QUERY,
                serde_json::json!({ "id": format!("gid://shopify/OnlineStoreTheme/{id}") }),
            )
            .await?;
        Ok(json.pointer("/data/theme").and_then(theme_from_json))
    }

    async fn create_theme(&self, name: String, role: String) -> Result<Theme, ThemeServiceError> {
        self.create_theme_with_src(name, role, None).await
    }

    async fn create_theme_with_src(
        &self,
        name: String,
        role: String,
        src: Option<String>,
    ) -> Result<Theme, ThemeServiceError> {
        let source = src.unwrap_or_else(|| SKELETON_THEME_CDN.to_string());
        let json = self
            .graphql(
                THEME_CREATE_MUTATION,
                serde_json::json!({
                    "name": name,
                    "source": source,
                    "role": graphql_role(&role),
                }),
            )
            .await?;
        if let Some(errors) = json
            .pointer("/data/themeCreate/userErrors")
            .and_then(|v| v.as_array())
        {
            let messages: Vec<String> = errors
                .iter()
                .filter_map(|error| error.get("message")?.as_str().map(ToOwned::to_owned))
                .collect();
            if !messages.is_empty() {
                return Err(ThemeServiceError::Api(messages.join(", ")));
            }
        }
        json.pointer("/data/themeCreate/theme")
            .and_then(theme_from_json)
            .ok_or_else(|| ThemeServiceError::Api("Failed to create theme".into()))
    }

    async fn delete_theme(&self, _id: i64) -> Result<(), ThemeServiceError> {
        Err(ThemeServiceError::Api(
            "delete_theme is not used by host theme manager".into(),
        ))
    }

    async fn duplicate_theme(
        &self,
        _id: i64,
        _name: Option<String>,
    ) -> Result<crate::services::DuplicateResult, ThemeServiceError> {
        Err(ThemeServiceError::Api(
            "duplicate_theme is not used by host theme manager".into(),
        ))
    }

    async fn publish_theme(&self, _id: i64) -> Result<Option<Theme>, ThemeServiceError> {
        Err(ThemeServiceError::Api(
            "publish_theme is not used by host theme manager".into(),
        ))
    }

    async fn update_theme_name(
        &self,
        _id: i64,
        _name: String,
    ) -> Result<Option<Theme>, ThemeServiceError> {
        Err(ThemeServiceError::Api(
            "update_theme_name is not used by host theme manager".into(),
        ))
    }
}

fn theme_from_json(value: &serde_json::Value) -> Option<Theme> {
    let id = value.get("id")?.as_str().and_then(parse_theme_gid)?;
    let name = value.get("name")?.as_str()?.to_string();
    let role = value
        .get("role")
        .and_then(|value| value.as_str())
        .map(domain_role)
        .unwrap_or_else(|| DEVELOPMENT_THEME_ROLE.to_string());
    let processing = value
        .get("processing")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    Some(Theme {
        id,
        name,
        role,
        created_at_runtime: false,
        processing,
        src: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::DuplicateResult;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;

    struct MockAdmin {
        themes: Mutex<Vec<Theme>>,
        create_fail_times: AtomicUsize,
        created_src: std::sync::Arc<Mutex<Vec<Option<String>>>>,
        polls_until_ready: AtomicUsize,
    }

    fn theme(id: i64, processing: bool) -> Theme {
        Theme {
            id,
            name: format!("theme-{id}"),
            role: DEVELOPMENT_THEME_ROLE.into(),
            created_at_runtime: true,
            processing,
            src: None,
        }
    }

    #[async_trait::async_trait]
    impl ThemeAdmin for MockAdmin {
        async fn fetch_themes(&self) -> Result<Vec<Theme>, ThemeServiceError> {
            if self.polls_until_ready.load(Ordering::SeqCst) > 0 {
                self.polls_until_ready.fetch_sub(1, Ordering::SeqCst);
            } else {
                let mut themes = self.themes.lock().expect("themes poisoned");
                for item in themes.iter_mut() {
                    item.processing = false;
                }
            }
            Ok(self.themes.lock().expect("themes poisoned").clone())
        }

        async fn create_theme(
            &self,
            name: String,
            role: String,
        ) -> Result<Theme, ThemeServiceError> {
            self.create_theme_with_src(name, role, None).await
        }

        async fn create_theme_with_src(
            &self,
            name: String,
            role: String,
            src: Option<String>,
        ) -> Result<Theme, ThemeServiceError> {
            self.created_src
                .lock()
                .expect("src poisoned")
                .push(src.clone());
            if self.create_fail_times.load(Ordering::SeqCst) > 0 {
                self.create_fail_times.fetch_sub(1, Ordering::SeqCst);
                return Err(ThemeServiceError::Api("create failed".into()));
            }
            let created = Theme {
                id: 42,
                name,
                role,
                created_at_runtime: true,
                processing: true,
                src,
            };
            self.themes
                .lock()
                .expect("themes poisoned")
                .push(created.clone());
            Ok(created)
        }

        async fn delete_theme(&self, _id: i64) -> Result<(), ThemeServiceError> {
            Ok(())
        }
        async fn duplicate_theme(
            &self,
            _id: i64,
            _name: Option<String>,
        ) -> Result<DuplicateResult, ThemeServiceError> {
            unimplemented!()
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

    fn storage() -> ThemeLocalStorage {
        ThemeLocalStorage::with_path(tempfile::tempdir().unwrap().path())
    }

    #[tokio::test]
    async fn reuses_persisted_host_theme() {
        let store = "shop.myshopify.com";
        let storage = storage();
        storage.store_host_theme_id(store, 7).unwrap();
        let admin = MockAdmin {
            themes: Mutex::new(vec![theme(7, false)]),
            create_fail_times: AtomicUsize::new(0),
            created_src: std::sync::Arc::new(Mutex::new(vec![])),
            polls_until_ready: AtomicUsize::new(0),
        };
        let manager = HostThemeManager::new(admin, store)
            .with_storage(storage)
            .with_timings(Duration::from_millis(1), Duration::from_millis(50));
        let result = manager.find_or_create().await.unwrap();
        assert_eq!(result.id, 7);
    }

    #[tokio::test]
    async fn creates_dawn_zip_and_waits_until_processed() {
        let store = "shop.myshopify.com";
        let storage = storage();
        let admin = MockAdmin {
            themes: Mutex::new(vec![]),
            create_fail_times: AtomicUsize::new(0),
            created_src: std::sync::Arc::new(Mutex::new(vec![])),
            polls_until_ready: AtomicUsize::new(1),
        };
        let manager = HostThemeManager::new(admin, store)
            .with_storage(storage.clone())
            .with_timings(Duration::from_millis(1), Duration::from_millis(200));
        let result = manager.find_or_create().await.unwrap();
        assert_eq!(result.id, 42);
        assert!(!result.processing);
        assert_eq!(storage.host_theme_id(store).unwrap(), Some(42));
    }

    #[tokio::test]
    async fn falls_back_to_catalog_zip_after_retries() {
        let store = "shop.myshopify.com";
        let created_src = std::sync::Arc::new(Mutex::new(vec![]));
        let admin = MockAdmin {
            themes: Mutex::new(vec![]),
            create_fail_times: AtomicUsize::new(RETRY_ATTEMPTS),
            created_src: created_src.clone(),
            polls_until_ready: AtomicUsize::new(0),
        };
        let manager = HostThemeManager::new(admin, store)
            .with_storage(storage())
            .with_timings(Duration::from_millis(1), Duration::from_millis(50));
        let result = manager.find_or_create().await.unwrap();
        assert_eq!(result.id, 42);
        let srcs = created_src.lock().unwrap().clone();
        assert_eq!(srcs.len(), RETRY_ATTEMPTS + 1);
        assert_eq!(srcs.last().unwrap().as_deref(), Some(FALLBACK_THEME_ZIP));
    }

    #[tokio::test]
    async fn flag_id_fetches_existing_theme() {
        let admin = MockAdmin {
            themes: Mutex::new(vec![theme(99, false)]),
            create_fail_times: AtomicUsize::new(0),
            created_src: std::sync::Arc::new(Mutex::new(vec![])),
            polls_until_ready: AtomicUsize::new(0),
        };
        let theme = find_or_create_host_theme(admin, "shop.myshopify.com", Some("99"), storage())
            .await
            .unwrap();
        assert_eq!(theme.id, 99);
    }

    #[test]
    fn storefront_origin_prefixes_https() {
        assert_eq!(
            storefront_origin("shop.myshopify.com"),
            "https://shop.myshopify.com"
        );
        assert_eq!(
            storefront_origin("http://127.0.0.1:9"),
            "http://127.0.0.1:9"
        );
    }
}
