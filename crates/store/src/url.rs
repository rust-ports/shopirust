/// Lowercased hostname from a URL or bare host, matching cli-kit `extractHost`.
pub fn extract_host(value: &str) -> Option<String> {
    let lowered = value.trim().to_lowercase();
    if lowered.is_empty() {
        return None;
    }
    let with_scheme = format!("https://{lowered}");
    for candidate in [&lowered, &with_scheme] {
        if let Ok(parsed) = url::Url::parse(candidate) {
            if let Some(host) = parsed.host_str() {
                if !host.is_empty() {
                    return Some(host.to_string());
                }
            }
        }
    }
    let fallback = lowered
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .split('/')
        .next()
        .unwrap_or("");
    if fallback.is_empty() {
        None
    } else {
        Some(fallback.to_string())
    }
}

/// First DNS label of a host, used as the BP destinations search term.
pub fn extract_search_subdomain(host: &str) -> String {
    host.split('.').next().unwrap_or(host).to_string()
}

/// Myshopify subdomain handle (`shop` from `shop.myshopify.com`).
pub fn extract_myshopify_handle(value: &str) -> Option<String> {
    let host = extract_host(value)?;
    let handle = host.strip_suffix(".myshopify.com")?;
    if handle.is_empty() || handle.contains('.') {
        None
    } else {
        Some(handle.to_string())
    }
}

pub fn encode_uri_component(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

pub fn build_admin_url(handle: Option<&str>) -> Option<String> {
    let handle = handle.filter(|h| !h.is_empty())?;
    Some(format!(
        "https://admin.shopify.com/store/{}",
        encode_uri_component(handle)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_from_url_and_bare() {
        assert_eq!(
            extract_host("https://Shop.myshopify.com/admin"),
            Some("shop.myshopify.com".into())
        );
        assert_eq!(
            extract_host("shop.myshopify.com"),
            Some("shop.myshopify.com".into())
        );
    }

    #[test]
    fn myshopify_handle() {
        assert_eq!(
            extract_myshopify_handle("https://acme-widgets.myshopify.com"),
            Some("acme-widgets".into())
        );
        assert!(extract_myshopify_handle("my-dev-store.shop.dev").is_none());
    }

    #[test]
    fn search_subdomain_local_dev() {
        assert_eq!(extract_search_subdomain("my-dev-store.shop.dev"), "my-dev-store");
        assert_eq!(extract_search_subdomain("shop.myshopify.com"), "shop");
    }
}
