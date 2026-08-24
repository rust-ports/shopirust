//! Path-prefix reverse proxy for `app dev` (`http-reverse-proxy.ts`).

use super::types::{DevProcess, DevProcessKind};
use crate::error::AppError;
use crate::services::dev::mkcert::LocalhostCert;
use http_body_util::{BodyExt, Full};
use hyper::body::Bytes;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone)]
pub struct ProxyServerOptions {
    pub port: u16,
    pub rules: BTreeMap<String, String>,
    pub localhost_cert: Option<LocalhostCert>,
}

pub fn setup_proxy_server_process(opts: ProxyServerOptions) -> DevProcess {
    DevProcess::new("proxy", DevProcessKind::ProxyServer, move |ctx| {
        run_proxy(ctx.abort, opts)
    })
}

/// Match a request path against prefix rules (`default` / `websocket` are special keys).
pub fn match_proxy_target(
    rules: &BTreeMap<String, String>,
    path: &str,
    websocket: bool,
) -> Option<String> {
    for (prefix, target) in rules {
        if prefix == "default" || prefix == "websocket" {
            continue;
        }
        if path.starts_with(prefix) {
            return Some(target.clone());
        }
    }
    if websocket {
        if let Some(ws) = rules.get("websocket") {
            return Some(ws.clone());
        }
    }
    rules.get("default").cloned()
}

async fn run_proxy(abort: CancellationToken, opts: ProxyServerOptions) -> Result<(), AppError> {
    let addr = SocketAddr::from(([127, 0, 0, 1], opts.port));
    let listener = TcpListener::bind(addr)
        .await
        .map_err(|e| AppError::message(format!("failed to bind reverse proxy on {addr}: {e}")))?;
    tracing::info!(
        target: "app_dev",
        "Proxy server started on port {} {}",
        opts.port,
        opts.localhost_cert
            .as_ref()
            .map(|c| format!("with certificate {}", c.cert_path))
            .unwrap_or_default()
    );

    let rules = Arc::new(opts.rules);
    let tls = opts.localhost_cert.as_ref().map(tls_acceptor).transpose()?;

    loop {
        tokio::select! {
            _ = abort.cancelled() => break,
            accepted = listener.accept() => {
                let Ok((stream, _)) = accepted else { continue };
                let rules = rules.clone();
                let tls = tls.clone();
                tokio::spawn(async move {
                    if let Some(acceptor) = tls {
                        match acceptor.accept(stream).await {
                            Ok(tls_stream) => {
                                let io = TokioIo::new(tls_stream);
                                let _ = http1::Builder::new()
                                    .serve_connection(
                                        io,
                                        service_fn(move |req| handle(req, rules.clone())),
                                    )
                                    .with_upgrades()
                                    .await;
                            }
                            Err(e) => {
                                tracing::debug!(target: "app_dev", "tls accept: {e}");
                            }
                        }
                    } else {
                        let io = TokioIo::new(stream);
                        let _ = http1::Builder::new()
                            .serve_connection(
                                io,
                                service_fn(move |req| handle(req, rules.clone())),
                            )
                            .with_upgrades()
                            .await;
                    }
                });
            }
        }
    }
    Ok(())
}

fn tls_acceptor(cert: &LocalhostCert) -> Result<tokio_rustls::TlsAcceptor, AppError> {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let mut cert_reader = std::io::BufReader::new(cert.cert.as_bytes());
    let mut key_reader = std::io::BufReader::new(cert.key.as_bytes());
    let certs: Vec<_> = rustls_pemfile::certs(&mut cert_reader)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| AppError::message(format!("invalid localhost cert: {e}")))?;
    let key = rustls_pemfile::private_key(&mut key_reader)
        .map_err(|e| AppError::message(format!("invalid localhost key: {e}")))?
        .ok_or_else(|| AppError::message("localhost key PEM contained no private key"))?;
    let config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|e| AppError::message(format!("tls config: {e}")))?;
    Ok(tokio_rustls::TlsAcceptor::from(Arc::new(config)))
}

type BoxBody = http_body_util::Full<Bytes>;

async fn handle(
    req: Request<hyper::body::Incoming>,
    rules: Arc<BTreeMap<String, String>>,
) -> Result<Response<BoxBody>, hyper::Error> {
    let path = req
        .uri()
        .path_and_query()
        .map(|p| p.as_str().to_string())
        .unwrap_or_else(|| "/".into());
    let is_ws = req
        .headers()
        .get("upgrade")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.eq_ignore_ascii_case("websocket"))
        .unwrap_or(false);

    if req.method() == hyper::Method::OPTIONS {
        return Ok(cors_preflight(&req));
    }

    let Some(target) = match_proxy_target(&rules, &path, is_ws) else {
        return Ok(Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Full::new(Bytes::from(format!("Invalid path {path}"))))
            .unwrap());
    };

    if is_ws {
        return proxy_websocket(req, &target).await;
    }

    match proxy_http(req, &target).await {
        Ok(resp) => Ok(resp),
        Err(e) => {
            tracing::warn!(
                target: "app_dev",
                "Error forwarding web request: {e} (target={target} path={path})"
            );
            Ok(Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .body(Full::new(Bytes::from(format!(
                    "Unreachable target {target}"
                ))))
                .unwrap())
        }
    }
}

fn cors_preflight(req: &Request<hyper::body::Incoming>) -> Response<BoxBody> {
    let origin = req
        .headers()
        .get("origin")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("*");
    let methods = req
        .headers()
        .get("access-control-request-method")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("GET, POST, PUT, DELETE, PATCH, OPTIONS");
    let headers = req
        .headers()
        .get("access-control-request-headers")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("Content-Type, Authorization");
    Response::builder()
        .status(StatusCode::NO_CONTENT)
        .header("Access-Control-Allow-Origin", origin)
        .header("Access-Control-Allow-Methods", methods)
        .header("Access-Control-Allow-Headers", headers)
        .header("Access-Control-Max-Age", "86400")
        .body(Full::new(Bytes::new()))
        .unwrap()
}

async fn proxy_http(
    req: Request<hyper::body::Incoming>,
    target: &str,
) -> Result<Response<BoxBody>, AppError> {
    let path = req
        .uri()
        .path_and_query()
        .map(|p| p.as_str())
        .unwrap_or("/");
    let url = format!("{}{path}", target.trim_end_matches('/'));
    let method = req.method().clone();
    let headers = req.headers().clone();
    let body = req
        .into_body()
        .collect()
        .await
        .map_err(|e| AppError::message(e.to_string()))?
        .to_bytes();

    let client = reqwest::Client::new();
    let mut builder = client.request(method.clone(), &url);
    for (name, value) in headers.iter() {
        if name == hyper::header::HOST || name == hyper::header::CONNECTION {
            continue;
        }
        builder = builder.header(name, value);
    }
    let response = builder
        .body(body)
        .send()
        .await
        .map_err(|e| AppError::message(e.to_string()))?;
    let status = response.status();
    let resp_headers = response.headers().clone();
    let bytes = response
        .bytes()
        .await
        .map_err(|e| AppError::message(e.to_string()))?;
    let mut out = Response::builder().status(status.as_u16());
    for (name, value) in resp_headers.iter() {
        if name == hyper::header::TRANSFER_ENCODING || name == hyper::header::CONNECTION {
            continue;
        }
        out = out.header(name, value);
    }
    out.body(Full::new(bytes))
        .map_err(|e| AppError::message(e.to_string()))
}

async fn proxy_websocket(
    mut req: Request<hyper::body::Incoming>,
    target: &str,
) -> Result<Response<BoxBody>, hyper::Error> {
    let path = req
        .uri()
        .path_and_query()
        .map(|p| p.as_str())
        .unwrap_or("/")
        .to_string();
    let http_url = format!("{}{path}", target.trim_end_matches('/'));
    let Ok(target_url) = url::Url::parse(&http_url) else {
        return Ok(bad_gateway("invalid websocket target"));
    };
    let host = target_url.host_str().unwrap_or("127.0.0.1").to_string();
    let port = target_url.port_or_known_default().unwrap_or(80);
    let host_header = if port == 80 || port == 443 {
        host.clone()
    } else {
        format!("{host}:{port}")
    };

    let Ok(mut upstream) = TcpStream::connect((host.as_str(), port)).await else {
        return Ok(bad_gateway("failed to connect websocket target"));
    };

    let mut head = format!(
        "{} {path} HTTP/1.1\r\nHost: {host_header}\r\n",
        req.method()
    );
    for (name, value) in req.headers() {
        if name == hyper::header::HOST {
            continue;
        }
        if let Ok(value) = value.to_str() {
            head.push_str(&format!("{name}: {value}\r\n"));
        }
    }
    head.push_str("\r\n");
    if upstream.write_all(head.as_bytes()).await.is_err() {
        return Ok(bad_gateway("failed to write websocket handshake"));
    }

    let Ok((status, headers, leftover)) = read_http_head(&mut upstream).await else {
        return Ok(bad_gateway("failed to read websocket handshake"));
    };
    if status != u16::from(StatusCode::SWITCHING_PROTOCOLS) {
        let mut builder = Response::builder().status(status);
        for (name, value) in &headers {
            builder = builder.header(name, value);
        }
        return Ok(builder
            .body(Full::new(Bytes::from(leftover)))
            .unwrap_or_else(|_| bad_gateway("invalid handshake response")));
    }

    let upgrade = hyper::upgrade::on(&mut req);
    tokio::spawn(async move {
        let Ok(upgraded) = upgrade.await else {
            return;
        };
        let mut io = TokioIo::new(upgraded);
        if !leftover.is_empty() {
            let _ = tokio::io::AsyncWriteExt::write_all(&mut io, &leftover).await;
        }
        let _ = tokio::io::copy_bidirectional(&mut io, &mut upstream).await;
    });

    let mut builder = Response::builder().status(StatusCode::SWITCHING_PROTOCOLS);
    for (name, value) in headers {
        builder = builder.header(name, value);
    }
    Ok(builder.body(Full::new(Bytes::new())).unwrap())
}

fn bad_gateway(message: &'static str) -> Response<BoxBody> {
    Response::builder()
        .status(StatusCode::BAD_GATEWAY)
        .body(Full::new(Bytes::from(message)))
        .unwrap()
}

async fn read_http_head(
    stream: &mut TcpStream,
) -> Result<(u16, Vec<(String, String)>, Vec<u8>), std::io::Error> {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 1024];
    loop {
        let n = stream.read(&mut tmp).await?;
        if n == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "eof before websocket handshake headers",
            ));
        }
        buf.extend_from_slice(&tmp[..n]);
        if let Some(end) = buf.windows(4).position(|window| window == b"\r\n\r\n") {
            let head = buf[..end].to_vec();
            let leftover = buf[end + 4..].to_vec();
            let parsed = parse_http_head(&head).ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, "invalid handshake")
            })?;
            return Ok((parsed.0, parsed.1, leftover));
        }
        if buf.len() > 64 * 1024 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "handshake headers too large",
            ));
        }
    }
}

fn parse_http_head(head: &[u8]) -> Option<(u16, Vec<(String, String)>)> {
    let text = std::str::from_utf8(head).ok()?;
    let mut lines = text.split("\r\n");
    let status_line = lines.next()?;
    let status = status_line.split_whitespace().nth(1)?.parse().ok()?;
    let mut headers = Vec::new();
    for line in lines {
        let (name, value) = line.split_once(':')?;
        headers.push((name.trim().to_string(), value.trim().to_string()));
    }
    Some((status, headers))
}

/// Bind and serve a proxy until `abort` is cancelled. Used by unit tests.
pub async fn serve_proxy_for_tests(
    rules: BTreeMap<String, String>,
    abort: CancellationToken,
    cert: Option<LocalhostCert>,
) -> Result<u16, AppError> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|e| AppError::message(e.to_string()))?;
    let port = listener
        .local_addr()
        .map_err(|e| AppError::message(e.to_string()))?
        .port();
    drop(listener);
    let opts = ProxyServerOptions {
        port,
        rules,
        localhost_cert: cert,
    };
    tokio::spawn(run_proxy(abort, opts));
    // Give the server a moment to bind.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    Ok(port)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::routing::get;
    use axum::Router;
    use std::net::SocketAddr;

    async fn spawn_echo(body: &'static str) -> u16 {
        let app = Router::new()
            .route("/*rest", get(move || async move { body }))
            .route("/", get(move || async move { body }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        port
    }

    #[test]
    fn matches_path_prefix() {
        let mut rules = BTreeMap::new();
        rules.insert("/path1".into(), "http://localhost:1".into());
        rules.insert("/path2".into(), "http://localhost:2".into());
        rules.insert("default".into(), "http://localhost:1".into());
        assert_eq!(
            match_proxy_target(&rules, "/path1/test", false).as_deref(),
            Some("http://localhost:1")
        );
        assert_eq!(
            match_proxy_target(&rules, "/path2/test", false).as_deref(),
            Some("http://localhost:2")
        );
        assert_eq!(
            match_proxy_target(&rules, "/unknown", false).as_deref(),
            Some("http://localhost:1")
        );
    }

    #[test]
    fn websocket_fallback() {
        let mut rules = BTreeMap::new();
        rules.insert("websocket".into(), "http://localhost:9".into());
        rules.insert("default".into(), "http://localhost:1".into());
        assert_eq!(
            match_proxy_target(&rules, "/ws", true).as_deref(),
            Some("http://localhost:9")
        );
    }

    #[tokio::test]
    async fn routes_http_by_path() {
        let p1 = spawn_echo("Response from target server 1").await;
        let p2 = spawn_echo("Response from target server 2").await;
        let mut rules = BTreeMap::new();
        rules.insert("/path1".into(), format!("http://127.0.0.1:{p1}"));
        rules.insert("/path2".into(), format!("http://127.0.0.1:{p2}"));
        rules.insert("default".into(), format!("http://127.0.0.1:{p1}"));
        let abort = CancellationToken::new();
        let port = serve_proxy_for_tests(rules, abort.clone(), None)
            .await
            .unwrap();

        let client = reqwest::Client::new();
        let r1 = client
            .get(format!("http://127.0.0.1:{port}/path1/test"))
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        assert_eq!(r1, "Response from target server 1");
        let r2 = client
            .get(format!("http://127.0.0.1:{port}/path2/test"))
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        assert_eq!(r2, "Response from target server 2");
        let r3 = client
            .get(format!("http://127.0.0.1:{port}/unknown/path"))
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        assert_eq!(r3, "Response from target server 1");

        let cors = client
            .request(
                reqwest::Method::OPTIONS,
                format!("http://127.0.0.1:{port}/path1/test"),
            )
            .header("Origin", "https://extensions.shopifycdn.com")
            .header("Access-Control-Request-Method", "GET")
            .header("Access-Control-Request-Headers", "Authorization")
            .send()
            .await
            .unwrap();
        assert_eq!(cors.status(), 204);
        assert_eq!(
            cors.headers()
                .get("access-control-allow-origin")
                .unwrap()
                .to_str()
                .unwrap(),
            "https://extensions.shopifycdn.com"
        );
        assert_eq!(
            cors.headers()
                .get("access-control-allow-methods")
                .unwrap()
                .to_str()
                .unwrap(),
            "GET"
        );

        let cors_default = client
            .request(
                reqwest::Method::OPTIONS,
                format!("http://127.0.0.1:{port}/path1/test"),
            )
            .send()
            .await
            .unwrap();
        assert_eq!(cors_default.status(), 204);
        assert_eq!(
            cors_default
                .headers()
                .get("access-control-allow-origin")
                .unwrap()
                .to_str()
                .unwrap(),
            "*"
        );

        abort.cancel();
        let _ = SocketAddr::from(([127, 0, 0, 1], port));
    }

    #[tokio::test]
    async fn websocket_completes_upstream_handshake() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let upstream_port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 4096];
            let mut n = 0usize;
            loop {
                let read = stream.read(&mut buf[n..]).await.unwrap();
                n += read;
                if buf[..n].windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
            }
            stream
                .write_all(b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: dummy\r\n\r\n")
                .await
                .unwrap();
            let mut payload = vec![0u8; 16];
            let k = stream.read(&mut payload).await.unwrap();
            stream.write_all(&payload[..k]).await.unwrap();
        });

        let mut rules = BTreeMap::new();
        rules.insert(
            "websocket".into(),
            format!("http://127.0.0.1:{upstream_port}"),
        );
        let abort = CancellationToken::new();
        let port = serve_proxy_for_tests(rules, abort.clone(), None)
            .await
            .unwrap();

        let mut client = tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .unwrap();
        client
            .write_all(b"GET /ext HTTP/1.1\r\nHost: 127.0.0.1\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n")
            .await
            .unwrap();

        let mut buf = vec![0u8; 4096];
        let mut n = 0usize;
        loop {
            let read = client.read(&mut buf[n..]).await.unwrap();
            n += read;
            if buf[..n].windows(4).any(|window| window == b"\r\n\r\n") {
                break;
            }
        }
        let head = String::from_utf8_lossy(&buf[..n]);
        assert!(head.contains("101"), "handshake was not 101: {head}");
        assert!(
            head.to_ascii_lowercase().contains("sec-websocket-accept"),
            "missing accept header in: {head}"
        );
        assert!(
            head.contains("dummy"),
            "upstream accept not forwarded in: {head}"
        );

        client.write_all(b"hello-ws").await.unwrap();
        let mut echo = vec![0u8; 8];
        client.read_exact(&mut echo).await.unwrap();
        assert_eq!(&echo, b"hello-ws");
        abort.cancel();
    }
}
