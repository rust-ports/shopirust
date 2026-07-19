use std::sync::atomic::{AtomicUsize, Ordering};

static LINK_COUNTER: AtomicUsize = AtomicUsize::new(0);

fn supports_hyperlinks() -> bool {
    std::env::var("TERM").ok().is_some_and(|term| {
        term != "dumb" && term != "linux"
    }) && !std::env::var("FORCE_HYPERLINK").unwrap_or_default().is_empty()
}

pub fn render_link(label: Option<&str>, url: &str) -> String {
    if supports_hyperlinks() {
        let text = label.unwrap_or(url);
        format!("\x1b]8;;{url}\x1b\\{text}\x1b]8;;\x1b\\")
    } else {
        let label = label.unwrap_or(url);
        if url == label {
            url.to_string()
        } else {
            format!("{label} ( {url} )")
        }
    }
}

pub fn render_link_with_footnote(label: Option<&str>, url: &str) -> String {
    if supports_hyperlinks() {
        let text = label.unwrap_or(url);
        format!("\x1b]8;;{url}\x1b\\{text}\x1b]8;;\x1b\\")
    } else {
        let counter = LINK_COUNTER.fetch_add(1, Ordering::Relaxed);
        let label = label.unwrap_or(url);
        format!("{label} [{counter}]")
    }
}

pub fn render_footnotes(footnotes: &[(String, String)]) -> String {
    if footnotes.is_empty() {
        return String::new();
    }
    footnotes
        .iter()
        .enumerate()
        .map(|(i, (label, url))| {
            let label = if label.is_empty() { url } else { label };
            format!("[{i}] {label} — {url}")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn reset_link_counter() {
    LINK_COUNTER.store(0, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_link_without_hyperlink_fallback() {
        let result = render_link(Some("Shopify"), "https://shopify.com");
        assert!(result.contains("Shopify"));
    }

    #[test]
    fn test_render_link_url_only() {
        let result = render_link(None, "https://shopify.com");
        assert!(result.contains("https://shopify.com"));
    }

    #[test]
    fn test_render_link_with_footnote() {
        let result = render_link_with_footnote(Some("Docs"), "https://docs.shopify.com");
        assert!(result.contains("Docs"));
    }

    #[test]
    fn test_render_footnotes_empty() {
        assert_eq!(render_footnotes(&[]), "");
    }

    #[test]
    fn test_reset_link_counter() {
        reset_link_counter();
        assert_eq!(LINK_COUNTER.load(Ordering::Relaxed), 0);
    }
}
