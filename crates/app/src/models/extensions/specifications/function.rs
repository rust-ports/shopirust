use crate::models::extensions::specification::{
    ExtensionExperience, ExtensionFeature, ExtensionSpecification,
};

pub fn function_specification() -> ExtensionSpecification {
    ExtensionSpecification {
        identifier: "function".into(),
        external_identifier: "function".into(),
        external_name: "Function".into(),
        partners_web_identifier: "function".into(),
        surface: "admin".into(),
        experience: ExtensionExperience::Extension,
        registration_limit: 50,
        additional_identifiers: vec![
            "product_discounts".into(),
            "order_discounts".into(),
            "shipping_discounts".into(),
        ],
        group: Some("Functions".into()),
        features: vec![ExtensionFeature::Function],
    }
}
