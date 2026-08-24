//! Predefined app templates (upstream `prompts/init/init.ts`).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TemplateBranch {
    pub branch: &'static str,
    pub label: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppTemplate {
    pub key: &'static str,
    pub url: &'static str,
    pub label: &'static str,
    pub visible: bool,
    pub flavor_prompt: Option<&'static str>,
    pub flavors: &'static [(&'static str, TemplateBranch)],
}

pub const TEMPLATES: &[AppTemplate] = &[
    AppTemplate {
        key: "reactRouter",
        url: "https://github.com/Shopify/shopify-app-template-react-router",
        label: "Build a React Router app (recommended)",
        visible: true,
        flavor_prompt: Some("For your React Router template, which language do you want?"),
        flavors: &[
            (
                "javascript",
                TemplateBranch {
                    branch: "javascript-cli",
                    label: "JavaScript",
                },
            ),
            (
                "typescript",
                TemplateBranch {
                    branch: "main-cli",
                    label: "TypeScript",
                },
            ),
        ],
    },
    AppTemplate {
        key: "remix",
        url: "https://github.com/Shopify/shopify-app-template-remix",
        label: "Build a Remix app",
        visible: false,
        flavor_prompt: Some("For your Remix template, which language do you want?"),
        flavors: &[
            (
                "javascript",
                TemplateBranch {
                    branch: "javascript",
                    label: "JavaScript",
                },
            ),
            (
                "typescript",
                TemplateBranch {
                    branch: "main",
                    label: "TypeScript",
                },
            ),
        ],
    },
    AppTemplate {
        key: "none",
        url: "https://github.com/Shopify/shopify-app-template-extension-only",
        label: "Build an extension-only app",
        visible: true,
        flavor_prompt: None,
        flavors: &[],
    },
    AppTemplate {
        key: "node",
        url: "https://github.com/Shopify/shopify-app-template-node",
        label: "Node",
        visible: false,
        flavor_prompt: None,
        flavors: &[],
    },
    AppTemplate {
        key: "ruby",
        url: "https://github.com/Shopify/shopify-app-template-ruby",
        label: "Ruby",
        visible: false,
        flavor_prompt: None,
        flavors: &[],
    },
];

pub fn visible_templates() -> Vec<&'static AppTemplate> {
    TEMPLATES.iter().filter(|t| t.visible).collect()
}

pub fn lookup_template(key_or_url: &str) -> Option<&'static AppTemplate> {
    TEMPLATES.iter().find(|t| {
        t.key.eq_ignore_ascii_case(key_or_url) || t.url == key_or_url || key_or_url.contains(t.key)
    })
}

/// Resolve a GitHub URL, applying flavor branch when the template is predefined.
pub fn resolve_template_url(template: &str, flavor: Option<&str>) -> String {
    if let Some(spec) = lookup_template(template) {
        if let Some(flavor) = flavor {
            if let Some((_, branch)) = spec.flavors.iter().find(|(k, b)| {
                *k == flavor || b.branch == flavor || b.label.eq_ignore_ascii_case(flavor)
            }) {
                return format!("{}#{}", spec.url, branch.branch);
            }
        }
        return spec.url.to_string();
    }
    if let Some(flavor) = flavor {
        if !template.contains('#') {
            return format!("{template}#{flavor}");
        }
    }
    template.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visible_excludes_remix() {
        let keys: Vec<_> = visible_templates().iter().map(|t| t.key).collect();
        assert!(keys.contains(&"reactRouter"));
        assert!(keys.contains(&"none"));
        assert!(!keys.contains(&"remix"));
    }

    #[test]
    fn flavor_appends_branch() {
        let url = resolve_template_url("reactRouter", Some("javascript"));
        assert!(url.ends_with("#javascript-cli"));
    }

    #[test]
    fn custom_url_passthrough() {
        assert_eq!(
            resolve_template_url("https://github.com/org/repo", None),
            "https://github.com/org/repo"
        );
    }

    #[test]
    fn custom_url_appends_flavor() {
        assert_eq!(
            resolve_template_url("https://github.com/org/repo", Some("javascript")),
            "https://github.com/org/repo#javascript"
        );
    }

    #[test]
    fn lookup_by_url() {
        assert!(
            lookup_template("https://github.com/Shopify/shopify-app-template-react-router")
                .is_some()
        );
        assert!(lookup_template("none").is_some());
        assert!(lookup_template("missing").is_none());
    }

    #[test]
    fn remix_flavor_typescript() {
        let url = resolve_template_url("remix", Some("typescript"));
        assert!(url.ends_with("#main"));
    }
}
