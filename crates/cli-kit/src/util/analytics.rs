use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const MONORAIL_ENDPOINT: &str = "https://monorail-edge.shopifysvc.com/v1/produce";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyticsEvent {
    pub schema_id: String,
    pub payload: HashMap<String, serde_json::Value>,
    pub project_external_id: Option<String>,
    pub shop_id: Option<String>,
}

pub async fn report_analytics_event(event: &AnalyticsEvent) -> Result<(), String> {
    let client = reqwest::Client::new();
    let response = client
        .post(MONORAIL_ENDPOINT)
        .json(event)
        .send()
        .await
        .map_err(|e| format!("Analytics send failed: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("Analytics rejected: {}", response.status()));
    }
    Ok(())
}

pub async fn report_unexpected_error(error_message: &str, stack_trace: Option<&str>) {
    let mut payload = HashMap::new();
    payload.insert("error".into(), serde_json::Value::String(error_message.to_string()));
    if let Some(stack) = stack_trace {
        payload.insert("stack".into(), serde_json::Value::String(stack.to_string()));
    }

    let event = AnalyticsEvent {
        schema_id: "cli/unexpected_error/1.0".into(),
        payload,
        project_external_id: None,
        shop_id: None,
    };

    let _ = report_analytics_event(&event).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analytics_event_serialization() {
        let mut payload = HashMap::new();
        payload.insert("key".into(), serde_json::Value::String("value".into()));

        let event = AnalyticsEvent {
            schema_id: "test/event/1.0".into(),
            payload,
            project_external_id: Some("proj_1".into()),
            shop_id: None,
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("test/event/1.0"));
        assert!(json.contains("proj_1"));
    }

    #[test]
    fn test_unexpected_error_event() {
        let mut payload = HashMap::new();
        payload.insert("error".into(), serde_json::Value::String("test error".into()));

        let event = AnalyticsEvent {
            schema_id: "cli/unexpected_error/1.0".into(),
            payload,
            project_external_id: None,
            shop_id: None,
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("test error"));
        assert!(json.contains("cli/unexpected_error/1.0"));
    }
}
