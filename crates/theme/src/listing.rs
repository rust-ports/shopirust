use crate::filesystem::ThemeAsset;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum ListingError {
    #[error("The listing '{name}' doesn't exist. Available listings: {available}")]
    NotFound { name: String, available: String },
    #[error("No theme listings are available in {0}")]
    NoPresets(PathBuf),
    #[error("Unable to read theme listings: {0}")]
    Io(#[from] std::io::Error),
}

pub fn display_case(value: &str) -> String {
    value
        .split(['-', '_', ' '])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            chars
                .next()
                .map(|first| first.to_uppercase().collect::<String>() + chars.as_str())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn available_listings(root: impl AsRef<Path>) -> Result<Vec<String>, ListingError> {
    let directory = root.as_ref().join("listings");
    if !directory.is_dir() {
        return Ok(Vec::new());
    }
    let mut names = fs::read_dir(directory)?
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect::<Vec<_>>();
    names.sort();
    Ok(names)
}

pub fn validate_listing(root: impl AsRef<Path>, name: &str) -> Result<PathBuf, ListingError> {
    let listings = available_listings(&root)?;
    if listings.is_empty() {
        return Err(ListingError::NoPresets(root.as_ref().join("listings")));
    }
    listings
        .iter()
        .find(|item| item.eq_ignore_ascii_case(name))
        .map(|item| root.as_ref().join("listings").join(item))
        .ok_or_else(|| ListingError::NotFound {
            name: name.into(),
            available: listings
                .iter()
                .map(|item| display_case(item))
                .collect::<Vec<_>>()
                .join(", "),
        })
}

pub fn apply_listing(
    root: impl AsRef<Path>,
    name: &str,
    assets: &mut BTreeMap<String, ThemeAsset>,
) -> Result<(), ListingError> {
    let directory = validate_listing(&root, name)?;
    for prefix in ["templates", "sections"] {
        let source = directory.join(prefix);
        if source.is_dir() {
            overlay_json(&directory, &source, assets)?;
        }
    }
    if let Some(settings) = assets.get_mut("config/settings_data.json") {
        if let Some(value) = settings.value.as_mut() {
            if let Ok(mut json) = serde_json::from_str::<serde_json::Value>(value) {
                if let Some(current) = json
                    .as_object_mut()
                    .and_then(|object| object.get_mut("current"))
                {
                    *current = serde_json::Value::String(display_case(name));
                }
                if let Ok(serialized) = serde_json::to_string_pretty(&json) {
                    *value = serialized;
                    settings.checksum = crate::checksum::calculate_checksum(
                        &settings.key,
                        Some(value.clone().into()),
                    );
                }
            }
        }
    }
    Ok(())
}

fn overlay_json(
    base: &Path,
    directory: &Path,
    assets: &mut BTreeMap<String, ThemeAsset>,
) -> Result<(), std::io::Error> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            overlay_json(base, &path, assets)?;
            continue;
        }
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let key = path
            .strip_prefix(base)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        let value = fs::read_to_string(&path)?;
        assets.insert(
            key.clone(),
            ThemeAsset {
                checksum: crate::checksum::calculate_checksum(&key, Some(value.clone().into())),
                key,
                value: Some(value),
                attachment: None,
                stats: None,
            },
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_listing_names() {
        assert_eq!(display_case("summer-sale_2026"), "Summer Sale 2026");
    }

    #[test]
    fn available_listings_returns_empty_when_listings_directory_missing() {
        let temp = tempfile::tempdir().unwrap();
        let listings = available_listings(temp.path()).unwrap();
        assert!(listings.is_empty());
    }

    #[test]
    fn available_listings_returns_sorted_directory_names() {
        let temp = tempfile::tempdir().unwrap();
        let listings_dir = temp.path().join("listings");
        fs::create_dir_all(listings_dir.join("zebra")).unwrap();
        fs::create_dir_all(listings_dir.join("alpha")).unwrap();
        fs::create_dir_all(listings_dir.join("beta")).unwrap();

        let listings = available_listings(temp.path()).unwrap();
        assert_eq!(listings, vec!["alpha", "beta", "zebra"]);
    }

    #[test]
    fn validate_listing_succeeds_for_existing_listing() {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir_all(temp.path().join("listings").join("modern")).unwrap();

        let path = validate_listing(temp.path(), "modern").unwrap();
        assert!(path.ends_with("listings/modern"));
    }

    #[test]
    fn validate_listing_is_case_insensitive() {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir_all(temp.path().join("listings").join("modern")).unwrap();

        let path = validate_listing(temp.path(), "MODERN").unwrap();
        assert!(path.ends_with("listings/modern"));
    }

    #[test]
    fn validate_listing_fails_with_not_found_error() {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir_all(temp.path().join("listings").join("modern")).unwrap();

        let result = validate_listing(temp.path(), "classic");
        assert!(matches!(result, Err(ListingError::NotFound { .. })));
    }

    #[test]
    fn validate_listing_fails_with_no_presets_when_directory_missing() {
        let temp = tempfile::tempdir().unwrap();

        let result = validate_listing(temp.path(), "modern");
        assert!(matches!(result, Err(ListingError::NoPresets(_))));
    }

    #[test]
    fn apply_listing_overlays_template_and_section_json() {
        let temp = tempfile::tempdir().unwrap();
        let listing_dir = temp.path().join("listings").join("modern");
        fs::create_dir_all(listing_dir.join("templates")).unwrap();
        fs::create_dir_all(listing_dir.join("sections")).unwrap();
        fs::write(
            listing_dir.join("templates").join("index.json"),
            r#"{"sections": {"main": {"type": "main"}}}"#,
        )
        .unwrap();
        fs::write(
            listing_dir.join("sections").join("header.json"),
            r#"{"name": "header"}"#,
        )
        .unwrap();

        let mut assets = BTreeMap::new();
        assets.insert(
            "config/settings_data.json".into(),
            ThemeAsset {
                key: "config/settings_data.json".into(),
                checksum: String::new(),
                attachment: None,
                value: Some(r#"{"current":"Default"}"#.into()),
                stats: None,
            },
        );

        apply_listing(temp.path(), "modern", &mut assets).unwrap();

        assert!(assets.contains_key("templates/index.json"));
        assert!(assets.contains_key("sections/header.json"));
        assert_eq!(
            assets
                .get("config/settings_data.json")
                .unwrap()
                .value
                .as_deref(),
            Some("{\n  \"current\": \"Modern\"\n}")
        );
    }

    #[test]
    fn apply_listing_does_not_overlay_non_json_files() {
        let temp = tempfile::tempdir().unwrap();
        let listing_dir = temp.path().join("listings").join("modern");
        fs::create_dir_all(listing_dir.join("sections")).unwrap();
        fs::write(listing_dir.join("sections").join("header.liquid"), "html").unwrap();

        let mut assets = BTreeMap::new();
        apply_listing(temp.path(), "modern", &mut assets).unwrap();

        assert!(!assets.contains_key("sections/header.liquid"));
    }

    #[test]
    fn apply_listing_preserves_settings_data_when_json_is_malformed() {
        let temp = tempfile::tempdir().unwrap();
        let listing_dir = temp.path().join("listings").join("modern");
        fs::create_dir_all(listing_dir.join("templates")).unwrap();
        fs::write(listing_dir.join("templates").join("index.json"), "{}").unwrap();

        let mut assets = BTreeMap::new();
        assets.insert(
            "config/settings_data.json".into(),
            ThemeAsset {
                key: "config/settings_data.json".into(),
                checksum: String::new(),
                attachment: None,
                value: Some("not valid json".into()),
                stats: None,
            },
        );

        apply_listing(temp.path(), "modern", &mut assets).unwrap();
        assert_eq!(
            assets
                .get("config/settings_data.json")
                .unwrap()
                .value
                .as_deref(),
            Some("not valid json")
        );
    }
}
