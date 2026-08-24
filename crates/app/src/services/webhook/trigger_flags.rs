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
    Url::parse(&address.to_ascii_lowercase())
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

pub fn delivery_method_instructions(method: &str) -> Vec<String> {
    if method == DELIVERY_METHOD_HTTP {
        return vec![
            "For remote HTTP testing, use a URL that starts with https://".into(),
            "For local HTTP testing, use http://localhost:{port}/{url-path}".into(),
        ];
    }
    if method == DELIVERY_METHOD_PUBSUB {
        return vec!["For Google Pub/Sub, use pubsub://{project-id}:{topic-id}".into()];
    }
    if method == DELIVERY_METHOD_EVENTBRIDGE {
        return vec![
            "For Amazon EventBridge, use an Amazon Resource Name (ARN) starting with arn:aws:events:"
                .into(),
        ];
    }
    vec![]
}

pub fn delivery_method_instructions_as_string(method: &str) -> String {
    delivery_method_instructions(method)
        .into_iter()
        .map(|hint| format!("      · {hint}"))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn validate_address_method(
    address: &str,
    delivery_method: &str,
) -> Result<(String, String), AppError> {
    if !is_address_allowed_for_delivery_method(address, delivery_method) {
        let hints = delivery_method_instructions(delivery_method);
        let hint = if hints.is_empty() {
            "Use a valid URL for address.".to_string()
        } else {
            hints.join(" ")
        };
        return Err(AppError::message(format!(
            "Can't deliver your webhook payload to this address using '{delivery_method}'. {hint}"
        )));
    }
    let mut method = delivery_method.to_string();
    if is_local(address) {
        method = DELIVERY_METHOD_LOCALHOST.to_string();
    }
    Ok((address.trim().to_string(), method))
}

pub fn parse_api_version_flag(
    passed_version: &str,
    available_versions: &[String],
) -> Result<String, AppError> {
    if available_versions.iter().any(|v| v == passed_version) {
        return Ok(passed_version.to_string());
    }
    Err(AppError::message(format!(
        "Api Version '{passed_version}' does not exist. Allowed values: {}",
        available_versions.join(", ")
    )))
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

    const EVENTBRIDGE_ADDRESS: &str =
        "arn:aws:events:us-east-1::event-source/aws.partner/shopify.com/3737297/source";
    const PUBSUB_ADDRESS: &str = "pubsub://topic:subscription";
    const REMOTE_HTTP_ADDRESS: &str = "https://example.org/api/webhooks";
    const LOCAL_HTTP_ADDRESS: &str = "http://localhost:9090/api/webhooks";
    const FTP_ADDRESS: &str = "ftp://user:pass@host";

    #[test]
    fn pubsub_accepts_pubsub_addresses() {
        assert!(is_address_allowed_for_delivery_method(
            PUBSUB_ADDRESS,
            DELIVERY_METHOD_PUBSUB
        ));
    }

    #[test]
    fn pubsub_rejects_non_pubsub_addresses() {
        assert!(!is_address_allowed_for_delivery_method(
            REMOTE_HTTP_ADDRESS,
            DELIVERY_METHOD_PUBSUB
        ));
    }

    #[test]
    fn eventbridge_accepts_arn_addresses() {
        assert!(is_address_allowed_for_delivery_method(
            EVENTBRIDGE_ADDRESS,
            DELIVERY_METHOD_EVENTBRIDGE
        ));
    }

    #[test]
    fn eventbridge_rejects_non_arn_addresses() {
        assert!(!is_address_allowed_for_delivery_method(
            REMOTE_HTTP_ADDRESS,
            DELIVERY_METHOD_EVENTBRIDGE
        ));
    }

    #[test]
    fn http_accepts_localhost_addresses() {
        assert!(is_address_allowed_for_delivery_method(
            LOCAL_HTTP_ADDRESS,
            DELIVERY_METHOD_HTTP
        ));
    }

    #[test]
    fn http_accepts_https_remote_addresses() {
        assert!(is_address_allowed_for_delivery_method(
            REMOTE_HTTP_ADDRESS,
            DELIVERY_METHOD_HTTP
        ));
    }

    #[test]
    fn http_rejects_http_remote_addresses() {
        assert!(!is_address_allowed_for_delivery_method(
            "http://example.org/api/webhooks",
            DELIVERY_METHOD_HTTP
        ));
    }

    #[test]
    fn rejects_unknown_address_formats() {
        assert!(!is_address_allowed_for_delivery_method(
            FTP_ADDRESS,
            DELIVERY_METHOD_HTTP
        ));
    }

    #[test]
    fn validate_returns_address_method_for_http() {
        assert_eq!(
            validate_address_method("https://example.org", "http").unwrap(),
            ("https://example.org".into(), "http".into())
        );
    }

    #[test]
    fn validate_returns_localhost_when_http_with_localhost_address() {
        assert_eq!(
            validate_address_method("http://localhost:3000/webhooks", "http").unwrap(),
            (
                "http://localhost:3000/webhooks".into(),
                DELIVERY_METHOD_LOCALHOST.into()
            )
        );
    }

    #[test]
    fn validate_returns_address_method_for_pubsub() {
        assert_eq!(
            validate_address_method(PUBSUB_ADDRESS, "google-pub-sub").unwrap(),
            (PUBSUB_ADDRESS.into(), DELIVERY_METHOD_PUBSUB.into())
        );
    }

    #[test]
    fn validate_returns_address_method_for_eventbridge() {
        assert_eq!(
            validate_address_method(EVENTBRIDGE_ADDRESS, "event-bridge").unwrap(),
            (
                EVENTBRIDGE_ADDRESS.into(),
                DELIVERY_METHOD_EVENTBRIDGE.into()
            )
        );
    }

    #[test]
    fn validate_fails_when_incompatible() {
        assert!(validate_address_method("https://example.org", "google-pub-sub").is_err());
    }

    #[test]
    fn infers_pubsub_address() {
        assert_eq!(
            delivery_method_for_address(Some(PUBSUB_ADDRESS)),
            Some(DeliveryMethod::PubSub)
        );
    }

    #[test]
    fn infers_eventbridge_address() {
        assert_eq!(
            delivery_method_for_address(Some(EVENTBRIDGE_ADDRESS)),
            Some(DeliveryMethod::EventBridge)
        );
    }

    #[test]
    fn infers_localhost_address() {
        assert_eq!(
            delivery_method_for_address(Some(LOCAL_HTTP_ADDRESS)),
            Some(DeliveryMethod::Localhost)
        );
    }

    #[test]
    fn infers_localhost_address_case_insensitive() {
        assert_eq!(
            delivery_method_for_address(Some(&LOCAL_HTTP_ADDRESS.to_uppercase())),
            Some(DeliveryMethod::Localhost)
        );
    }

    #[test]
    fn infers_remote_http_address() {
        assert_eq!(
            delivery_method_for_address(Some(REMOTE_HTTP_ADDRESS)),
            Some(DeliveryMethod::Http)
        );
    }

    #[test]
    fn infers_remote_http_address_case_insensitive() {
        assert_eq!(
            delivery_method_for_address(Some(&REMOTE_HTTP_ADDRESS.to_uppercase())),
            Some(DeliveryMethod::Http)
        );
    }

    #[test]
    fn infers_none_for_unknown() {
        assert_eq!(delivery_method_for_address(Some(FTP_ADDRESS)), None);
        assert_eq!(delivery_method_for_address(None), None);
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

    #[test]
    fn parse_topic_empty_list_errors() {
        let err = parse_topic_flag("orders/create", "2024-07", &[]).unwrap_err();
        assert!(err.to_string().contains("No topics found"));
    }

    #[test]
    fn parse_api_version_flag_accepts_known() {
        assert_eq!(
            parse_api_version_flag("2023-01", &["2023-01".into(), "unstable".into()]).unwrap(),
            "2023-01"
        );
    }

    #[test]
    fn parse_api_version_flag_rejects_unknown() {
        let err = parse_api_version_flag("nope", &["2023-01".into()]).unwrap_err();
        assert!(err.to_string().contains("does not exist"));
        assert!(err.to_string().contains("2023-01"));
    }

    #[test]
    fn instructions_for_each_method() {
        assert!(delivery_method_instructions_as_string(DELIVERY_METHOD_HTTP).contains("https://"));
        assert!(
            delivery_method_instructions_as_string(DELIVERY_METHOD_PUBSUB).contains("pubsub://")
        );
        assert!(
            delivery_method_instructions_as_string(DELIVERY_METHOD_EVENTBRIDGE)
                .contains("arn:aws:events:")
        );
        assert!(delivery_method_instructions("unknown").is_empty());
    }

    #[test]
    fn delivery_method_parse() {
        assert_eq!(DeliveryMethod::parse("http").unwrap(), DeliveryMethod::Http);
        assert!(DeliveryMethod::parse("ftp").is_err());
    }
}
