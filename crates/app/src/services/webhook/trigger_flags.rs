use crate::error::AppError;
use url::Url;

pub const DELIVERY_METHOD_LOCALHOST: &str = "localhost";
pub const DELIVERY_METHOD_HTTP: &str = "http";
pub const DELIVERY_METHOD_PUBSUB: &str = "google-pub-sub";
pub const DELIVERY_METHOD_EVENTBRIDGE: &str = "event-bridge";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryMethod {
    Localhost,
    Http,
    PubSub,
    EventBridge,
}

impl DeliveryMethod {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Localhost => DELIVERY_METHOD_LOCALHOST,
            Self::Http => DELIVERY_METHOD_HTTP,
            Self::PubSub => DELIVERY_METHOD_PUBSUB,
            Self::EventBridge => DELIVERY_METHOD_EVENTBRIDGE,
        }
    }

    pub fn parse(value: &str) -> Result<Self, AppError> {
        match value {
            DELIVERY_METHOD_LOCALHOST => Ok(Self::Localhost),
            DELIVERY_METHOD_HTTP => Ok(Self::Http),
            DELIVERY_METHOD_PUBSUB => Ok(Self::PubSub),
            DELIVERY_METHOD_EVENTBRIDGE => Ok(Self::EventBridge),
            other => Err(AppError::message(format!(
                "Unknown delivery method '{other}'. Allowed: http, google-pub-sub, event-bridge"
            ))),
        }
    }
}

fn is_local(address: &str) -> bool {
    if !address.to_ascii_lowercase().starts_with("http:") {
        return false;
    }
    Url::parse(address)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.eq_ignore_ascii_case("localhost")))
        .unwrap_or(false)
}

pub fn delivery_method_for_address(address: Option<&str>) -> Option<DeliveryMethod> {
    let address = address?;
    if address.starts_with("pubsub:") {
        return Some(DeliveryMethod::PubSub);
    }
    if address.starts_with("arn:aws:events:") {
        return Some(DeliveryMethod::EventBridge);
    }
    if is_local(address) {
        return Some(DeliveryMethod::Localhost);
    }
    if address.to_ascii_lowercase().starts_with("https:") {
        return Some(DeliveryMethod::Http);
    }
    None
}

pub fn is_address_allowed_for_delivery_method(address: &str, delivery_method: &str) -> bool {
    let expected = delivery_method_for_address(Some(address));
    if expected == Some(DeliveryMethod::Localhost) && delivery_method == DELIVERY_METHOD_HTTP {
        return true;
    }
    expected.map(|m| m.as_str()) == Some(delivery_method)
}

pub fn validate_address_method(
    address: &str,
    delivery_method: &str,
) -> Result<(String, String), AppError> {
    if !is_address_allowed_for_delivery_method(address, delivery_method) {
        return Err(AppError::message(format!(
            "Can't deliver your webhook payload to this address using '{delivery_method}'. Use a valid URL for address."
        )));
    }
    let mut method = delivery_method.to_string();
    if is_local(address) {
        method = DELIVERY_METHOD_LOCALHOST.to_string();
    }
    Ok((address.trim().to_string(), method))
}

pub fn parse_topic_flag(
    passed_topic: &str,
    api_version: &str,
    available_topics: &[String],
) -> Result<String, AppError> {
    if available_topics.is_empty() {
        return Err(AppError::message(format!(
            "No topics found for '{api_version}'"
        )));
    }
    let trimmed = passed_topic.trim();
    if available_topics.iter().any(|t| t == trimmed) {
        return Ok(trimmed.to_string());
    }
    if let Some(found) = available_topics
        .iter()
        .find(|t| t.to_uppercase().replace('/', "_") == trimmed)
    {
        return Ok(found.clone());
    }
    Err(AppError::message(format!(
        "Topic '{passed_topic}' does not exist for ApiVersion '{api_version}'. Allowed values: {}",
        available_topics.join(", ")
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn infers_delivery_methods() {
        assert_eq!(
            delivery_method_for_address(Some("pubsub://topic:sub")),
            Some(DeliveryMethod::PubSub)
        );
        assert_eq!(
            delivery_method_for_address(Some(
                "arn:aws:events:us-east-1::event-source/aws.partner/shopify.com/1/source"
            )),
            Some(DeliveryMethod::EventBridge)
        );
        assert_eq!(
            delivery_method_for_address(Some("http://localhost:3000/hooks")),
            Some(DeliveryMethod::Localhost)
        );
        assert_eq!(
            delivery_method_for_address(Some("https://example.com/hooks")),
            Some(DeliveryMethod::Http)
        );
        assert_eq!(
            delivery_method_for_address(Some("http://example.com/hooks")),
            None
        );
    }

    #[test]
    fn validates_localhost_with_http_method() {
        let (addr, method) =
            validate_address_method("http://localhost:9090/api/webhooks", "http").unwrap();
        assert_eq!(addr, "http://localhost:9090/api/webhooks");
        assert_eq!(method, DELIVERY_METHOD_LOCALHOST);
    }

    #[test]
    fn rejects_mismatched_address() {
        assert!(validate_address_method("https://example.com", "google-pub-sub").is_err());
    }

    #[test]
    fn parses_graphql_style_topic() {
        let topics = vec!["orders/create".into(), "products/update".into()];
        assert_eq!(
            parse_topic_flag("ORDERS_CREATE", "2024-07", &topics).unwrap(),
            "orders/create"
        );
        assert_eq!(
            parse_topic_flag("orders/create", "2024-07", &topics).unwrap(),
            "orders/create"
        );
    }
}
