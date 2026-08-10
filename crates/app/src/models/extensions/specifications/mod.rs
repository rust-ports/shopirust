use crate::models::extensions::specification::{
    ExtensionExperience, ExtensionSpecification,
};

mod function;
mod theme;
mod ui_extension;

pub use function::function_specification;
pub use theme::theme_specification;
pub use ui_extension::ui_extension_specification;

pub fn lookup(identifier: &str) -> Option<ExtensionSpecification> {
    match identifier {
        "theme" | "theme_app_extension" => Some(theme_specification()),
        "ui_extension" | "checkout_ui_extension" | "customer_account_ui_extension" => {
            Some(ui_extension_specification())
        }
        "function" | "product_discounts" | "order_discounts" | "shipping_discounts" => {
            Some(function_specification())
        }
        "app_access" => Some(app_config_spec(
            "app_access",
            "App access",
            "app_access",
        )),
        "webhooks" | "app_config_webhook" => Some(app_config_spec(
            "webhooks",
            "Webhooks",
            "webhooks",
        )),
        "app_proxy" => Some(app_config_spec("app_proxy", "App proxy", "app_proxy")),
        "pos" | "point_of_sale" => Some(app_config_spec("pos", "Point of Sale", "point_of_sale")),
        "branding" => Some(app_config_spec("branding", "Branding", "branding")),
        "privacy_compliance_webhooks" => Some(app_config_spec(
            "privacy_compliance_webhooks",
            "Privacy compliance webhooks",
            "privacy_compliance_webhooks",
        )),
        other => Some(ExtensionSpecification {
            identifier: other.to_string(),
            external_identifier: other.to_string(),
            external_name: other.replace('_', " "),
            partners_web_identifier: other.to_string(),
            surface: "admin".into(),
            experience: ExtensionExperience::Extension,
            registration_limit: 10,
            additional_identifiers: vec![],
            group: None,
            features: vec![],
        }),
    }
}

fn app_config_spec(id: &str, name: &str, partners: &str) -> ExtensionSpecification {
    ExtensionSpecification {
        identifier: id.into(),
        external_identifier: id.into(),
        external_name: name.into(),
        partners_web_identifier: partners.into(),
        surface: "app_config".into(),
        experience: ExtensionExperience::Configuration,
        registration_limit: 1,
        additional_identifiers: vec![],
        group: Some("Configuration".into()),
        features: vec![],
    }
}

pub fn all_known_specifications() -> Vec<ExtensionSpecification> {
    vec![
        theme_specification(),
        ui_extension_specification(),
        function_specification(),
        lookup("app_access").unwrap(),
        lookup("webhooks").unwrap(),
        lookup("app_proxy").unwrap(),
        lookup("pos").unwrap(),
        lookup("branding").unwrap(),
    ]
}
