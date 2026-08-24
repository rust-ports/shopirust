use crate::console::{ConsoleError, DevServerSession, RenderContext, StorefrontRenderer};
use base64::Engine;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

pub const PROFILE_ACCEPT: &str = "application/vnd.speedscope+json";
pub const PROFILE_PASSWORD_ERROR: &str =
    "Unable to use Admin API or Theme Access tokens with the profile command";
pub const PROFILE_PASSWORD_NEXT_STEPS: &str =
    "You must authenticate manually by not passing the --password flag.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenProfileFiles {
    pub js_path: PathBuf,
    pub html_path: PathBuf,
    pub url: String,
}

pub async fn capture_profile<R: StorefrontRenderer + Sync>(
    renderer: &R,
    theme_session: &DevServerSession,
    theme_id: String,
    url: String,
) -> Result<String, ConsoleError> {
    let response = renderer
        .render(
            theme_session,
            RenderContext {
                method: "GET".into(),
                path: url,
                query: Vec::new(),
                theme_id,
                section_id: None,
                app_block_id: None,
                headers: BTreeMap::from([("Accept".into(), PROFILE_ACCEPT.into())]),
                replace_templates: BTreeMap::new(),
            },
        )
        .await?;

    if response.status != 200 {
        return Err(ConsoleError::Abort(format!(
            "Bad response: {}: {}",
            response.status, response.body
        )));
    }

    Ok(response.body)
}

pub fn prepare_profile_files(profile_json: &str) -> Result<OpenProfileFiles, ConsoleError> {
    let distro_name = std::env::var("WSL_DISTRO_NAME").unwrap_or_default();
    let wsl_prefix = if distro_name.is_empty() {
        String::new()
    } else {
        format!("//wsl$/{distro_name}")
    };

    let mut url_to_open = resolve_speedscope_index()?.display().to_string();
    let filename = "liquid-profile";
    let source_base64 = base64::engine::general_purpose::STANDARD.encode(profile_json);
    let js_source = format!(
        "speedscope.loadFileFromBase64({}, {})",
        serde_json::to_string(filename)?,
        serde_json::to_string(&source_base64)?
    );

    let file_prefix = format!(
        "speedscope-{}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or_default(),
        std::process::id()
    );
    let mut js_path = std::env::temp_dir();
    js_path.push(format!("{file_prefix}.js"));
    std::fs::write(&js_path, js_source).map_err(|error| ConsoleError::Io(error.to_string()))?;

    url_to_open = format!(
        "{wsl_prefix}{}#localProfilePath={wsl_prefix}{}",
        url_to_open,
        js_path.display()
    );

    let mut html_path = std::env::temp_dir();
    html_path.push(format!("{file_prefix}.html"));
    let html_source = format!(
        "<script>window.location={}</script>",
        serde_json::to_string(&url_to_open)?
    );
    std::fs::write(&html_path, html_source).map_err(|error| ConsoleError::Io(error.to_string()))?;

    let url = format!("file://{wsl_prefix}{}", html_path.display());
    Ok(OpenProfileFiles {
        js_path,
        html_path,
        url,
    })
}

pub fn open_profile(profile_json: &str) -> Result<OpenProfileFiles, ConsoleError> {
    let files = prepare_profile_files(profile_json)?;
    open::that(&files.url).map_err(|error| ConsoleError::Abort(error.to_string()))?;
    Ok(files)
}

fn resolve_speedscope_index() -> Result<PathBuf, ConsoleError> {
    static CACHE: OnceLock<PathBuf> = OnceLock::new();

    if let Some(path) = CACHE.get() {
        return Ok(path.clone());
    }

    let html = String::from_utf8_lossy(include_bytes!("../assets/speedscope/index.html"));
    let mut path = std::env::temp_dir();
    path.push("shopify-speedscope-index.html");
    std::fs::write(&path, html.as_bytes()).map_err(|error| ConsoleError::Io(error.to_string()))?;

    let _ = CACHE.set(path.clone());
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::console::{RenderResponse, StorefrontRenderer};
    use async_trait::async_trait;
    use std::sync::Mutex;

    struct MockRenderer {
        response: Mutex<Option<RenderResponse>>,
        request: Mutex<Option<RenderContext>>,
    }

    #[async_trait]
    impl StorefrontRenderer for MockRenderer {
        async fn render(
            &self,
            _session: &DevServerSession,
            context: RenderContext,
        ) -> Result<RenderResponse, ConsoleError> {
            *self.request.lock().unwrap() = Some(context);
            Ok(self.response.lock().unwrap().take().unwrap())
        }
    }

    fn session() -> DevServerSession {
        DevServerSession {
            store_fqdn: "store.myshopify.com".into(),
            token: "token".into(),
            storefront_token: Some("storefront".into()),
            theme_access_domain: None,
            session_cookies: BTreeMap::new(),
        }
    }

    #[tokio::test]
    async fn capture_profile_sends_upstream_render_request() {
        let renderer = MockRenderer {
            response: Mutex::new(Some(RenderResponse {
                status: 200,
                headers: BTreeMap::new(),
                body: r#"{"profile":true}"#.into(),
            })),
            request: Mutex::new(None),
        };

        let result = capture_profile(&renderer, &session(), "123".into(), "/products/test".into())
            .await
            .unwrap();

        assert_eq!(result, r#"{"profile":true}"#);
        let request = renderer.request.lock().unwrap().clone().unwrap();
        assert_eq!(request.method, "GET");
        assert_eq!(request.path, "/products/test");
        assert_eq!(request.theme_id, "123");
        assert_eq!(
            request.headers.get("Accept").map(String::as_str),
            Some(PROFILE_ACCEPT)
        );
    }

    #[tokio::test]
    async fn capture_profile_rejects_non_200_response() {
        let renderer = MockRenderer {
            response: Mutex::new(Some(RenderResponse {
                status: 404,
                headers: BTreeMap::new(),
                body: r#"{"error":"Some error message"}"#.into(),
            })),
            request: Mutex::new(None),
        };

        let error = capture_profile(&renderer, &session(), "123".into(), "/".into())
            .await
            .unwrap_err();

        assert_eq!(
            error.to_string(),
            r#"Bad response: 404: {"error":"Some error message"}"#
        );
    }

    #[test]
    fn prepare_profile_files_writes_redirect_html() {
        let files = prepare_profile_files(r#"{"profile":true}"#).unwrap();
        let html = std::fs::read_to_string(&files.html_path).unwrap();
        let js = std::fs::read_to_string(&files.js_path).unwrap();

        assert!(files.url.starts_with("file://"));
        assert!(html.contains("window.location"));
        assert!(html.contains("speedscope"));
        assert!(js.contains("speedscope.loadFileFromBase64"));
    }
}
