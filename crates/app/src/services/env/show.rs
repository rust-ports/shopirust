use super::pull::EnvValues;
use crate::error::AppError;
use crate::models::loader::LoadedApp;
use cli_api::OrganizationApp;
use serde_json::json;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvFormat {
    Text,
    Json,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShowEnvResult {
    pub output: String,
}

pub fn show_env(
    app: &LoadedApp,
    remote_app: &OrganizationApp,
    format: EnvFormat,
) -> Result<ShowEnvResult, AppError> {
    let values = EnvValues::from_apps(app, remote_app);
    let output = match format {
        EnvFormat::Json => format_env_json(&values),
        EnvFormat::Text => format_env_text(&values),
    };
    Ok(ShowEnvResult { output })
}

pub fn format_env_text(values: &EnvValues) -> String {
    let secret = values.shopify_api_secret.as_deref().unwrap_or("");
    format!(
        "SHOPIFY_API_KEY={}\nSHOPIFY_API_SECRET={}\nSCOPES={}",
        values.shopify_api_key, secret, values.scopes
    )
}

pub fn format_env_json(values: &EnvValues) -> String {
    serde_json::to_string_pretty(&json!({
        "SHOPIFY_API_KEY": values.shopify_api_key,
        "SHOPIFY_API_SECRET": values.shopify_api_secret,
        "SCOPES": values.scopes,
    }))
    .unwrap_or_else(|_| "{}".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_and_json_formatting() {
        let values = EnvValues {
            shopify_api_key: "key".into(),
            shopify_api_secret: Some("secret".into()),
            scopes: "read_products".into(),
        };
        assert_eq!(
            format_env_text(&values),
            "SHOPIFY_API_KEY=key\nSHOPIFY_API_SECRET=secret\nSCOPES=read_products"
        );
        let json = format_env_json(&values);
        assert!(json.contains("\"SHOPIFY_API_KEY\": \"key\""));
        assert!(json.contains("\"SCOPES\": \"read_products\""));
    }

    #[test]
    fn missing_secret_prints_empty() {
        let values = EnvValues {
            shopify_api_key: "key".into(),
            shopify_api_secret: None,
            scopes: String::new(),
        };
        assert_eq!(
            format_env_text(&values),
            "SHOPIFY_API_KEY=key\nSHOPIFY_API_SECRET=\nSCOPES="
        );
    }
}
