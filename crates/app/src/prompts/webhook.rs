//! Webhook trigger prompts (upstream `prompts/webhook/trigger.ts`).

use super::{PromptItem, Prompter};
use crate::error::AppError;
use crate::services::webhook::{
    delivery_method_instructions_as_string, is_address_allowed_for_delivery_method,
    DELIVERY_METHOD_EVENTBRIDGE, DELIVERY_METHOD_HTTP, DELIVERY_METHOD_PUBSUB,
};

pub fn prompt_topic(prompter: &dyn Prompter, topics: &[String]) -> Result<String, AppError> {
    if topics.is_empty() {
        return prompter.text("Webhook Topic", None);
    }
    let items: Vec<_> = topics
        .iter()
        .map(|t| PromptItem::new(t.clone(), t.clone()))
        .collect();
    prompter.autocomplete("Webhook Topic", &items)
}

pub fn prompt_api_version(
    prompter: &dyn Prompter,
    versions: &[String],
) -> Result<String, AppError> {
    if versions.is_empty() {
        return prompter.text("Webhook ApiVersion", Some("2025-01"));
    }
    let items: Vec<_> = versions
        .iter()
        .map(|v| PromptItem::new(v.clone(), v.clone()))
        .collect();
    prompter.select("Webhook ApiVersion", &items)
}

pub fn prompt_delivery_method(prompter: &dyn Prompter) -> Result<String, AppError> {
    let items = vec![
        PromptItem::new("HTTP", DELIVERY_METHOD_HTTP),
        PromptItem::new("Google Pub/Sub", DELIVERY_METHOD_PUBSUB),
        PromptItem::new("Amazon EventBridge", DELIVERY_METHOD_EVENTBRIDGE),
    ];
    prompter.select("Delivery method", &items)
}

pub fn prompt_address(
    prompter: &dyn Prompter,
    delivery_method: &str,
) -> Result<String, AppError> {
    let addr = prompter.text("Address for delivery", None)?;
    let trimmed = addr.trim();
    if trimmed.is_empty() {
        return Err(AppError::message("Address can't be empty"));
    }
    if !is_address_allowed_for_delivery_method(trimmed, delivery_method) {
        return Err(AppError::message(format!(
            "Invalid address.\n{}",
            delivery_method_instructions_as_string(delivery_method)
        )));
    }
    Ok(trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prompts::InjectedPrompter;

    #[test]
    fn topic_from_list() {
        let p = InjectedPrompter::new();
        p.push_select("orders/create");
        assert_eq!(
            prompt_topic(&p, &["orders/create".into(), "anything/else".into()]).unwrap(),
            "orders/create"
        );
    }

    #[test]
    fn topic_empty_falls_back_to_text() {
        let p = InjectedPrompter::new();
        p.push_text("orders/create");
        assert_eq!(prompt_topic(&p, &[]).unwrap(), "orders/create");
    }

    #[test]
    fn api_version_prompt() {
        let p = InjectedPrompter::new();
        p.push_select("2022-10");
        assert_eq!(
            prompt_api_version(&p, &["2023-01".into(), "2022-10".into(), "unstable".into()])
                .unwrap(),
            "2022-10"
        );
    }

    #[test]
    fn delivery_method_prompt_http() {
        let p = InjectedPrompter::new();
        p.push_select("http");
        assert_eq!(prompt_delivery_method(&p).unwrap(), "http");
    }

    #[test]
    fn delivery_method_prompt_defaults_to_http() {
        let p = InjectedPrompter::new();
        assert_eq!(prompt_delivery_method(&p).unwrap(), "http");
    }

    #[test]
    fn address_prompt_https() {
        let p = InjectedPrompter::new();
        p.push_text("https://example.org");
        assert_eq!(
            prompt_address(&p, DELIVERY_METHOD_HTTP).unwrap(),
            "https://example.org"
        );
    }

    #[test]
    fn address_prompt_rejects_empty() {
        let p = InjectedPrompter::new();
        p.push_text("   ");
        assert!(prompt_address(&p, DELIVERY_METHOD_HTTP)
            .unwrap_err()
            .to_string()
            .contains("empty"));
    }

    #[test]
    fn address_prompt_rejects_invalid_for_method() {
        let p = InjectedPrompter::new();
        p.push_text("https://example.org");
        let err = prompt_address(&p, DELIVERY_METHOD_PUBSUB).unwrap_err();
        assert!(err.to_string().contains("Invalid address"));
        assert!(err.to_string().contains("pubsub://"));
    }

    #[test]
    fn address_prompt_accepts_localhost_for_http() {
        let p = InjectedPrompter::new();
        p.push_text("http://localhost:3000/hooks");
        assert_eq!(
            prompt_address(&p, DELIVERY_METHOD_HTTP).unwrap(),
            "http://localhost:3000/hooks"
        );
    }
}
