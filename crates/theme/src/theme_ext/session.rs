//! Storefront session bootstrap for the theme-extension preview server.

use crate::dev::{
    cookie_from_set_cookie, serialize_cookies, should_retry_storefront_session,
    storefront_session_retry_delay_ms, DevServerSession,
};
use crate::utilities::host_theme_manager::storefront_origin;
use std::collections::BTreeMap;
use std::time::Duration;

pub fn empty_dev_session(store_fqdn: impl Into<String>) -> DevServerSession {
    DevServerSession {
        store_fqdn: store_fqdn.into(),
        admin_token: String::new(),
        storefront_token: None,
        theme_access_domain: None,
        session_cookies: BTreeMap::new(),
    }
}

/// Admin + storefront-password session (upstream `initializeDevServerSession`).
/// Cookie fetch is best-effort so the preview server can still bind without Shopify.
pub async fn initialize_dev_server_session(
    theme_id: i64,
    store_fqdn: &str,
    admin_token: &str,
    storefront_password: Option<&str>,
) -> DevServerSession {
    let mut session = DevServerSession {
        store_fqdn: store_fqdn.to_string(),
        admin_token: admin_token.to_string(),
        storefront_token: None,
        theme_access_domain: None,
        session_cookies: BTreeMap::new(),
    };
    if admin_token.is_empty() && store_fqdn.is_empty() {
        return session;
    }
    match fetch_storefront_session_cookies(theme_id, &session, storefront_password).await {
        Ok(cookies) => session.session_cookies = cookies,
        Err(error) => {
            eprintln!("Theme extension storefront session: {error}");
        }
    }
    session
}

async fn fetch_storefront_session_cookies(
    theme_id: i64,
    session: &DevServerSession,
    storefront_password: Option<&str>,
) -> Result<BTreeMap<String, String>, String> {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|error| error.to_string())?;
    let origin = storefront_origin(&session.store_fqdn);
    let mut url = url::Url::parse(&origin).map_err(|error| error.to_string())?;
    url.query_pairs_mut()
        .append_pair("preview_theme_id", &theme_id.to_string())
        .append_pair("_fd", "0")
        .append_pair("pb", "0");

    let mut last_error = String::from("Unable to create storefront session.");
    for attempt in 1..=3u32 {
        match client.head(url.clone()).send().await {
            Ok(response) => {
                let status = response.status().as_u16();
                if should_retry_storefront_session(status) && attempt < 3 {
                    tokio::time::sleep(Duration::from_millis(storefront_session_retry_delay_ms(
                        attempt,
                    )))
                    .await;
                    continue;
                }
                let set_cookies = set_cookie_headers(response.headers());
                if let Some(essential) =
                    cookie_from_set_cookie(&set_cookies, "_shopify_essential")
                {
                    let mut cookies =
                        BTreeMap::from([("_shopify_essential".into(), essential)]);
                    if let Some(password) = storefront_password {
                        if let Ok(extra) = enrich_storefront_password(
                            &client,
                            &origin,
                            password,
                            &cookies,
                        )
                        .await
                        {
                            cookies.extend(extra);
                        }
                    }
                    return Ok(cookies);
                }
                last_error = "_shopify_essential cookie was not returned".into();
            }
            Err(error) => last_error = error.to_string(),
        }
        if attempt < 3 {
            tokio::time::sleep(Duration::from_secs(attempt.into())).await;
        }
    }
    Err(last_error)
}

async fn enrich_storefront_password(
    client: &reqwest::Client,
    origin: &str,
    password: &str,
    cookies: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, String>, String> {
    let response = client
        .post(format!("{origin}/password"))
        .header("cookie", serialize_cookies(cookies))
        .header("content-type", "application/x-www-form-urlencoded")
        .body(format!(
            "form_type=storefront_password&password={}",
            url::form_urlencoded::byte_serialize(password.as_bytes()).collect::<String>()
        ))
        .send()
        .await
        .map_err(|error| error.to_string())?;
    let set_cookies = set_cookie_headers(response.headers());
    let mut result = BTreeMap::new();
    if let Some(digest) = cookie_from_set_cookie(&set_cookies, "storefront_digest") {
        result.insert("storefront_digest".into(), digest);
    }
    if let Some(essential) = cookie_from_set_cookie(&set_cookies, "_shopify_essential") {
        result.insert("_shopify_essential".into(), essential);
    }
    Ok(result)
}

fn set_cookie_headers(headers: &reqwest::header::HeaderMap) -> Vec<String> {
    headers
        .get_all("set-cookie")
        .iter()
        .filter_map(|value| value.to_str().ok())
        .map(ToOwned::to_owned)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn session_captures_essential_cookie() {
        let server = MockServer::start().await;
        Mock::given(method("HEAD"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("set-cookie", "_shopify_essential=abc; Path=/"),
            )
            .mount(&server)
            .await;
        let session = initialize_dev_server_session(11, &server.uri(), "tok", None).await;
        assert_eq!(
            session.session_cookies.get("_shopify_essential").map(String::as_str),
            Some("abc")
        );
    }
}
