use crate::models::extensions::specification::{
    ExtensionExperience, ExtensionFeature, ExtensionSpecification,
};

pub fn theme_specification() -> ExtensionSpecification {
    ExtensionSpecification {
        identifier: "theme".into(),
        external_identifier: "theme_app_extension".into(),
        external_name: "Theme app extension".into(),
        partners_web_identifier: "theme_app_extension".into(),
        surface: "admin".into(),
        experience: ExtensionExperience::Extension,
        registration_limit: 1,
        additional_identifiers: vec!["theme_app_extension".into()],
        group: Some("Online store".into()),
        features: vec![ExtensionFeature::Theme],
    }
}
