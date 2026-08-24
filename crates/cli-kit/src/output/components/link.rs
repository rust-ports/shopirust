use crate::output::engine::contexts::LinksContext;
use crate::output::tokens::TokenItem;

/// Render a hyperlink as a TokenItem.
/// Uses OSC 8 hyperlinks if supported, otherwise falls back to `label (url)` or footnotes.
pub fn render_link(label: Option<&str>, url: &str) -> TokenItem {
    let value = label.unwrap_or(url).to_string();
    let fallback = label.map(|s| s.to_string());
    TokenItem::link(value, url, fallback)
}

/// Render a link with footnote numbering.
/// The footnote ID is allocated from the provided LinksContext.
pub fn render_link_with_footnote(
    label: Option<&str>,
    url: &str,
    ctx: &mut LinksContext,
) -> TokenItem {
    let label_str = label.unwrap_or(url);
    let id = ctx.add_link(label.map(|s| s.to_string()), url.to_string());
    TokenItem::link(
        format!("{label_str} [{id}]"),
        url,
        Some(format!("{label_str} [{id}]")),
    )
}

/// Render the footnotes block from a LinksContext.
pub fn render_footnotes(ctx: &LinksContext) -> Vec<TokenItem> {
    if ctx.is_empty() {
        return Vec::new();
    }
    let mut items = Vec::new();
    for (id, info) in ctx.links() {
        let label = info.label.clone().unwrap_or_else(|| info.url.clone());
        items.push(TokenItem::raw(format!("[{id}] {label} — {} ", info.url)));
    }
    items
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_link_basic() {
        let t = render_link(Some("Shopify"), "https://shopify.com");
        let plain = t.render_plain();
        assert_eq!(plain, "Shopify");
    }

    #[test]
    fn test_render_link_no_label() {
        let t = render_link(None, "https://shopify.com");
        assert!(t.render_plain().contains("shopify.com"));
    }

    #[test]
    fn test_render_link_with_footnote_adds_id() {
        let mut ctx = LinksContext::new();
        let t = render_link_with_footnote(Some("Docs"), "https://docs.shopify.com", &mut ctx);
        assert!(t.render_plain().contains("[1]"));
    }

    #[test]
    fn test_render_footnotes_empty() {
        let ctx = LinksContext::new();
        let items = render_footnotes(&ctx);
        assert!(items.is_empty());
    }

    #[test]
    fn test_render_footnotes_non_empty() {
        let mut ctx = LinksContext::new();
        ctx.add_link(Some("Shopify".into()), "https://shopify.com".into());
        let items = render_footnotes(&ctx);
        assert_eq!(items.len(), 1);
    }

    #[test]
    fn test_render_link_colors() {
        colored::control::set_override(true);
        let t = render_link(Some("Shopify"), "https://shopify.com");
        let out = t.render_ansi(true);
        // Link falls back to plain text when hyperlinks not supported
        assert!(out.contains("Shopify"));
    }

    #[test]
    fn test_render_link_footnote_plain() {
        let t = render_link(Some("Shopify"), "https://shopify.com");
        let plain = t.render_plain();
        assert_eq!(plain, "Shopify");
    }
}
