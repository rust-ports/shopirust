use crate::models::extensions::specification::{
    ExtensionExperience, ExtensionFeature, ExtensionSpecification, UidStrategy,
};

pub fn ui_extension_specification() -> ExtensionSpecification {
    ExtensionSpecification {
        identifier: "ui_extension".into(),
        external_identifier: "ui_extension_external".into(),
        external_name: "UI extension".into(),
        partners_web_identifier: "ui_extension".into(),
        surface: "checkout".into(),
        experience: ExtensionExperience::Extension,
        registration_limit: 50,
        additional_identifiers: vec![],
        group: Some("Checkout".into()),
        features: vec![
            ExtensionFeature::UiPreview,
            ExtensionFeature::Esbuild,
            ExtensionFeature::GeneratesSourceMaps,
            ExtensionFeature::CartUrl,
            ExtensionFeature::SingleJsEntryPath,
            ExtensionFeature::Localization,
        ],
        uid_strategy: UidStrategy::Uuid,
        graph_ql_type: None,
        dependency: Some("@shopify/checkout-ui-extensions".into()),
    }
}
