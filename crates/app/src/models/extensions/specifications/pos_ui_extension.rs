use super::ext_spec;
use crate::models::extensions::specification::{ExtensionFeature, ExtensionSpecification};

pub fn pos_ui_extension_specification() -> ExtensionSpecification {
    let mut spec = ext_spec(
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
    spec.dependency = Some("@shopify/retail-ui-extensions".into());
    spec
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifier() {
        let spec = pos_ui_extension_specification();
        assert_eq!(spec.identifier, "pos_ui_extension");
        assert!(spec.features.contains(&ExtensionFeature::Esbuild));
    }
}
