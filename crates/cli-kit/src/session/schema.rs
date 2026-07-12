use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityToken {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: DateTime<Utc>,
    pub scopes: Vec<String>,
    pub user_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplicationToken {
    pub access_token: String,
    pub expires_at: DateTime<Utc>,
    pub scopes: Vec<String>,
}

pub type ApplicationTokens = HashMap<String, ApplicationToken>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub identity: IdentityToken,
    pub applications: ApplicationTokens,
}

pub type Sessions = HashMap<String, HashMap<String, Session>>;
