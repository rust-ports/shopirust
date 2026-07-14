use std::collections::HashMap;

pub fn add_cursor_and_filters_to_app_logs_url(
    base_url: &str,
    cursor: Option<&str>,
    filters: Option<HashMap<String, String>>,
) -> String {
    let mut url = base_url.to_string();
    let mut params: Vec<String> = Vec::new();

    if let Some(c) = cursor {
        params.push(format!("cursor={}", c));
    }

    if let Some(f) = filters {
        if let Some(status) = f.get("status") {
            params.push(format!("status={}", status));
        }
        if let Some(source) = f.get("source") {
            params.push(format!("source={}", source));
        }
    }

    if !params.is_empty() {
        url.push('?');
        url.push_str(&params.join("&"));
    }

    url
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_cursor_param() {
        let url = add_cursor_and_filters_to_app_logs_url(
            "https://app.shopify.com/app_management/unstable/orgs/1/app_logs/poll",
            Some("cursor-abc"),
            None,
        );
        assert!(url.contains("cursor=cursor-abc"));
    }

    #[test]
    fn adds_status_filter() {
        let mut filters = HashMap::new();
        filters.insert("status".into(), "success".into());
        let url = add_cursor_and_filters_to_app_logs_url(
            "https://app.shopify.com/app_management/unstable/orgs/1/app_logs/poll",
            None,
            Some(filters),
        );
        assert!(url.contains("status=success"));
    }

    #[test]
    fn adds_multiple_params() {
        let mut filters = HashMap::new();
        filters.insert("status".into(), "failure".into());
        filters.insert("source".into(), "checkout".into());
        let url = add_cursor_and_filters_to_app_logs_url(
            "https://app.shopify.com/app_management/unstable/orgs/1/app_logs/poll",
            Some("abc"),
            Some(filters),
        );
        assert!(url.contains("cursor=abc"));
        assert!(url.contains("status=failure"));
        assert!(url.contains("source=checkout"));
    }

    #[test]
    fn no_params_returns_base_url() {
        let url = add_cursor_and_filters_to_app_logs_url(
            "https://app.shopify.com/base",
            None,
            None,
        );
        assert_eq!(url, "https://app.shopify.com/base");
    }
}
