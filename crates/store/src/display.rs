use chrono::{DateTime, Utc};

pub use crate::list::result::format_store_list;

pub fn format_short_date(value: &str) -> String {
    let parsed = DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|d| d.with_timezone(&Utc))
        .or_else(|| {
            DateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%SZ")
                .ok()
                .map(|d| d.with_timezone(&Utc))
        });
    let Some(date) = parsed else {
        return String::new();
    };
    date.format("%b %d, %Y").to_string()
}

pub fn extract_subdomain(value: &str) -> Option<String> {
    let host = value
        .trim()
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .split('/')
        .next()
        .unwrap_or(value);
    host.split('.').next().map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::list::{format_store_list, list_stores, StoreListEntry};

    #[test]
    fn empty() {
        let result = list_stores(vec![], None, false);
        assert!(format_store_list(&result, false).contains("No stores found"));
    }

    #[test]
    fn rows() {
        let result = list_stores(
            vec![StoreListEntry {
                id: Some("1".into()),
                store: "a.myshopify.com".into(),
                created_at: "2026-01-01T00:00:00Z".into(),
                organization_id: "1".into(),
                organization_name: "Acme".into(),
                name: Some("A".into()),
                store_type: Some("dev".into()),
            }],
            None,
            false,
        );
        let text = format_store_list(&result, false);
        assert!(text.contains('A'));
        assert!(format_store_list(&result, true).contains("a.myshopify.com"));
    }

    #[test]
    fn short_date_utc() {
        assert_eq!(format_short_date("2026-05-22T00:00:00Z"), "May 22, 2026");
    }

    #[test]
    fn subdomain_from_fqdn() {
        assert_eq!(
            extract_subdomain("my-shop.myshopify.com").as_deref(),
            Some("my-shop")
        );
    }
}
