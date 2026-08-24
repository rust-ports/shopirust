use super::ext_spec;
use crate::models::extensions::specification::ExtensionSpecification;

pub fn flow_trigger_specification() -> ExtensionSpecification {
    ext_spec("flow_trigger", None, vec![], &[], 50, Some("Flow"), "admin")
}

pub fn flow_template_specification() -> ExtensionSpecification {
    ext_spec(
        "flow_template",
        None,
        vec![crate::models::extensions::specification::ExtensionFeature::UiPreview],
        &[],
        50,
        Some("Flow"),
        "admin",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flow_specs() {
        assert_eq!(flow_trigger_specification().identifier, "flow_trigger");
        assert_eq!(flow_template_specification().identifier, "flow_template");
    }
}
