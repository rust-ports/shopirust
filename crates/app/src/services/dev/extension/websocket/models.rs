//! WebSocket message types.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EventType {
    Update,
    Dispatch,
    Log,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncomingMessage {
    pub event: String,
    #[serde(default)]
    pub data: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogPayload {
    #[serde(rename = "type")]
    pub log_type: String,
    pub message: String,
    #[serde(rename = "extensionName")]
    pub extension_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsClientMessage {
    pub event: String,
    pub data: Value,
}
