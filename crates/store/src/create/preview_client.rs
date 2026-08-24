use crate::error::StoreError;
use serde::Deserialize;

pub const CLI_INSTANCE_HEADER: &str = "X-Shopify-CLI-Instance";
pub const CLI_VERSION_HEADER: &str = "X-Shopify-CLI-Version";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewStoreClaimResponse {
    pub claim_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewStoreGetResponse {
    pub shop_id: String,
    pub shop_name: String,
    pub shop_domain: String,
    pub access_url: String,
}

#[derive(Debug, Deserialize)]
struct RawClaim {
    claim_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawGet {
    shop: Option<RawShop>,
    access_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawShop {
    id: Option<String>,
    name: Option<String>,
    domain: Option<String>,
}

fn preview_headers(
    cli_instance_id: &str,
    cli_version: &str,
    admin_api_token: &str,
) -> reqwest::header::HeaderMap {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(reqwest::header::ACCEPT, "application/json".parse().unwrap());
    headers.insert(
        reqwest::header::CONTENT_TYPE,
        "application/json".parse().unwrap(),
    );
    headers.insert(
        reqwest::header::USER_AGENT,
        format!("Shopify CLI; v={cli_version}").parse().unwrap(),
    );
    headers.insert(CLI_INSTANCE_HEADER, cli_instance_id.parse().unwrap());
    headers.insert(CLI_VERSION_HEADER, cli_version.parse().unwrap());
    headers.insert(
        reqwest::header::AUTHORIZATION,
        admin_api_token.parse().unwrap(),
    );
    headers.insert("X-Shopify-Access-Token", admin_api_token.parse().unwrap());
    headers
}

pub async fn claim_preview_store(
    http: &reqwest::Client,
    app_management_fqdn: &str,
    shop_id: &str,
    admin_api_token: &str,
    cli_instance_id: &str,
    cli_version: &str,
) -> Result<PreviewStoreClaimResponse, StoreError> {
    let url = format!(
        "https://{app_management_fqdn}/services/preview-stores/{}/claim",
        urlencoding_encode(shop_id)
    );
    let response = http
        .post(&url)
        .headers(preview_headers(
            cli_instance_id,
            cli_version,
            admin_api_token,
        ))
        .json(&serde_json::json!({}))
        .send()
        .await
        .map_err(|e| StoreError::message(e.to_string()))?;
    let status = response.status().as_u16();
    let raw = response
        .text()
        .await
        .map_err(|e| StoreError::message(e.to_string()))?;
    if !(200..300).contains(&status) {
        return Err(StoreError::http(
            status,
            format!("Preview store claim failed with HTTP {status}."),
        ));
    }
    let parsed: RawClaim =
        serde_json::from_str(&raw).map_err(|e| StoreError::message(e.to_string()))?;
    let claim_url = parsed
        .claim_url
        .filter(|s| !s.is_empty())
        .ok_or_else(|| StoreError::message("Preview store claim response omitted claim_url."))?;
    Ok(PreviewStoreClaimResponse { claim_url })
}

pub async fn get_preview_store(
    http: &reqwest::Client,
    app_management_fqdn: &str,
    shop_id: &str,
    admin_api_token: &str,
    cli_instance_id: &str,
    cli_version: &str,
) -> Result<PreviewStoreGetResponse, StoreError> {
    let url = format!("https://{app_management_fqdn}/services/preview-stores/{shop_id}");
    let response = http
        .get(&url)
        .headers(preview_headers(
            cli_instance_id,
            cli_version,
            admin_api_token,
        ))
        .send()
        .await
        .map_err(|e| StoreError::message(e.to_string()))?;
    let status = response.status().as_u16();
    let raw = response
        .text()
        .await
        .map_err(|e| StoreError::message(e.to_string()))?;
    if !(200..300).contains(&status) {
        return Err(StoreError::http(
            status,
            format!("Preview store lookup failed with HTTP {status}."),
        ));
    }
    let parsed: RawGet =
        serde_json::from_str(&raw).map_err(|e| StoreError::message(e.to_string()))?;
    let shop = parsed
        .shop
        .ok_or_else(|| StoreError::message("Preview store lookup omitted shop."))?;
    let access_url = parsed
        .access_url
        .filter(|s| !s.is_empty())
        .ok_or_else(|| StoreError::message("Preview store lookup omitted access_url."))?;
    Ok(PreviewStoreGetResponse {
        shop_id: shop.id.unwrap_or_default(),
        shop_name: shop.name.unwrap_or_default(),
        shop_domain: shop.domain.unwrap_or_default(),
        access_url,
    })
}

fn urlencoding_encode(s: &str) -> String {
    crate::url::encode_uri_component(s)
}
