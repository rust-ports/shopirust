use crate::models::extensions::specification::{
    ExtensionExperience, ExtensionFeature, ExtensionSpecification,
};

pub fn ui_extension_specification() -> ExtensionSpecification {
    ExtensionSpecification {
        identifier: "ui_extension".into(),
        external_identifier: "ui_extension".into(),
        external_name: "UI extension".into(),
        partners_web_identifier: "ui_extension".into(),
        surface: "checkout".into(),
        experience: ExtensionExperience::Extension,
        registration_limit: 50,
        additional_identifiers: vec![
            "checkout_ui_extension".into(),
            "customer_account_ui_extension".into(),
        ],
        group: Some("Checkout".into()),
        features: vec![
            ExtensionFeature::UiPreview,
            ExtensionFeature::Esbuild,
            ExtensionFeature::SingleJsEntryPath,
            ExtensionFeature::Localization,
            ExtensionFeature::GeneratesSourceMaps,
        ],
    }
}
