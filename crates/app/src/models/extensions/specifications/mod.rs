//! Local extension specification registry (mirrors upstream `load-specifications.ts`).

use crate::models::extensions::specification::{
    ExtensionExperience, ExtensionFeature, ExtensionSpecification, UidStrategy,
};
use std::sync::OnceLock;

mod function;
mod theme;
mod ui_extension;

pub use function::{function_specification, FUNCTION_ALIASES};
pub use theme::theme_specification;
pub use ui_extension::ui_extension_specification;

fn capitalize_identifier(id: &str) -> String {
    id.replace('_', " ")
        .split_whitespace()
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn ext_spec(
    identifier: &str,
    partners_web: Option<&str>,
    features: Vec<ExtensionFeature>,
    additional: &[&str],
    registration_limit: usize,
    group: Option<&str>,
    surface: &str,
) -> ExtensionSpecification {
    ExtensionSpecification {
        identifier: identifier.into(),
        external_identifier: format!("{identifier}_external"),
        external_name: capitalize_identifier(identifier),
        partners_web_identifier: partners_web.unwrap_or(identifier).into(),
        surface: surface.into(),
        experience: ExtensionExperience::Extension,
        registration_limit,
        additional_identifiers: additional.iter().map(|s| (*s).to_string()).collect(),
        group: group.map(str::to_string),
        features,
        uid_strategy: UidStrategy::Uuid,
        graph_ql_type: None,
        dependency: None,
    }
}

fn config_spec(identifier: &str) -> ExtensionSpecification {
    ExtensionSpecification {
        identifier: identifier.into(),
        external_identifier: format!("{identifier}_external"),
        external_name: capitalize_identifier(identifier),
        partners_web_identifier: identifier.into(),
        surface: "app_config".into(),
        experience: ExtensionExperience::Configuration,
        registration_limit: 1,
        additional_identifiers: vec![],
        group: Some("Configuration".into()),
        features: vec![],
        uid_strategy: UidStrategy::Single,
        graph_ql_type: None,
        dependency: None,
    }
}

/// Ordered config-module identifiers (upstream loader sort).
pub const CONFIG_SPEC_ORDER: &[&str] = &[
    "branding",
    "app_access",
    "webhooks",
    "webhook_subscription",
    "events",
    "privacy_compliance_webhooks",
    "app_proxy",
    "point_of_sale",
    "app_home",
];

/// Keys that are never treated as app-config modules.
pub const APP_SCHEMA_KEYS: &[&str] = &[
    "client_id",
    "name",
    "handle",
    "application_url",
    "embedded",
    "build",
    "extension_directories",
    "web_directories",
    "access_scopes",
    "auth",
    "organization_id",
    "path_prefix",
];

fn all_specs() -> &'static [ExtensionSpecification] {
    static SPECS: OnceLock<Vec<ExtensionSpecification>> = OnceLock::new();
    SPECS.get_or_init(|| {
        let mut webhook_sub = config_spec("webhook_subscription");
        webhook_sub.uid_strategy = UidStrategy::Dynamic;

        let mut channel = ext_spec(
            "channel_config",
            None,
            vec![],
            &[],
            1,
            Some("Channels"),
            "admin",
        );
        channel.uid_strategy = UidStrategy::Single;

        let mut order_attr = ext_spec(
            "order_attribution_config",
            None,
            vec![],
            &[],
            1,
            None,
            "admin",
        );
        order_attr.uid_strategy = UidStrategy::Single;

        let mut product_sub = ext_spec(
            "product_subscription",
            None,
            vec![
                ExtensionFeature::UiPreview,
                ExtensionFeature::Esbuild,
                ExtensionFeature::SingleJsEntryPath,
            ],
            &["subscription_management"],
            1,
            Some("Checkout"),
            "admin",
        );
        product_sub.graph_ql_type = Some("subscription_management".into());
        product_sub.dependency = Some("@shopify/admin-ui-extensions".into());

        let mut checkout_ui = ext_spec(
            "checkout_ui_extension",
            None,
            vec![
                ExtensionFeature::UiPreview,
                ExtensionFeature::CartUrl,
                ExtensionFeature::Esbuild,
                ExtensionFeature::SingleJsEntryPath,
                ExtensionFeature::GeneratesSourceMaps,
            ],
            &[],
            50,
            Some("Checkout"),
            "checkout",
        );
        checkout_ui.dependency = Some("@shopify/checkout-ui-extensions".into());

        let mut post_purchase = ext_spec(
            "checkout_post_purchase",
            Some("post_purchase"),
            vec![
                ExtensionFeature::UiPreview,
                ExtensionFeature::CartUrl,
                ExtensionFeature::Esbuild,
                ExtensionFeature::SingleJsEntryPath,
            ],
            &[],
            1,
            Some("Checkout"),
            "post_purchase",
        );
        post_purchase.dependency = Some("@shopify/post-purchase-ui-extensions".into());

        let mut pos_ui = ext_spec(
            "pos_ui_extension",
            None,
            vec![
                ExtensionFeature::UiPreview,
                ExtensionFeature::Esbuild,
                ExtensionFeature::SingleJsEntryPath,
            ],
            &[],
            50,
            Some("Point of Sale"),
            "pos",
        );
        pos_ui.dependency = Some("@shopify/retail-ui-extensions".into());

        let mut web_pixel = ext_spec(
            "web_pixel_extension",
            Some("web_pixel"),
            vec![
                ExtensionFeature::Esbuild,
                ExtensionFeature::SingleJsEntryPath,
            ],
            &[],
            1,
            Some("Analytics"),
            "customer_accounts",
        );
        web_pixel.dependency = Some("@shopify/web-pixels-extension".into());

        vec![
            config_spec("app_access"),
            config_spec("app_home"),
            config_spec("app_proxy"),
            config_spec("branding"),
            config_spec("events"),
            config_spec("point_of_sale"),
            config_spec("privacy_compliance_webhooks"),
            config_spec("webhooks"),
            webhook_sub,
            post_purchase,
            checkout_ui,
            ext_spec(
                "editor_extension_collection",
                None,
                vec![],
                &[],
                50,
                None,
                "admin",
            ),
            ext_spec("flow_action", None, vec![], &[], 50, Some("Flow"), "admin"),
            ext_spec(
                "flow_template",
                None,
                vec![ExtensionFeature::UiPreview],
                &[],
                50,
                Some("Flow"),
                "admin",
            ),
            ext_spec("flow_trigger", None, vec![], &[], 50, Some("Flow"), "admin"),
            function_specification(),
            ext_spec(
                "payments_extension",
                None,
                vec![],
                &[],
                50,
                Some("Payments"),
                "admin",
            ),
            pos_ui,
            product_sub,
            ext_spec(
                "tax_calculation",
                None,
                vec![],
                &[],
                1,
                Some("Checkout"),
                "admin",
            ),
            theme_specification(),
            ui_extension_specification(),
            web_pixel,
            ext_spec(
                "admin_link",
                None,
                vec![ExtensionFeature::Localization, ExtensionFeature::UiPreview],
                &[],
                50,
                Some("Admin"),
                "admin",
            ),
            channel,
            order_attr,
        ]
    })
}

/// Look up a local specification by identifier, external id, or alias.
pub fn lookup(identifier: &str) -> Option<ExtensionSpecification> {
    let normalized = match identifier {
        "theme_app_extension" => "theme",
        "subscription_management" => "product_subscription",
        "pos" => "point_of_sale",
        "app_config_webhook" => "webhooks",
        other => other,
    };

    all_specs()
        .iter()
        .find(|s| s.matches_type(normalized))
        .cloned()
}

pub fn all_known_specifications() -> Vec<ExtensionSpecification> {
    all_specs().to_vec()
}

pub fn is_config_specification(identifier: &str) -> bool {
    lookup(identifier)
        .map(|s| s.is_app_config())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::extensions::specification::ExtensionFeature;

    #[test]
    fn registry_covers_all_local_identifiers() {
        let ids = [
            "app_access",
            "app_home",
            "app_proxy",
            "branding",
            "events",
            "point_of_sale",
            "privacy_compliance_webhooks",
            "webhooks",
            "webhook_subscription",
            "checkout_post_purchase",
            "checkout_ui_extension",
            "editor_extension_collection",
            "flow_action",
            "flow_template",
            "flow_trigger",
            "function",
            "payments_extension",
            "pos_ui_extension",
            "product_subscription",
            "tax_calculation",
            "theme",
            "ui_extension",
            "web_pixel_extension",
            "admin_link",
            "channel_config",
            "order_attribution_config",
        ];
        assert_eq!(all_known_specifications().len(), ids.len());
        for id in ids {
            assert!(lookup(id).is_some(), "missing spec {id}");
        }
    }

    #[test]
    fn function_aliases_resolve() {
        for alias in FUNCTION_ALIASES {
            let spec = lookup(alias).expect(alias);
            assert_eq!(spec.identifier, "function");
            assert!(spec.features.contains(&ExtensionFeature::Function));
        }
    }

    #[test]
    fn theme_api_alias_resolves() {
        let spec = lookup("theme_app_extension").unwrap();
        assert_eq!(spec.identifier, "theme");
        assert_eq!(spec.partners_web_identifier, "theme_app_extension");
    }

    #[test]
    fn product_subscription_alias() {
        let spec = lookup("subscription_management").unwrap();
        assert_eq!(spec.identifier, "product_subscription");
    }

    #[test]
    fn config_specs_are_configuration_experience() {
        for id in CONFIG_SPEC_ORDER {
            let spec = lookup(id).unwrap();
            assert!(spec.is_app_config(), "{id}");
        }
    }
}
