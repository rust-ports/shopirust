//! Admin GraphQL lookup of a published product variant for checkout cart URLs.

use crate::error::AppError;
use serde::Deserialize;

const FIND_PRODUCT_VARIANT: &str = r#"
query FindProductVariant {
  products(first: 1, query: "published_status:published") {
    edges {
      node {
        id
        variants(first: 1) {
          edges {
            node { id }
          }
        }
      }
    }
  }
}
"#;

#[derive(Debug, Deserialize)]
struct ProductVariantResponse {
    data: Option<ProductVariantData>,
    errors: Option<Vec<GraphqlError>>,
}

#[derive(Debug, Deserialize)]
struct GraphqlError {
    message: String,
}

#[derive(Debug, Deserialize)]
struct ProductVariantData {
    products: ProductConnection,
}

#[derive(Debug, Deserialize)]
struct ProductConnection {
    edges: Vec<ProductEdge>,
}

#[derive(Debug, Deserialize)]
struct ProductEdge {
    node: ProductNode,
}

#[derive(Debug, Deserialize)]
struct ProductNode {
    variants: VariantConnection,
}

#[derive(Debug, Deserialize)]
struct VariantConnection {
    edges: Vec<VariantEdge>,
}

#[derive(Debug, Deserialize)]
struct VariantEdge {
    node: VariantNode,
}

#[derive(Debug, Deserialize)]
struct VariantNode {
    id: String,
}

/// Fetch the first published product variant numeric ID for `store_fqdn`.
pub async fn fetch_product_variant(
    admin_graphql_url: &str,
    admin_token: &str,
    store_fqdn: &str,
) -> Result<String, AppError> {
    let client = reqwest::Client::new();
    let resp = client
        .post(admin_graphql_url)
        .header("X-Shopify-Access-Token", admin_token)
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({ "query": FIND_PRODUCT_VARIANT }))
        .send()
        .await
        .map_err(|e| AppError::message(e.to_string()))?;
    let parsed: ProductVariantResponse = resp
        .json()
        .await
        .map_err(|e| AppError::message(e.to_string()))?;
    if let Some(errors) = parsed.errors {
        if !errors.is_empty() {
            return Err(AppError::message(
                errors
                    .into_iter()
                    .map(|e| e.message)
                    .collect::<Vec<_>>()
                    .join(", "),
            ));
        }
    }
    let variant_gid = parsed
        .data
        .and_then(|d| d.products.edges.into_iter().next())
        .and_then(|e| e.node.variants.edges.into_iter().next())
        .map(|e| e.node.id)
        .ok_or_else(|| {
            AppError::message(format!(
                "Could not find a product variant on {store_fqdn}. Add a published product in the store admin."
            ))
        })?;
    Ok(variant_gid
        .rsplit('/')
        .next()
        .unwrap_or(&variant_gid)
        .to_string())
}

/// Build a checkout cart URL (`cart/{variant}:1`) when checkout UI extensions need one.
pub async fn build_cart_url_if_needed(
    needs_cart: bool,
    checkout_cart_url: Option<String>,
    admin_graphql_url: Option<&str>,
    admin_token: Option<&str>,
    store_fqdn: &str,
) -> Result<Option<String>, AppError> {
    if !needs_cart {
        return Ok(None);
    }
    if let Some(url) = checkout_cart_url {
        return Ok(Some(url));
    }
    match (admin_graphql_url, admin_token) {
        (Some(url), Some(token)) if !url.is_empty() && !token.is_empty() => {
            let id = fetch_product_variant(url, token, store_fqdn).await?;
            Ok(Some(format!("cart/{id}:1")))
        }
        _ => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn skips_when_not_needed() {
        let url = build_cart_url_if_needed(false, Some("cart/1:1".into()), None, None, "s")
            .await
            .unwrap();
        assert!(url.is_none());
    }

    #[tokio::test]
    async fn uses_flag_when_provided() {
        let url = build_cart_url_if_needed(
            true,
            Some("cart/99:1".into()),
            None,
            None,
            "shop.myshopify.com",
        )
        .await
        .unwrap();
        assert_eq!(url.as_deref(), Some("cart/99:1"));
    }

    #[tokio::test]
    async fn returns_none_without_admin_session() {
        let url = build_cart_url_if_needed(true, None, None, None, "shop.myshopify.com")
            .await
            .unwrap();
        assert!(url.is_none());
    }
}
