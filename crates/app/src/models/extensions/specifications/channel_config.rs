use super::ext_spec;
use crate::models::extensions::specification::{ExtensionSpecification, UidStrategy};

pub fn channel_config_specification() -> ExtensionSpecification {
    let mut spec = ext_spec(
        "channel_config",
        None,
        vec![],
        &[],
        1,
        Some("Channels"),
        "admin",
    );
    spec.uid_strategy = UidStrategy::Single;
    spec
}

pub fn order_attribution_config_specification() -> ExtensionSpecification {
    let mut spec = ext_spec(
        "order_attribution_config",
        None,
        vec![],
        &[],
        1,
        None,
        "admin",
    );
    spec.uid_strategy = UidStrategy::Single;
    spec
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_uid() {
        assert_eq!(
            channel_config_specification().uid_strategy,
            UidStrategy::Single
        );
        assert_eq!(
            order_attribution_config_specification().identifier,
            "order_attribution_config"
        );
    }
}
