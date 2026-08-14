use crate::auth::config::{normalize_store_fqdn, STORE_AUTH_CALLBACK_PATH};
use crate::auth::pkce::WaitForAuthCodeOptions;
use crate::auth::recovery::retry_store_auth_with_permanent_domain_error;
use crate::error::StoreError;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::time::timeout;

fn html_escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn render_auth_callback_page(title: &str, message: &str) -> String {
    let safe_title = html_escape(title);
    let safe_message = html_escape(message);
    format!(
        r#"<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>{safe_title}</title>
  </head>
  <body>
    <h1>{safe_title}</h1>
    <p>{safe_message}</p>
  </body>
</html>"#
    )
}

fn constant_time_eq(left: &str, right: &str) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.bytes()
        .zip(right.bytes())
        .fold(0u8, |acc, (a, b)| acc | (a ^ b))
        == 0
}

fn http_response(status: u16, reason: &str, body: &str) -> Vec<u8> {
    format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: text/html\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    )
    .into_bytes()
}

fn parse_request_target(buffer: &str) -> Option<String> {
    let first = buffer.lines().next()?;
    let mut parts = first.split_whitespace();
    let _method = parts.next()?;
    parts.next().map(str::to_string)
}

pub async fn wait_for_store_auth_code(
    options: WaitForAuthCodeOptions,
    mut on_listening: Option<Box<dyn FnOnce() + Send>>,
) -> Result<String, StoreError> {
    let normalized_store = normalize_store_fqdn(&options.store);
    let listener = match TcpListener::bind(("127.0.0.1", options.port)).await {
        Ok(l) => l,
        Err(err) if err.kind() == std::io::ErrorKind::AddrInUse => {
            return Err(StoreError::with_try(
                format!("Port {} is already in use.", options.port),
                format!(
                    "Free port {} and re-run shopify store auth --store {} --scopes <comma-separated-scopes>. Ensure that redirect URI is allowed in the app configuration.",
                    options.port, options.store
                ),
            ));
        }
        Err(err) => return Err(err.into()),
    };

    if let Some(cb) = on_listening.take() {
        cb();
    }

    let wait = timeout(Duration::from_millis(options.timeout_ms.max(1)), async {
        loop {
            let (mut socket, _) = listener.accept().await?;
            let mut buf = vec![0u8; 8192];
            let n = socket.read(&mut buf).await?;
            let request = String::from_utf8_lossy(&buf[..n]);
            let Some(target) = parse_request_target(&request) else {
                let body = render_auth_callback_page("Authentication failed", "Invalid request");
                socket
                    .write_all(&http_response(400, "Bad Request", &body))
                    .await?;
                continue;
            };
            let url = format!("http://127.0.0.1:{}{target}", options.port);
            let parsed = url::Url::parse(&url).map_err(|e| StoreError::message(e.to_string()))?;
            if parsed.path() != STORE_AUTH_CALLBACK_PATH {
                socket
                    .write_all(&http_response(404, "Not Found", "Not found"))
                    .await?;
                continue;
            }

            let pairs: std::collections::HashMap<_, _> =
                parsed.query_pairs().into_owned().collect();

            let fail = |message: StoreError| -> (u16, String, StoreError) {
                (
                    400,
                    render_auth_callback_page(
                        "Authentication failed",
                        message.to_string().lines().next().unwrap_or("Authentication failed"),
                    ),
                    message,
                )
            };

            let outcome = if let Some(returned_store) = pairs.get("shop") {
                let normalized_returned = normalize_store_fqdn(returned_store);
                if normalized_returned != normalized_store {
                    Some(fail(retry_store_auth_with_permanent_domain_error(
                        &normalized_returned,
                    )))
                } else if pairs.get("state").map(|s| constant_time_eq(s, &options.state)) != Some(true)
                {
                    Some(fail(StoreError::message(
                        "OAuth callback state does not match the original request.",
                    )))
                } else if let Some(error) = pairs.get("error") {
                    Some(fail(StoreError::message(format!(
                        "Shopify returned an OAuth error: {error}"
                    ))))
                } else if let Some(code) = pairs.get("code").filter(|c| !c.is_empty()).cloned() {
                    let body = render_auth_callback_page(
                        "Authentication succeeded",
                        "Close this window and return to the terminal",
                    );
                    socket
                        .write_all(&http_response(200, "OK", &body))
                        .await?;
                    return Ok(code);
                } else {
                    Some(fail(StoreError::message(
                        "OAuth callback did not include an authorization code.",
                    )))
                }
            } else {
                Some(fail(StoreError::message(
                    "OAuth callback store does not match the requested store.",
                )))
            };

            if let Some((status, body, err)) = outcome {
                socket
                    .write_all(&http_response(status, "Bad Request", &body))
                    .await?;
                return Err(err);
            }
        }
    });

    match wait.await {
        Ok(Ok(code)) => Ok(code),
        Ok(Err(err)) => Err(err),
        Err(_) => Err(StoreError::message("Timed out waiting for OAuth callback.")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener as StdListener;

    async fn available_port() -> u16 {
        StdListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port()
    }

    fn callback_url(port: u16, extra: &str) -> String {
        format!("http://127.0.0.1:{port}/auth/callback?shop=shop.myshopify.com&state=state-123{extra}")
    }

    #[tokio::test]
    async fn resolves_after_valid_callback() {
        let port = available_port().await;
        let handle = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            reqwest::get(callback_url(port, "&code=abc123"))
                .await
                .unwrap()
                .status()
        });
        let code = wait_for_store_auth_code(
            WaitForAuthCodeOptions {
                store: "shop.myshopify.com".into(),
                state: "state-123".into(),
                port,
                timeout_ms: 2000,
            },
            None,
        )
        .await
        .unwrap();
        assert_eq!(code, "abc123");
        assert_eq!(handle.await.unwrap(), reqwest::StatusCode::OK);
    }

    #[tokio::test]
    async fn rejects_mismatched_state() {
        let port = available_port().await;
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            let _ = reqwest::get(format!(
                "http://127.0.0.1:{port}/auth/callback?shop=shop.myshopify.com&state=wrong-state&code=abc123"
            ))
            .await;
        });
        let err = wait_for_store_auth_code(
            WaitForAuthCodeOptions {
                store: "shop.myshopify.com".into(),
                state: "state-123".into(),
                port,
                timeout_ms: 2000,
            },
            None,
        )
        .await
        .unwrap_err();
        assert!(err
            .to_string()
            .contains("OAuth callback state does not match the original request."));
    }

    #[tokio::test]
    async fn rejects_mismatched_shop() {
        let port = available_port().await;
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            let _ = reqwest::get(format!(
                "http://127.0.0.1:{port}/auth/callback?shop=other-shop.myshopify.com&state=state-123&code=abc123"
            ))
            .await;
        });
        let err = wait_for_store_auth_code(
            WaitForAuthCodeOptions {
                store: "shop.myshopify.com".into(),
                state: "state-123".into(),
                port,
                timeout_ms: 2000,
            },
            None,
        )
        .await
        .unwrap_err();
        assert!(err
            .to_string()
            .contains("OAuth callback store does not match the requested store."));
        assert!(err.to_string().contains("other-shop.myshopify.com"));
    }

    #[tokio::test]
    async fn rejects_oauth_error() {
        let port = available_port().await;
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            let _ = reqwest::get(callback_url(port, "&error=access_denied")).await;
        });
        let err = wait_for_store_auth_code(
            WaitForAuthCodeOptions {
                store: "shop.myshopify.com".into(),
                state: "state-123".into(),
                port,
                timeout_ms: 2000,
            },
            None,
        )
        .await
        .unwrap_err();
        assert!(err
            .to_string()
            .contains("Shopify returned an OAuth error: access_denied"));
    }

    #[tokio::test]
    async fn rejects_missing_code() {
        let port = available_port().await;
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            let _ = reqwest::get(format!(
                "http://127.0.0.1:{port}/auth/callback?shop=shop.myshopify.com&state=state-123"
            ))
            .await;
        });
        let err = wait_for_store_auth_code(
            WaitForAuthCodeOptions {
                store: "shop.myshopify.com".into(),
                state: "state-123".into(),
                port,
                timeout_ms: 2000,
            },
            None,
        )
        .await
        .unwrap_err();
        assert!(err
            .to_string()
            .contains("OAuth callback did not include an authorization code."));
    }

    #[tokio::test]
    async fn rejects_port_in_use() {
        let listener = StdListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let err = wait_for_store_auth_code(
            WaitForAuthCodeOptions {
                store: "shop.myshopify.com".into(),
                state: "state-123".into(),
                port,
                timeout_ms: 1000,
            },
            None,
        )
        .await
        .unwrap_err();
        assert!(err.to_string().contains(&format!("Port {port} is already in use.")));
        drop(listener);
    }

    #[tokio::test]
    async fn rejects_on_timeout() {
        let port = available_port().await;
        let err = wait_for_store_auth_code(
            WaitForAuthCodeOptions {
                store: "shop.myshopify.com".into(),
                state: "state-123".into(),
                port,
                timeout_ms: 25,
            },
            None,
        )
        .await
        .unwrap_err();
        assert!(err
            .to_string()
            .contains("Timed out waiting for OAuth callback."));
    }
}
