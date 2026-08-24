use crate::error::StoreError;
use serde::{Deserialize, Serialize};

pub const POLL_INTERVAL_MS: u64 = 2_000;
pub const POLL_TIMEOUT_MS: u64 = 5 * 60 * 1_000;

pub const CREATE_APP_DEVELOPMENT_STORE_MUTATION: &str = r#"
mutation CreateAppDevelopmentStore($shopName: String!, $priceLookupKey: String!, $prepopulateTestData: Boolean) {
  createAppDevelopmentStore(
    shopName: $shopName
    priceLookupKey: $priceLookupKey
    prepopulateTestData: $prepopulateTestData
  ) {
    shopAdminUrl
    shopDomain
    userErrors { code field message }
  }
}
"#;

pub const POLL_STORE_CREATION_QUERY: &str = r#"
query PollStoreCreation($shopDomain: String!) {
  organization {
    id
    storeCreation(shopDomain: $shopDomain) {
      status
    }
  }
}
"#;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CreateDevStoreResult {
    pub shop_domain: String,
    pub shop_admin_url: Option<String>,
    pub user_errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CreateDevStoreOutput {
    pub store: CreateDevStoreOutputStore,
    pub organization: CreateDevStoreOutputOrg,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CreateDevStoreOutputStore {
    pub name: String,
    pub domain: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub admin_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CreateDevStoreOutputOrg {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreCreationStatus {
    CallingCore,
    AwaitingCoreStoreReady,
    Finalizing,
    Complete,
    Failed,
    TimedOut,
    UserError,
    Other(String),
}

impl StoreCreationStatus {
    pub fn parse(raw: &str) -> Self {
        match raw {
            "CALLING_CORE" => Self::CallingCore,
            "AWAITING_CORE_STORE_READY" => Self::AwaitingCoreStoreReady,
            "FINALIZING" => Self::Finalizing,
            "COMPLETE" => Self::Complete,
            "FAILED" => Self::Failed,
            "TIMED_OUT" => Self::TimedOut,
            "USER_ERROR" => Self::UserError,
            other => Self::Other(other.to_string()),
        }
    }

    pub fn is_terminal_failure(&self) -> bool {
        matches!(self, Self::Failed | Self::TimedOut | Self::UserError)
    }

    pub fn friendly_status(&self) -> String {
        match self {
            Self::CallingCore => "Initiating store creation...".into(),
            Self::AwaitingCoreStoreReady => "Waiting for store to be ready...".into(),
            Self::Finalizing => "Finalizing store setup...".into(),
            Self::Complete => "Store creation complete!".into(),
            Self::Failed => "Store creation failed.".into(),
            Self::TimedOut => "Store creation timed out.".into(),
            Self::UserError => "Store creation encountered a user error.".into(),
            Self::Other(status) => format!("Store creation status: {status}"),
        }
    }
}

pub fn parse_create_dev_response(value: &serde_json::Value) -> CreateDevStoreResult {
    let node = value
        .get("createAppDevelopmentStore")
        .or_else(|| {
            value
                .get("data")
                .and_then(|d| d.get("createAppDevelopmentStore"))
        })
        .cloned()
        .unwrap_or_else(|| value.clone());
    let errors = node
        .get("userErrors")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|e| {
            e.get("message")
                .and_then(|m| m.as_str())
                .map(str::to_string)
        })
        .collect();
    CreateDevStoreResult {
        shop_domain: node
            .get("shopDomain")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        shop_admin_url: node
            .get("shopAdminUrl")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        user_errors: errors,
    }
}

pub fn parse_poll_status(value: &serde_json::Value) -> Result<StoreCreationStatus, StoreError> {
    let status = value
        .pointer("/organization/storeCreation/status")
        .and_then(|v| v.as_str())
        .ok_or_else(|| StoreError::message("Unable to determine store creation status."))?;
    Ok(StoreCreationStatus::parse(status))
}

pub fn validate_create_dev_mutation(result: &CreateDevStoreResult) -> Result<(), StoreError> {
    if result.user_errors.is_empty()
        && result.shop_domain.is_empty()
        && result.shop_admin_url.is_none()
    {
        // Distinguish empty mutation payload from a successful empty domain.
        // Callers that received `createAppDevelopmentStore: null` should pass a marker.
    }
    if !result.user_errors.is_empty() {
        return Err(StoreError::message(format!(
            "Failed to create development store: {}",
            result.user_errors.join(", ")
        )));
    }
    if result.shop_domain.is_empty() {
        return Err(StoreError::message(
            "Store creation succeeded but no shop domain was returned.",
        ));
    }
    Ok(())
}

pub fn format_create_success_text(name: &str, result: &CreateDevStoreResult) -> String {
    format!(
        "Development store \"{name}\" created successfully.\nDomain: {}\nAdmin: {}",
        result.shop_domain,
        result.shop_admin_url.as_deref().unwrap_or("N/A")
    )
}

pub fn format_create_success_json(
    name: &str,
    result: &CreateDevStoreResult,
    organization_id: &str,
    organization_name: &str,
) -> String {
    let output = CreateDevStoreOutput {
        store: CreateDevStoreOutputStore {
            name: name.to_string(),
            domain: result.shop_domain.clone(),
            admin_url: result.shop_admin_url.clone(),
        },
        organization: CreateDevStoreOutputOrg {
            id: organization_id.to_string(),
            name: organization_name.to_string(),
        },
    };
    serde_json::to_string_pretty(&output).unwrap_or_else(|_| "{}".into())
}

/// Back-compat helper.
pub fn format_create_success(result: &CreateDevStoreResult) -> String {
    let mut out = format!("Created development store {}", result.shop_domain);
    if let Some(url) = &result.shop_admin_url {
        out.push_str(&format!("\nAdmin: {url}"));
    }
    out
}

#[async_trait::async_trait]
pub trait CreateDevStoreIo: Send + Sync {
    async fn create_store(
        &self,
        organization_id: &str,
        shop_name: &str,
    ) -> Result<serde_json::Value, StoreError>;
    async fn poll_status(
        &self,
        organization_id: &str,
        shop_domain: &str,
    ) -> Result<serde_json::Value, StoreError>;
    async fn sleep_ms(&self, ms: u64);
    fn now_ms(&self) -> u64;
    fn on_status(&self, _message: &str) {}
}

pub struct CreateDevStoreInput {
    pub name: String,
    pub organization_id: String,
    pub organization_name: String,
    pub json: bool,
}

pub async fn create_dev_store(
    input: CreateDevStoreInput,
    io: &dyn CreateDevStoreIo,
) -> Result<String, StoreError> {
    let mutation = io.create_store(&input.organization_id, &input.name).await?;
    if mutation
        .get("createAppDevelopmentStore")
        .map(|v| v.is_null())
        .unwrap_or(false)
    {
        return Err(StoreError::message(
            "Store creation failed: unexpected empty response.",
        ));
    }
    let result = parse_create_dev_response(&mutation);
    validate_create_dev_mutation(&result)?;

    let start = io.now_ms();
    loop {
        if io.now_ms().saturating_sub(start) > POLL_TIMEOUT_MS {
            return Err(StoreError::message(
                "Store creation timed out after 5 minutes.",
            ));
        }
        let poll = io
            .poll_status(&input.organization_id, &result.shop_domain)
            .await?;
        let status = parse_poll_status(&poll)?;
        if matches!(status, StoreCreationStatus::Complete) {
            break;
        }
        if status.is_terminal_failure() {
            return Err(StoreError::message(format!(
                "Store creation failed with status: {}",
                match status {
                    StoreCreationStatus::Failed => "FAILED",
                    StoreCreationStatus::TimedOut => "TIMED_OUT",
                    StoreCreationStatus::UserError => "USER_ERROR",
                    StoreCreationStatus::Other(ref s) => s.as_str(),
                    _ => "UNKNOWN",
                }
            )));
        }
        io.on_status(&status.friendly_status());
        io.sleep_ms(POLL_INTERVAL_MS).await;
    }

    if input.json {
        Ok(format_create_success_json(
            &input.name,
            &result,
            &input.organization_id,
            &input.organization_name,
        ))
    } else {
        Ok(format_create_success_text(&input.name, &result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::Mutex;

    struct FakeIo {
        create: Mutex<Result<serde_json::Value, StoreError>>,
        polls: Mutex<Vec<Result<serde_json::Value, StoreError>>>,
        now: Mutex<u64>,
        sleeps: Mutex<u32>,
        statuses: Mutex<Vec<String>>,
    }

    #[async_trait::async_trait]
    impl CreateDevStoreIo for FakeIo {
        async fn create_store(
            &self,
            _organization_id: &str,
            _shop_name: &str,
        ) -> Result<serde_json::Value, StoreError> {
            match &*self.create.lock().unwrap() {
                Ok(v) => Ok(v.clone()),
                Err(e) => Err(e.clone()),
            }
        }
        async fn poll_status(
            &self,
            _organization_id: &str,
            _shop_domain: &str,
        ) -> Result<serde_json::Value, StoreError> {
            let mut polls = self.polls.lock().unwrap();
            if polls.is_empty() {
                return Ok(json!({"organization":{"storeCreation":{"status":"COMPLETE"}}}));
            }
            polls.remove(0)
        }
        async fn sleep_ms(&self, _ms: u64) {
            *self.sleeps.lock().unwrap() += 1;
            *self.now.lock().unwrap() += POLL_INTERVAL_MS;
        }
        fn now_ms(&self) -> u64 {
            *self.now.lock().unwrap()
        }
        fn on_status(&self, message: &str) {
            self.statuses.lock().unwrap().push(message.to_string());
        }
    }

    fn ok_mutation() -> serde_json::Value {
        json!({
            "createAppDevelopmentStore": {
                "shopAdminUrl": "https://test-store.myshopify.com/admin",
                "shopDomain": "test-store.myshopify.com",
                "userErrors": []
            }
        })
    }

    #[tokio::test]
    async fn creates_and_polls_to_complete() {
        let io = FakeIo {
            create: Mutex::new(Ok(ok_mutation())),
            polls: Mutex::new(vec![
                Ok(json!({"organization":{"storeCreation":{"status":"CALLING_CORE"}}})),
                Ok(json!({"organization":{"storeCreation":{"status":"COMPLETE"}}})),
            ]),
            now: Mutex::new(0),
            sleeps: Mutex::new(0),
            statuses: Mutex::new(vec![]),
        };
        let out = create_dev_store(
            CreateDevStoreInput {
                name: "test-store".into(),
                organization_id: "123".into(),
                organization_name: "Test Org".into(),
                json: false,
            },
            &io,
        )
        .await
        .unwrap();
        assert!(out.contains("test-store"));
        assert!(out.contains("test-store.myshopify.com"));
        assert_eq!(*io.sleeps.lock().unwrap(), 1);
        assert!(io
            .statuses
            .lock()
            .unwrap()
            .iter()
            .any(|s| s.contains("Initiating")));
    }

    #[tokio::test]
    async fn outputs_json() {
        let io = FakeIo {
            create: Mutex::new(Ok(ok_mutation())),
            polls: Mutex::new(vec![Ok(
                json!({"organization":{"storeCreation":{"status":"COMPLETE"}}}),
            )]),
            now: Mutex::new(0),
            sleeps: Mutex::new(0),
            statuses: Mutex::new(vec![]),
        };
        let out = create_dev_store(
            CreateDevStoreInput {
                name: "test-store".into(),
                organization_id: "123".into(),
                organization_name: "Test Org".into(),
                json: true,
            },
            &io,
        )
        .await
        .unwrap();
        let payload: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(payload["store"]["domain"], "test-store.myshopify.com");
        assert_eq!(payload["organization"]["id"], "123");
    }

    #[tokio::test]
    async fn rejects_null_mutation() {
        let io = FakeIo {
            create: Mutex::new(Ok(json!({"createAppDevelopmentStore": null}))),
            polls: Mutex::new(vec![]),
            now: Mutex::new(0),
            sleeps: Mutex::new(0),
            statuses: Mutex::new(vec![]),
        };
        let err = create_dev_store(
            CreateDevStoreInput {
                name: "test-store".into(),
                organization_id: "123".into(),
                organization_name: "Test Org".into(),
                json: false,
            },
            &io,
        )
        .await
        .unwrap_err();
        assert!(err.to_string().contains("unexpected empty response"));
    }

    #[tokio::test]
    async fn rejects_user_errors() {
        let io = FakeIo {
            create: Mutex::new(Ok(json!({
                "createAppDevelopmentStore": {
                    "shopAdminUrl": null,
                    "shopDomain": null,
                    "userErrors": [{"message":"Name is taken"}]
                }
            }))),
            polls: Mutex::new(vec![]),
            now: Mutex::new(0),
            sleeps: Mutex::new(0),
            statuses: Mutex::new(vec![]),
        };
        let err = create_dev_store(
            CreateDevStoreInput {
                name: "test-store".into(),
                organization_id: "123".into(),
                organization_name: "Test Org".into(),
                json: false,
            },
            &io,
        )
        .await
        .unwrap_err();
        assert!(err.to_string().contains("Name is taken"));
    }

    #[tokio::test]
    async fn times_out() {
        let io = FakeIo {
            create: Mutex::new(Ok(ok_mutation())),
            polls: Mutex::new(vec![Ok(
                json!({"organization":{"storeCreation":{"status":"FINALIZING"}}}),
            )]),
            now: Mutex::new(0),
            sleeps: Mutex::new(0),
            statuses: Mutex::new(vec![]),
        };
        // First poll returns FINALIZING; sleep advances clock past timeout before next iteration.
        // Override sleep behavior via pre-seeded now bump after first status.
        struct TimeoutIo {
            inner: FakeIo,
        }
        #[async_trait::async_trait]
        impl CreateDevStoreIo for TimeoutIo {
            async fn create_store(
                &self,
                organization_id: &str,
                shop_name: &str,
            ) -> Result<serde_json::Value, StoreError> {
                self.inner.create_store(organization_id, shop_name).await
            }
            async fn poll_status(
                &self,
                organization_id: &str,
                shop_domain: &str,
            ) -> Result<serde_json::Value, StoreError> {
                self.inner.poll_status(organization_id, shop_domain).await
            }
            async fn sleep_ms(&self, _ms: u64) {
                *self.inner.sleeps.lock().unwrap() += 1;
                *self.inner.now.lock().unwrap() = POLL_TIMEOUT_MS + 2;
            }
            fn now_ms(&self) -> u64 {
                self.inner.now_ms()
            }
        }
        let io = TimeoutIo { inner: io };
        let err = create_dev_store(
            CreateDevStoreInput {
                name: "test-store".into(),
                organization_id: "123".into(),
                organization_name: "Test Org".into(),
                json: false,
            },
            &io,
        )
        .await
        .unwrap_err();
        assert!(err.to_string().contains("timed out after 5 minutes"));
    }

    #[tokio::test]
    async fn fails_on_terminal_poll_status() {
        let io = FakeIo {
            create: Mutex::new(Ok(ok_mutation())),
            polls: Mutex::new(vec![Ok(
                json!({"organization":{"storeCreation":{"status":"FAILED"}}}),
            )]),
            now: Mutex::new(0),
            sleeps: Mutex::new(0),
            statuses: Mutex::new(vec![]),
        };
        let err = create_dev_store(
            CreateDevStoreInput {
                name: "test-store".into(),
                organization_id: "123".into(),
                organization_name: "Test Org".into(),
                json: false,
            },
            &io,
        )
        .await
        .unwrap_err();
        assert!(err.to_string().contains("FAILED"));
    }

    #[test]
    fn parses_success() {
        let result = parse_create_dev_response(&ok_mutation());
        assert_eq!(result.shop_domain, "test-store.myshopify.com");
        assert!(result.user_errors.is_empty());
    }

    #[test]
    fn friendly_status_messages() {
        assert!(StoreCreationStatus::CallingCore
            .friendly_status()
            .contains("Initiating"));
        assert!(StoreCreationStatus::Complete
            .friendly_status()
            .contains("complete"));
    }
}
