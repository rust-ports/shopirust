/// Public store-type handle for every BP `Store` enum member.
pub fn store_type_handle(store_type: Option<&str>) -> Option<String> {
    let raw = store_type?.trim();
    if raw.is_empty() {
        return None;
    }
    match raw {
        "APP_DEVELOPMENT" | "DEVELOPMENT" | "DEVELOPMENT_SUPERSET" => Some("dev".into()),
        "CLIENT_TRANSFER" => Some("client_transfer".into()),
        "COLLABORATOR" => Some("collaborator".into()),
        "PRODUCTION" => Some("production".into()),
        _ => None,
    }
}

pub fn store_type_label(handle: Option<&str>) -> String {
    handle.map(capitalize_words).unwrap_or_default()
}

pub fn capitalize_words(input: &str) -> String {
    input
        .split(['_', '-', ' '])
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut chars = w.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => {
                    first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase()
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_known_types() {
        assert_eq!(
            store_type_handle(Some("APP_DEVELOPMENT")),
            Some("dev".into())
        );
        assert_eq!(store_type_handle(Some("DEVELOPMENT")), Some("dev".into()));
        assert_eq!(
            store_type_handle(Some("DEVELOPMENT_SUPERSET")),
            Some("dev".into())
        );
        assert_eq!(
            store_type_handle(Some("PRODUCTION")),
            Some("production".into())
        );
        assert_eq!(
            store_type_handle(Some("CLIENT_TRANSFER")),
            Some("client_transfer".into())
        );
        assert_eq!(
            store_type_handle(Some("COLLABORATOR")),
            Some("collaborator".into())
        );
    }

    #[test]
    fn unknown_and_empty_omitted() {
        assert!(store_type_handle(None).is_none());
        assert!(store_type_handle(Some("")).is_none());
        assert!(store_type_handle(Some("FUTURE_TYPE")).is_none());
    }

    #[test]
    fn labels() {
        assert_eq!(store_type_label(Some("dev")), "Dev");
        assert_eq!(store_type_label(Some("client_transfer")), "Client Transfer");
        assert_eq!(store_type_label(None), "");
    }
}
