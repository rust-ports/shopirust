use crate::models::extensions::specification::{
    ExtensionExperience, ExtensionFeature, ExtensionSpecification, UidStrategy,
};

/// Function API type aliases (upstream `additionalIdentifiers`).
pub const FUNCTION_ALIASES: &[&str] = &[
    "order_discounts",
    "cart_checkout_validation",
    "cart_transform",
    "delivery_customization",
    "payment_customization",
    "product_discounts",
    "shipping_discounts",
    "fulfillment_constraints",
    "order_routing_location_rule",
    "local_pickup_delivery_option_generator",
    "pickup_point_delivery_option_generator",
];

pub fn function_specification() -> ExtensionSpecification {
    ExtensionSpecification {
        identifier: "function".into(),
        external_identifier: "function_external".into(),
        external_name: "Function".into(),
        partners_web_identifier: "function".into(),
        surface: "admin".into(),
        experience: ExtensionExperience::Extension,
        registration_limit: 50,
        additional_identifiers: FUNCTION_ALIASES
            .iter()
            .map(|s| (*s).to_string())
            .collect(),
        group: Some("Functions".into()),
        features: vec![ExtensionFeature::Function],
        uid_strategy: UidStrategy::Uuid,
        graph_ql_type: None,
        dependency: None,
    }
}
