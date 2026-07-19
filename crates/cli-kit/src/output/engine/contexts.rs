use std::collections::HashMap;

/// Tracks hyperlinks for footnote rendering inside Banners.
#[derive(Debug, Clone)]
pub struct LinksContext {
    links: HashMap<String, LinkInfo>,
    counter: usize,
}

#[derive(Debug, Clone)]
pub struct LinkInfo {
    pub label: Option<String>,
    pub url: String,
}

impl LinksContext {
    pub fn new() -> Self {
        Self {
            links: HashMap::new(),
            counter: 0,
        }
    }

    /// Register a link and return its numeric ID.
    /// Deduplicates by URL — returns the same ID for the same URL.
    pub fn add_link(&mut self, label: Option<String>, url: String) -> String {
        if let Some((id, _)) = self.links.iter().find(|(_, info)| info.url == url) {
            return id.clone();
        }
        self.counter += 1;
        let id = self.counter.to_string();
        self.links.insert(
            id.clone(),
            LinkInfo {
                label,
                url,
            },
        );
        id
    }

    pub fn links(&self) -> &HashMap<String, LinkInfo> {
        &self.links
    }

    pub fn is_empty(&self) -> bool {
        self.links.is_empty()
    }
}

impl Default for LinksContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Signals completion of an interactive flow (Ink's `useComplete` equivalent).
#[derive(Debug, Clone)]
pub struct CompletionContext {
    completed: bool,
    error: Option<String>,
}

impl CompletionContext {
    pub fn new() -> Self {
        Self {
            completed: false,
            error: None,
        }
    }

    pub fn complete(&mut self) {
        self.completed = true;
    }

    pub fn complete_with_error(&mut self, error: String) {
        self.completed = true;
        self.error = Some(error);
    }

    pub fn is_completed(&self) -> bool {
        self.completed
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
}

impl Default for CompletionContext {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_links_context_add_link() {
        let mut ctx = LinksContext::new();
        let id = ctx.add_link(Some("Shopify".into()), "https://shopify.com".into());
        assert_eq!(id, "1");
    }

    #[test]
    fn test_links_context_deduplicates() {
        let mut ctx = LinksContext::new();
        let id1 = ctx.add_link(Some("A".into()), "https://example.com".into());
        let id2 = ctx.add_link(Some("B".into()), "https://example.com".into());
        assert_eq!(id1, id2);
    }

    #[test]
    fn test_completion_context_default_not_completed() {
        let ctx = CompletionContext::new();
        assert!(!ctx.is_completed());
    }

    #[test]
    fn test_completion_context_complete() {
        let mut ctx = CompletionContext::new();
        ctx.complete();
        assert!(ctx.is_completed());
        assert!(ctx.error().is_none());
    }

    #[test]
    fn test_completion_context_with_error() {
        let mut ctx = CompletionContext::new();
        ctx.complete_with_error("oops".into());
        assert!(ctx.is_completed());
        assert_eq!(ctx.error(), Some("oops"));
    }

    #[test]
    fn test_links_context_is_empty() {
        let ctx = LinksContext::new();
        assert!(ctx.is_empty());
    }
}
