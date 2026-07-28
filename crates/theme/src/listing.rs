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
}
