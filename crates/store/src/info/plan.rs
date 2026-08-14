/// Maps a raw BP plan name to the public plan handle surfaced by `store info`.
pub fn map_plan_to_public_handle(plan_name: Option<&str>) -> Option<String> {
    let raw = plan_name?.trim();
    if raw.is_empty() {
        return None;
    }
    match raw.to_ascii_lowercase().as_str() {
        "basic" => Some("basic".into()),
        "professional" | "grow" => Some("grow".into()),
        "unlimited" | "advanced" => Some("advanced".into()),
        "shopify_plus" | "plus" => Some("plus".into()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_internal_plan_names_to_public_handles() {
        assert_eq!(map_plan_to_public_handle(Some("basic")), Some("basic".into()));
        assert_eq!(
            map_plan_to_public_handle(Some("professional")),
            Some("grow".into())
        );
        assert_eq!(
            map_plan_to_public_handle(Some("unlimited")),
            Some("advanced".into())
        );
        assert_eq!(
            map_plan_to_public_handle(Some("shopify_plus")),
            Some("plus".into())
        );
    }

    #[test]
    fn accepts_the_public_handles_themselves() {
        assert_eq!(map_plan_to_public_handle(Some("grow")), Some("grow".into()));
        assert_eq!(
            map_plan_to_public_handle(Some("advanced")),
            Some("advanced".into())
        );
        assert_eq!(map_plan_to_public_handle(Some("plus")), Some("plus".into()));
    }

    #[test]
    fn is_case_insensitive() {
        assert_eq!(
            map_plan_to_public_handle(Some("Professional")),
            Some("grow".into())
        );
        assert_eq!(
            map_plan_to_public_handle(Some("SHOPIFY_PLUS")),
            Some("plus".into())
        );
    }

    #[test]
    fn returns_undefined_for_unrecognized_plans() {
        assert!(map_plan_to_public_handle(Some("staff")).is_none());
        assert!(map_plan_to_public_handle(Some("development_legacy")).is_none());
        assert!(map_plan_to_public_handle(Some("some_new_plan")).is_none());
    }

    #[test]
    fn returns_undefined_when_no_plan_is_provided() {
        assert!(map_plan_to_public_handle(None).is_none());
        assert!(map_plan_to_public_handle(Some("")).is_none());
    }
}
