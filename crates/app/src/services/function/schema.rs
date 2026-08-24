//! Fetch and write GraphQL schemas for function extensions.

use crate::error::AppError;
use crate::models::extensions::extension_instance::ExtensionInstance;
use crate::services::function::schema_version::prepend_schema_version_header;
use async_trait::async_trait;
use std::fs;
use std::path::PathBuf;

/// Fetches function GraphQL schema definitions from the developer platform.
#[async_trait]
pub trait SchemaDefinitionFetcher: Send + Sync {
    async fn by_api_type(
        &self,
        api_key: &str,
        version: &str,
        api_type: &str,
        org_id: &str,
    ) -> Result<Option<String>, AppError>;

    async fn by_target(
        &self,
        api_key: &str,
        version: &str,
        target: &str,
        org_id: &str,
    ) -> Result<Option<String>, AppError>;
}

#[derive(Debug, Clone)]
pub struct GenerateSchemaResult {
    pub definition: String,
    pub output_path: Option<PathBuf>,
}

/// Fetch schema for a function and optionally write `schema.graphql`.
pub async fn generate_schema_service(
    extension: &ExtensionInstance,
    api_key: &str,
    org_id: &str,
    stdout: bool,
    fetcher: &dyn SchemaDefinitionFetcher,
) -> Result<GenerateSchemaResult, AppError> {
    let version = extension.api_version().ok_or_else(|| {
        AppError::message(format!(
            "Function {} is missing api_version in its TOML",
            extension.handle
        ))
    })?;

    let targeting = extension.targeting();
    let fetched = if let Some(first) = targeting.first() {
        fetcher
            .by_target(api_key, version, &first.target, org_id)
            .await?
    } else {
        fetcher
            .by_api_type(api_key, version, extension.type_name(), org_id)
            .await?
    };

    let Some(raw) = fetched else {
        return Err(AppError::message(format!(
            "A schema could not be generated for {}. Check that the Function targets/API type and version are valid.",
            extension.local_identifier()
        )));
    };

    let definition = prepend_schema_version_header(&raw, version);

    if stdout {
        return Ok(GenerateSchemaResult {
            definition,
            output_path: None,
        });
    }

    let output_path = extension.directory.join("schema.graphql");
    fs::write(&output_path, &definition)?;
    Ok(GenerateSchemaResult {
        definition,
        output_path: Some(output_path),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::extensions::create_extension_specification;
    use crate::services::function::schema_version::SCHEMA_VERSION_MARKER_PREFIX;
    use serde_json::json;
    use std::collections::HashMap;
    use tempfile::tempdir;

    struct StubFetcher {
        definition: String,
    }

    #[async_trait]
    impl SchemaDefinitionFetcher for StubFetcher {
        async fn by_api_type(
            &self,
            _: &str,
            _: &str,
            _: &str,
            _: &str,
        ) -> Result<Option<String>, AppError> {
            Ok(Some(self.definition.clone()))
        }
        async fn by_target(
            &self,
            _: &str,
            _: &str,
            _: &str,
            _: &str,
        ) -> Result<Option<String>, AppError> {
            Ok(Some(self.definition.clone()))
        }
    }

    fn make_ext(dir: &std::path::Path, with_targeting: bool) -> ExtensionInstance {
        let mut config = HashMap::new();
        config.insert("type".into(), json!("function"));
        config.insert("api_version".into(), json!("2024-07"));
        if with_targeting {
            config.insert(
                "targeting".into(),
                json!([{ "target": "cart.transform.run" }]),
            );
        }
        let spec = create_extension_specification("function").unwrap();
        ExtensionInstance::new(
            "my-fn",
            dir.to_path_buf(),
            dir.join("shopify.extension.toml"),
            config,
            spec,
        )
    }

    #[tokio::test]
    async fn writes_schema_file() {
        let dir = tempdir().unwrap();
        let ext = make_ext(dir.path(), false);
        let fetcher = StubFetcher {
            definition: "type Query { id: ID }".into(),
        };
        let result = generate_schema_service(&ext, "key", "org", false, &fetcher)
            .await
            .unwrap();
        assert!(result.output_path.unwrap().is_file());
        let contents = fs::read_to_string(dir.path().join("schema.graphql")).unwrap();
        assert!(contents.starts_with(SCHEMA_VERSION_MARKER_PREFIX));
        assert!(contents.contains("type Query"));
    }

    #[tokio::test]
    async fn stdout_mode_skips_write() {
        let dir = tempdir().unwrap();
        let ext = make_ext(dir.path(), true);
        let fetcher = StubFetcher {
            definition: "type Query { id: ID }".into(),
        };
        let result = generate_schema_service(&ext, "key", "org", true, &fetcher)
            .await
            .unwrap();
        assert!(result.output_path.is_none());
        assert!(!dir.path().join("schema.graphql").exists());
        assert!(result.definition.contains("type Query"));
    }
}
