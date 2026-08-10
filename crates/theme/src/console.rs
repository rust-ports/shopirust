use crate::filesystem::ThemeAsset;
use crate::local_storage::{
    remove_repl_theme_id_for_store, repl_theme_id_for_store, store_repl_theme_id_for_store,
};
use crate::models::{Theme, DEVELOPMENT_THEME_ROLE};
use async_trait::async_trait;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::io::{BufRead, Write};
use thiserror::Error;
use tokio::sync::mpsc;

pub const DELIMITER_WARNING: &str = "Liquid Console doesn't support Liquid delimiters such as '{{ ... }}' or '{% ... %}'.\nPlease use 'collections.first' instead of '{{ collections.first }}'.";
pub const JSON_ERROR_WARNING: &str =
    "Object can't be printed, but you can access its fields. Read more at https://shopify.dev/docs/api/liquid.";
pub const ADMIN_TOKEN_ERROR: &str = "Unable to use Admin API tokens with the console command";
pub const ADMIN_TOKEN_NEXT_STEPS: &str = "To use this command with the --password flag you must:\n\n1. Install the Theme Access app on your shop\n2. Generate a new password\n\nAlternatively, you can authenticate normally by not passing the --password flag.\n\nLearn more: https://shopify.dev/docs/storefronts/themes/tools/theme-access";
pub const USER_AGENT: &str = "Shopify CLI; v=3.94.3";
pub const PROMPT: &str = "> ";

pub fn repl_theme_name(cli_version: &str) -> String {
    format!("Liquid Console ({cli_version})")
}

pub fn repl_theme_assets() -> Vec<ThemeAsset> {
    vec![
        asset("config/settings_data.json", "{}"),
        asset("config/settings_schema.json", "[]"),
        asset("snippets/eval.liquid", ""),
        asset(
            "layout/password.liquid",
            "{{ content_for_header }}{{ content_for_layout }}",
        ),
        asset(
            "layout/theme.liquid",
            "{{ content_for_header }}{{ content_for_layout }}",
        ),
        asset("sections/announcement-bar.liquid", ""),
        asset(
            "templates/index.json",
            &serde_json::json!({
                "sections": {
                    "announcement": {
                        "type": "announcement-bar",
                        "settings": {}
                    }
                },
                "order": ["announcement"]
            })
            .to_string(),
        ),
    ]
}

fn asset(key: &str, value: &str) -> ThemeAsset {
    ThemeAsset {
        key: key.into(),
        checksum: crate::checksum::calculate_checksum(key, Some(value.to_string().into())),
        attachment: None,
        value: Some(value.into()),
        stats: None,
    }
}

#[derive(Debug, Error)]
pub enum ConsoleError {
    #[error("{0}")]
    Abort(String),
    #[error("{0}")]
    Api(String),
    #[error("{0}")]
    Io(String),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Http(#[from] reqwest::Error),
}

#[async_trait]
pub trait ConsoleAdmin {
    async fn fetch_theme(&self, id: i64) -> Result<Option<Theme>, ConsoleError>;
    async fn create_theme(&self, name: String, role: String) -> Result<Theme, ConsoleError>;
    async fn upload_assets(
        &self,
        theme_id: i64,
        assets: Vec<ThemeAsset>,
    ) -> Result<(), ConsoleError>;
}

#[derive(Debug, Clone)]
pub struct ReplThemeManager {
    pub store: String,
    pub cli_version: String,
}

impl ReplThemeManager {
    pub fn new(store: impl Into<String>, cli_version: impl Into<String>) -> Self {
        Self {
            store: store.into(),
            cli_version: cli_version.into(),
        }
    }

    pub async fn find_or_create<A: ConsoleAdmin + Sync>(
        &self,
        api: &A,
    ) -> Result<Theme, ConsoleError> {
        if let Some(theme_id) = repl_theme_id_for_store(&self.store) {
            if let Some(theme) = api.fetch_theme(theme_id).await? {
                store_repl_theme_id_for_store(&self.store, theme.id);
                return Ok(theme);
            }
            remove_repl_theme_id_for_store(&self.store);
        }

        let theme = api
            .create_theme(
                repl_theme_name(&self.cli_version),
                DEVELOPMENT_THEME_ROLE.into(),
            )
            .await?;
        api.upload_assets(theme.id, repl_theme_assets()).await?;
        store_repl_theme_id_for_store(&self.store, theme.id);
        Ok(theme)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DevServerSession {
    pub store_fqdn: String,
    pub token: String,
    pub storefront_token: Option<String>,
    pub theme_access_domain: Option<String>,
    pub session_cookies: BTreeMap<String, String>,
}

impl From<crate::dev::DevServerSession> for DevServerSession {
    fn from(session: crate::dev::DevServerSession) -> Self {
        Self {
            store_fqdn: session.store_fqdn,
            token: session.admin_token,
            storefront_token: session.storefront_token,
            theme_access_domain: session.theme_access_domain,
            session_cookies: session.session_cookies,
        }
    }
}

impl From<DevServerSession> for crate::dev::DevServerSession {
    fn from(session: DevServerSession) -> Self {
        Self {
            store_fqdn: session.store_fqdn,
            admin_token: session.token,
            storefront_token: session.storefront_token,
            theme_access_domain: session.theme_access_domain,
            session_cookies: session.session_cookies,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderContext {
    pub method: String,
    pub path: String,
    pub query: Vec<(String, String)>,
    pub theme_id: String,
    pub section_id: Option<String>,
    pub app_block_id: Option<String>,
    pub headers: BTreeMap<String, String>,
    pub replace_templates: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderResponse {
    pub status: u16,
    pub headers: BTreeMap<String, String>,
    pub body: String,
}

#[async_trait]
pub trait StorefrontRenderer {
    async fn render(
        &self,
        session: &DevServerSession,
        context: RenderContext,
    ) -> Result<RenderResponse, ConsoleError>;
}

#[derive(Debug, Clone, Default)]
pub struct ReqwestStorefrontRenderer {
    client: reqwest::Client,
}

impl ReqwestStorefrontRenderer {
    pub fn new() -> Result<Self, ConsoleError> {
        Ok(Self {
            client: reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .build()?,
        })
    }
}

#[async_trait]
impl StorefrontRenderer for ReqwestStorefrontRenderer {
    async fn render(
        &self,
        session: &DevServerSession,
        mut context: RenderContext,
    ) -> Result<RenderResponse, ConsoleError> {
        if !context.path.starts_with('/') {
            context.path = format!("/{}", context.path);
        }

        let url = build_render_url(session, &context)?;

        let mut method = context
            .method
            .parse::<reqwest::Method>()
            .map_err(|error| ConsoleError::Abort(error.to_string()))?;
        if !context.replace_templates.is_empty() {
            method = reqwest::Method::POST;
        }
        let headers = build_render_headers(session, &context);
        let mut builder = self.client.request(method, url.clone());
        for (key, value) in &headers {
            if let (Ok(name), Ok(value)) = (
                HeaderName::from_bytes(key.as_bytes()),
                HeaderValue::from_str(value),
            ) {
                builder = builder.header(name, value);
            }
        }

        if !context.replace_templates.is_empty() {
            let params = storefront_replace_templates_params(&context);
            builder = builder
                .header("content-type", "application/x-www-form-urlencoded")
                .body(params);
        }

        let response = builder
            .send()
            .await
            .map_err(|error| fetch_error(error, url.as_str()))?;
        let status = response.status().as_u16();
        let headers = render_response_headers(response.headers());
        let body = response.text().await?;
        Ok(RenderResponse {
            status,
            headers,
            body,
        })
    }
}

fn build_render_url(
    session: &DevServerSession,
    context: &RenderContext,
) -> Result<reqwest::Url, ConsoleError> {
    let base = session
        .theme_access_domain
        .as_deref()
        .unwrap_or(&session.store_fqdn);
    let path = if session.theme_access_domain.is_some() {
        format!("/cli/sfr{}", context.path)
    } else {
        context.path.clone()
    };
    let mut url = reqwest::Url::parse(&format!("https://{base}{path}"))
        .map_err(|error| ConsoleError::Abort(error.to_string()))?;
    {
        let mut pairs = url.query_pairs_mut();
        pairs.append_pair("_fd", "0");
        pairs.append_pair("pb", "0");
        for (key, value) in &context.query {
            pairs.append_pair(key, value);
        }
        if let Some(section_id) = &context.section_id {
            pairs.append_pair("section_id", section_id);
        } else if let Some(app_block_id) = &context.app_block_id {
            pairs.append_pair("app_block_id", app_block_id);
        }
    }
    Ok(url)
}

fn fetch_error(error: reqwest::Error, url: &str) -> ConsoleError {
    ConsoleError::Abort(format!("Failed to fetch {url}: {error}"))
}

fn default_headers() -> BTreeMap<String, String> {
    BTreeMap::from([("User-Agent".into(), USER_AGENT.into())])
}

fn build_render_headers(
    session: &DevServerSession,
    context: &RenderContext,
) -> BTreeMap<String, String> {
    if session.theme_access_domain.is_some() {
        build_theme_access_headers(session, &context.headers)
    } else {
        build_standard_headers(session, &context.headers)
    }
}

fn build_standard_headers(
    session: &DevServerSession,
    headers: &BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    let mut result = clean_headers(headers.clone());
    result.extend(default_headers());
    if let Some(token) = &session.storefront_token {
        result.insert("Authorization".into(), format!("Bearer {token}"));
    }
    result.insert("Cookie".into(), build_cookie_header(session, headers));
    result
}

fn build_theme_access_headers(
    session: &DevServerSession,
    headers: &BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    let mut result = BTreeMap::new();
    for (key, value) in headers {
        if matches!(
            key.to_ascii_uppercase().as_str(),
            "ACCEPT" | "CONTENT-TYPE" | "CONTENT-LENGTH"
        ) {
            result.insert(key.clone(), value.clone());
        }
    }
    result.extend(default_headers());
    result.insert("X-Shopify-Shop".into(), session.store_fqdn.clone());
    result.insert("X-Shopify-Access-Token".into(), session.token.clone());
    if let Some(token) = &session.storefront_token {
        result.insert("Authorization".into(), format!("Bearer {token}"));
    }
    result.insert("Cookie".into(), build_cookie_header(session, headers));
    result
}

fn clean_headers(mut headers: BTreeMap<String, String>) -> BTreeMap<String, String> {
    headers.retain(|key, _| {
        !matches!(
            key.to_ascii_lowercase().as_str(),
            "cookie" | "authorization"
        )
    });
    headers
}

fn build_cookie_header(session: &DevServerSession, headers: &BTreeMap<String, String>) -> String {
    let mut cookies = parse_cookie_header(
        headers
            .get("cookie")
            .or_else(|| headers.get("Cookie"))
            .map(String::as_str)
            .unwrap_or_default(),
    );
    cookies.extend(session.session_cookies.clone());
    serialize_cookies(&cookies)
}

fn render_response_headers(headers: &HeaderMap) -> BTreeMap<String, String> {
    let mut result = headers
        .iter()
        .filter_map(|(name, value)| {
            Some((name.as_str().to_string(), value.to_str().ok()?.to_string()))
        })
        .collect::<BTreeMap<_, _>>();
    let content_type_key = result
        .keys()
        .find(|key| key.eq_ignore_ascii_case("content-type"))
        .cloned();
    if let Some(key) = content_type_key {
        let json = result
            .get(&key)
            .is_some_and(|value| value.contains("application/json"));
        if !json {
            result.remove(&key);
        }
    }
    result
}

fn parse_cookie_header(value: &str) -> BTreeMap<String, String> {
    value
        .split(';')
        .filter_map(|item| {
            let item = item.trim();
            if item.is_empty() {
                return None;
            }
            let (key, value) = item.split_once('=')?;
            Some((key.trim().to_string(), value.trim().to_string()))
        })
        .collect()
}

fn serialize_cookies(cookies: &BTreeMap<String, String>) -> String {
    cookies
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join("; ")
}

fn storefront_replace_templates_params(context: &RenderContext) -> String {
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    for (key, value) in &context.replace_templates {
        serializer.append_pair(&format!("replace_templates[{key}]"), value);
    }
    serializer.append_pair("_method", &context.method);
    serializer.finish()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionItem {
    pub r#type: String,
    pub value: String,
}

#[derive(Debug, Clone)]
pub struct EvaluationConfig {
    pub theme_session: DevServerSession,
    pub theme_id: String,
    pub url: String,
    pub repl_session: Vec<SessionItem>,
    pub snippet: String,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PresentedValue {
    Null,
    Json(Value),
    Warning(String),
}

pub async fn evaluate<R: StorefrontRenderer + Sync>(
    renderer: &R,
    config: &mut EvaluationConfig,
) -> Result<Option<Value>, ConsoleError> {
    if let Some(value) = eval_result(renderer, config).await? {
        return Ok(Some(value));
    }
    eval_context(renderer, config).await?;
    if eval_assignment_context(renderer, config).await? {
        return Ok(None);
    }
    eval_syntax_error(renderer, config).await?;
    Ok(None)
}

async fn eval_result<R: StorefrontRenderer + Sync>(
    renderer: &R,
    config: &EvaluationConfig,
) -> Result<Option<Value>, ConsoleError> {
    let input = format!(
        r#"{{ "type": "display", "value": {{{{ {} | json }}}} }}"#,
        config.snippet
    );
    let response = make_request(renderer, config, input).await?;
    if successful_request(response.status, &response.body) {
        Ok(parse_display_result(&response.body)?)
    } else {
        Ok(None)
    }
}

async fn eval_context<R: StorefrontRenderer + Sync>(
    renderer: &R,
    config: &mut EvaluationConfig,
) -> Result<bool, ConsoleError> {
    let escaped = config.snippet.replace('"', "\\\"");
    let json = format!(r#"{{ "type": "context", "value": "{{% {escaped} %}}" }}"#);
    let response = make_request(renderer, config, json.clone()).await?;
    if successful_request(response.status, &response.body) {
        config.repl_session.push(serde_json::from_str(&json)?);
        return Ok(true);
    }
    Ok(false)
}

async fn eval_assignment_context<R: StorefrontRenderer + Sync>(
    renderer: &R,
    config: &mut EvaluationConfig,
) -> Result<bool, ConsoleError> {
    if is_smart_assignment(&config.snippet) {
        config.snippet = format!("assign {}", config.snippet);
        config.diagnostics.push(format!("> {}", config.snippet));
        return eval_context(renderer, config).await;
    }
    Ok(false)
}

async fn eval_syntax_error<R: StorefrontRenderer + Sync>(
    renderer: &R,
    config: &mut EvaluationConfig,
) -> Result<(), ConsoleError> {
    let mut body = String::new();
    if !is_standard_assignment(&config.snippet) {
        let response =
            make_request(renderer, config, format!("{{{{ {} }}}}", config.snippet)).await?;
        body = response.body;
    }

    if !has_liquid_error(&body) {
        let response =
            make_request(renderer, config, format!("{{% {} %}}", config.snippet)).await?;
        body = response.body;
    }

    if has_liquid_error(&body) {
        if let Some(message) = syntax_error_message(&config.snippet, &body) {
            config.diagnostics.push(message);
        }
        return Ok(());
    }
    Ok(())
}

fn syntax_error_message(snippet: &str, body: &str) -> Option<String> {
    let error = regex_lite::Regex::new(r" \(snippets/eval line \d+\)")
        .map(|regex| regex.replace_all(body, "").to_string())
        .unwrap_or_else(|_| body.to_string());
    if error.contains("Unknown tag") {
        return Some(format!(
            "Unknown object, property, tag, or filter: '{snippet}'"
        ));
    }
    strip_html_content(&error)
}

async fn make_request<R: StorefrontRenderer + Sync>(
    renderer: &R,
    config: &EvaluationConfig,
    snippet: String,
) -> Result<RenderResponse, ConsoleError> {
    let request_body = build_request_body(&config.repl_session, &snippet);
    let path = if config.url.starts_with('/') {
        config.url.clone()
    } else {
        format!("/{}", config.url)
    };
    let response = renderer
        .render(
            &config.theme_session,
            RenderContext {
                method: "GET".into(),
                path,
                query: vec![],
                theme_id: config.theme_id.clone(),
                section_id: Some("announcement-bar".into()),
                app_block_id: None,
                headers: BTreeMap::new(),
                replace_templates: BTreeMap::from([
                    (
                        "sections/announcement-bar.liquid".into(),
                        "{% render 'eval' %}".into(),
                    ),
                    ("snippets/eval.liquid".into(), format!("\n{request_body}\n")),
                ]),
            },
        )
        .await?;

    if response.status == 401 || response.status == 403 {
        return Err(ConsoleError::Abort(
            "Session expired. Please initiate a new one.".into(),
        ));
    }
    if response.status == 429 || response.status == 430 {
        return Err(ConsoleError::Abort(
            "Evaluations limit reached. Try again later.".into(),
        ));
    }
    if response
        .headers
        .get("server-timing")
        .is_some_and(|value| value.contains("pageType;desc=\"404\""))
    {
        return Err(ConsoleError::Abort(
            "Page not found. Please provide a valid --url value!".into(),
        ));
    }

    Ok(response)
}

fn build_request_body(session: &[SessionItem], snippet: &str) -> String {
    let items = session
        .iter()
        .map(|item| {
            serde_json::json!({
                "type": item.r#type,
                "value": item.value,
            })
            .to_string()
        })
        .chain(std::iter::once(snippet.to_string()))
        .collect::<Vec<_>>();
    format!("[{}]", items.join(",").replace("\\\"", "\""))
}

fn parse_display_result(result: &str) -> Result<Option<Value>, ConsoleError> {
    let Some(content) = strip_html_content(result) else {
        return Ok(None);
    };
    let values = serde_json::from_str::<Vec<Value>>(&content)?;
    Ok(values
        .into_iter()
        .find(|value| value.get("type").and_then(Value::as_str) == Some("display"))
        .and_then(|value| value.get("value").cloned()))
}

fn strip_html_content(result: &str) -> Option<String> {
    let lines = result.split('\n').collect::<Vec<_>>();
    if lines.len() <= 2 {
        return None;
    }
    Some(lines[1..lines.len() - 1].join(""))
}

fn has_liquid_error(body: &str) -> bool {
    body.contains("Liquid syntax error")
}

fn is_standard_assignment(input: &str) -> bool {
    regex_lite::Regex::new(r"^\s*assign\s*((?:\(?[\w\-.\[\]]\)?)+)\s*=\s*(.*)\s*")
        .is_ok_and(|regex| regex.is_match(input))
}

fn is_smart_assignment(input: &str) -> bool {
    regex_lite::Regex::new(r"^\s*((?:\(?[\w\-.\[\]]\)?)+)\s*=\s*(.*)\s*")
        .is_ok_and(|regex| regex.is_match(input))
}

fn successful_request(status: u16, text: &str) -> bool {
    status == 200 && !has_liquid_error(text)
}

pub fn present_value(value: Option<Value>) -> PresentedValue {
    let Some(value) = value else {
        return PresentedValue::Null;
    };
    if value.is_null() {
        return PresentedValue::Null;
    }
    if has_json_error(&value) {
        return PresentedValue::Warning(JSON_ERROR_WARNING.into());
    }
    PresentedValue::Json(value)
}

fn has_json_error(value: &Value) -> bool {
    if let Some(array) = value.as_array() {
        return array.first().is_some_and(has_json_error);
    }
    value
        .as_object()
        .and_then(|object| object.get("error"))
        .and_then(Value::as_str)
        .is_some_and(|message| message.contains("json not allowed for this object"))
}

pub fn format_presented_value(value: PresentedValue) -> String {
    match value {
        PresentedValue::Null => "null".into(),
        PresentedValue::Json(value) => {
            serde_json::to_string_pretty(&value).unwrap_or_else(|_| "null".into())
        }
        PresentedValue::Warning(message) => message,
    }
}

pub fn has_delimiter(input: &str) -> bool {
    regex_lite::Regex::new(r"^\s*(\{\{|\{%)")
        .map(|regex| regex.is_match(input))
        .unwrap_or(false)
}

pub async fn run_repl<R, I, O>(
    renderer: &R,
    theme_session: DevServerSession,
    theme_id: String,
    url: String,
    input: I,
    mut output: O,
) -> Result<(), ConsoleError>
where
    R: StorefrontRenderer + Sync,
    I: BufRead,
    O: Write,
{
    let mut repl_session = Vec::new();
    write!(output, "{PROMPT}").map_err(|error| ConsoleError::Io(error.to_string()))?;
    output
        .flush()
        .map_err(|error| ConsoleError::Io(error.to_string()))?;
    for line in input.lines() {
        let line = line.map_err(|error| ConsoleError::Io(error.to_string()))?;
        if has_delimiter(&line) {
            writeln!(output, "{DELIMITER_WARNING}")
                .map_err(|error| ConsoleError::Io(error.to_string()))?;
            write!(output, "{PROMPT}").map_err(|error| ConsoleError::Io(error.to_string()))?;
            output
                .flush()
                .map_err(|error| ConsoleError::Io(error.to_string()))?;
            continue;
        }
        let mut config = EvaluationConfig {
            theme_session: theme_session.clone(),
            theme_id: theme_id.clone(),
            url: url.clone(),
            repl_session,
            snippet: line,
            diagnostics: Vec::new(),
        };
        let value = evaluate(renderer, &mut config).await?;
        for diagnostic in &config.diagnostics {
            writeln!(output, "{diagnostic}")
                .map_err(|error| ConsoleError::Io(error.to_string()))?;
        }
        repl_session = config.repl_session;
        writeln!(output, "{}", format_presented_value(present_value(value)))
            .map_err(|error| ConsoleError::Io(error.to_string()))?;
        write!(output, "{PROMPT}").map_err(|error| ConsoleError::Io(error.to_string()))?;
        output
            .flush()
            .map_err(|error| ConsoleError::Io(error.to_string()))?;
    }
    Ok(())
}

pub async fn run_repl_stdio<R>(
    renderer: &R,
    theme_session: DevServerSession,
    theme_id: String,
    url: String,
) -> Result<(), ConsoleError>
where
    R: StorefrontRenderer + Sync,
{
    run_repl_stdio_with_refresh(renderer, theme_session, theme_id, url, None).await
}

pub async fn run_repl_stdio_with_refresh<R>(
    renderer: &R,
    mut theme_session: DevServerSession,
    theme_id: String,
    url: String,
    mut refresh_rx: Option<mpsc::Receiver<Result<crate::dev::DevServerSession, String>>>,
) -> Result<(), ConsoleError>
where
    R: StorefrontRenderer + Sync,
{
    use tokio::io::AsyncBufReadExt;
    use tokio::io::AsyncWriteExt;

    let mut lines = tokio::io::BufReader::new(tokio::io::stdin()).lines();
    let mut repl_session = Vec::new();
    let mut stdout = tokio::io::stdout();
    stdout
        .write_all(PROMPT.as_bytes())
        .await
        .map_err(|error| ConsoleError::Io(error.to_string()))?;
    stdout
        .flush()
        .await
        .map_err(|error| ConsoleError::Io(error.to_string()))?;
    loop {
        let line = if let Some(rx) = refresh_rx.as_mut() {
            tokio::select! {
                line = lines.next_line() => {
                    line.map_err(|error| ConsoleError::Io(error.to_string()))?
                }
                refresh = rx.recv() => {
                    match refresh {
                        Some(Ok(session)) => {
                            theme_session = session.into();
                        }
                        Some(Err(error)) => {
                            eprintln!("Session could not be refreshed: {error}");
                        }
                        None => {
                            refresh_rx = None;
                        }
                    }
                    continue;
                }
            }
        } else {
            lines
                .next_line()
                .await
                .map_err(|error| ConsoleError::Io(error.to_string()))?
        };

        let Some(line) = line else {
            break;
        };
        if has_delimiter(&line) {
            eprintln!("{DELIMITER_WARNING}");
            stdout
                .write_all(PROMPT.as_bytes())
                .await
                .map_err(|error| ConsoleError::Io(error.to_string()))?;
            stdout
                .flush()
                .await
                .map_err(|error| ConsoleError::Io(error.to_string()))?;
            continue;
        }
        let mut config = EvaluationConfig {
            theme_session: theme_session.clone(),
            theme_id: theme_id.clone(),
            url: url.clone(),
            repl_session,
            snippet: line,
            diagnostics: Vec::new(),
        };
        let value = evaluate(renderer, &mut config).await?;
        for diagnostic in &config.diagnostics {
            eprintln!("{diagnostic}");
        }
        repl_session = config.repl_session;
        eprintln!("{}", format_presented_value(present_value(value)));
        stdout
            .write_all(PROMPT.as_bytes())
            .await
            .map_err(|error| ConsoleError::Io(error.to_string()))?;
        stdout
            .flush()
            .await
            .map_err(|error| ConsoleError::Io(error.to_string()))?;
    }
    Ok(())
}

impl Serialize for SessionItem {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serde_json::json!({
            "type": self.r#type,
            "value": self.value,
        })
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for SessionItem {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = Value::deserialize(deserializer)?;
        Ok(Self {
            r#type: value
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            value: value
                .get("value")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::sync::Mutex;

    struct MockRenderer {
        responses: Mutex<VecDeque<RenderResponse>>,
        fallback_response: Option<RenderResponse>,
        requests: Mutex<Vec<RenderContext>>,
    }

    impl MockRenderer {
        fn new(responses: Vec<RenderResponse>) -> Self {
            let fallback_response = responses.last().cloned();
            Self {
                responses: Mutex::new(responses.into()),
                fallback_response,
                requests: Mutex::new(Vec::new()),
            }
        }
    }

    #[async_trait]
    impl StorefrontRenderer for MockRenderer {
        async fn render(
            &self,
            _session: &DevServerSession,
            context: RenderContext,
        ) -> Result<RenderResponse, ConsoleError> {
            self.requests.lock().unwrap().push(context);
            self.responses
                .lock()
                .unwrap()
                .pop_front()
                .or_else(|| self.fallback_response.clone())
                .ok_or_else(|| ConsoleError::Abort("No mock response configured".into()))
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

    fn response(status: u16, body: &str) -> RenderResponse {
        RenderResponse {
            status,
            headers: BTreeMap::new(),
            body: body.into(),
        }
    }

    fn config(snippet: &str) -> EvaluationConfig {
        EvaluationConfig {
            theme_session: session(),
            theme_id: "123".into(),
            url: "/".into(),
            repl_session: Vec::new(),
            snippet: snippet.into(),
            diagnostics: Vec::new(),
        }
    }

    #[test]
    fn repl_assets_match_upstream_seed_theme() {
        let assets = repl_theme_assets();
        assert_eq!(
            assets
                .iter()
                .map(|asset| asset.key.as_str())
                .collect::<Vec<_>>(),
            vec![
                "config/settings_data.json",
                "config/settings_schema.json",
                "snippets/eval.liquid",
                "layout/password.liquid",
                "layout/theme.liquid",
                "sections/announcement-bar.liquid",
                "templates/index.json",
            ]
        );
        assert_eq!(
            assets[3].value.as_deref(),
            Some("{{ content_for_header }}{{ content_for_layout }}")
        );
    }

    #[test]
    fn detects_only_leading_liquid_delimiters() {
        assert!(has_delimiter("{{ collections.first }}"));
        assert!(has_delimiter("{%"));
        assert!(!has_delimiter("\"{{ collections.first }}\""));
    }

    #[test]
    fn presents_null_json_and_json_error_warning() {
        assert_eq!(present_value(None), PresentedValue::Null);
        assert_eq!(present_value(Some(Value::Null)), PresentedValue::Null);
        assert_eq!(
            present_value(Some(serde_json::json!({"foo": "bar"}))),
            PresentedValue::Json(serde_json::json!({"foo": "bar"}))
        );
        assert_eq!(
            present_value(Some(
                serde_json::json!({"error": "json not allowed for this object"})
            )),
            PresentedValue::Warning(JSON_ERROR_WARNING.into())
        );
    }

    #[test]
    fn storefront_replace_template_params_match_upstream_encoding() {
        let mut context = RenderContext {
            method: "GET".into(),
            path: "/products/1".into(),
            query: Vec::new(),
            theme_id: "123".into(),
            section_id: Some("announcement-bar".into()),
            app_block_id: None,
            headers: BTreeMap::new(),
            replace_templates: BTreeMap::new(),
        };
        context.replace_templates.insert(
            "sections/announcement-bar.liquid".into(),
            "<h1>Content</h1>".into(),
        );

        assert_eq!(
            storefront_replace_templates_params(&context),
            "replace_templates%5Bsections%2Fannouncement-bar.liquid%5D=%3Ch1%3EContent%3C%2Fh1%3E&_method=GET"
        );
    }

    fn render_context() -> RenderContext {
        RenderContext {
            method: "GET".into(),
            path: "/products/1".into(),
            query: Vec::new(),
            theme_id: "123".into(),
            section_id: None,
            app_block_id: None,
            headers: BTreeMap::new(),
            replace_templates: BTreeMap::new(),
        }
    }

    #[test]
    fn render_url_matches_storefront_renderer_query_order() {
        let mut context = render_context();
        context.query = vec![("value".into(), "A".into()), ("value".into(), "B".into())];

        assert_eq!(
            build_render_url(&session(), &context).unwrap().as_str(),
            "https://store.myshopify.com/products/1?_fd=0&pb=0&value=A&value=B"
        );

        context.section_id = Some("sections--1__announcement-bar".into());
        context.app_block_id = Some("00001111222233334444".into());
        assert_eq!(
            build_render_url(&session(), &context).unwrap().as_str(),
            "https://store.myshopify.com/products/1?_fd=0&pb=0&value=A&value=B&section_id=sections--1__announcement-bar"
        );

        context.section_id = None;
        assert_eq!(
            build_render_url(&session(), &context).unwrap().as_str(),
            "https://store.myshopify.com/products/1?_fd=0&pb=0&value=A&value=B&app_block_id=00001111222233334444"
        );
    }

    #[test]
    fn render_url_uses_theme_access_base() {
        let mut session = session();
        session.token = "shptka_admin".into();
        session.theme_access_domain = Some("theme-kit-access.shopifyapps.com".into());

        assert_eq!(
            build_render_url(&session, &render_context())
                .unwrap()
                .as_str(),
            "https://theme-kit-access.shopifyapps.com/cli/sfr/products/1?_fd=0&pb=0"
        );
    }

    #[test]
    fn render_response_headers_preserve_json_content_type_only() {
        let json = HeaderMap::from_iter([
            (
                HeaderName::from_static("content-type"),
                HeaderValue::from_static("application/json"),
            ),
            (
                "something".parse().unwrap(),
                HeaderValue::from_static("else"),
            ),
        ]);
        let html = HeaderMap::from_iter([
            (
                HeaderName::from_static("content-type"),
                HeaderValue::from_static("text/html"),
            ),
            (
                "something".parse().unwrap(),
                HeaderValue::from_static("else"),
            ),
        ]);

        assert_eq!(
            render_response_headers(&json)
                .get("content-type")
                .map(String::as_str),
            Some("application/json")
        );
        assert_eq!(
            render_response_headers(&json)
                .get("something")
                .map(String::as_str),
            Some("else")
        );
        assert!(!render_response_headers(&html).contains_key("content-type"));
        assert_eq!(
            render_response_headers(&html)
                .get("something")
                .map(String::as_str),
            Some("else")
        );
    }

    #[test]
    fn standard_headers_clean_and_merge_cookies() {
        let mut session = session();
        session
            .session_cookies
            .insert("_shopify_essential".into(), "session".into());
        let headers = BTreeMap::from([
            ("cookie".into(), "theme_cookie=abc".into()),
            ("authorization".into(), "Basic bad".into()),
            ("X-Special-Header".into(), "200".into()),
        ]);

        let result = build_standard_headers(&session, &headers);

        assert_eq!(
            result.get("User-Agent").map(String::as_str),
            Some(USER_AGENT)
        );
        assert_eq!(
            result.get("Authorization").map(String::as_str),
            Some("Bearer storefront")
        );
        assert_eq!(
            result.get("Cookie").map(String::as_str),
            Some("_shopify_essential=session; theme_cookie=abc")
        );
        assert_eq!(
            result.get("X-Special-Header").map(String::as_str),
            Some("200")
        );
        assert!(!result.contains_key("cookie"));
        assert!(!result.contains_key("authorization"));
    }

    #[test]
    fn theme_access_headers_filter_custom_headers() {
        let mut session = session();
        session.token = "shptka_admin".into();
        session.theme_access_domain = Some("theme-kit-access.shopifyapps.com".into());
        let headers = BTreeMap::from([
            ("Accept".into(), "text/html".into()),
            ("Content-Length".into(), "100".into()),
            ("X-Special-Header".into(), "200".into()),
            ("Cookie".into(), "theme_cookie=abc".into()),
        ]);

        let result = build_theme_access_headers(&session, &headers);

        assert_eq!(result.get("Accept").map(String::as_str), Some("text/html"));
        assert_eq!(
            result.get("Content-Length").map(String::as_str),
            Some("100")
        );
        assert_eq!(
            result.get("User-Agent").map(String::as_str),
            Some(USER_AGENT)
        );
        assert_eq!(
            result.get("X-Shopify-Shop").map(String::as_str),
            Some("store.myshopify.com")
        );
        assert_eq!(
            result.get("X-Shopify-Access-Token").map(String::as_str),
            Some("shptka_admin")
        );
        assert_eq!(
            result.get("Authorization").map(String::as_str),
            Some("Bearer storefront")
        );
        assert_eq!(
            result.get("Cookie").map(String::as_str),
            Some("theme_cookie=abc")
        );
        assert!(!result.contains_key("X-Special-Header"));
    }

    #[tokio::test]
    async fn repl_outputs_prompt_after_each_handled_input() {
        let renderer = MockRenderer::new(vec![response(
            200,
            "<div>\n[{ \"type\": \"display\", \"value\": 1 }]\n</div>",
        )]);
        let mut output = Vec::new();

        run_repl(
            &renderer,
            session(),
            "123".into(),
            "/".into(),
            "{{ bad }}\nshop.id\n".as_bytes(),
            &mut output,
        )
        .await
        .unwrap();

        let output = String::from_utf8(output).unwrap();
        assert!(output.starts_with(PROMPT));
        assert!(output.contains(DELIMITER_WARNING));
        assert!(output.ends_with(PROMPT));
    }

    #[tokio::test]
    async fn evaluates_display_result() {
        let renderer = MockRenderer::new(vec![response(
            200,
            "<div>\n[{ \"type\": \"display\", \"value\": 123123 }]\n</div>",
        )]);
        let mut config = config("shop.id");

        let result = evaluate(&renderer, &mut config).await.unwrap();

        assert_eq!(result, Some(serde_json::json!(123123)));
    }

    #[tokio::test]
    async fn prefixes_url_and_sends_replacement_templates() {
        let renderer = MockRenderer::new(vec![response(
            200,
            "<div>\n[{ \"type\": \"display\", \"value\": 1 }]\n</div>",
        )]);
        let mut config = config("shop.id");
        config.url = "products/foo".into();

        let _ = evaluate(&renderer, &mut config).await.unwrap();

        let requests = renderer.requests.lock().unwrap();
        let request = &requests[0];
        assert_eq!(request.path, "/products/foo");
        assert_eq!(request.section_id.as_deref(), Some("announcement-bar"));
        assert_eq!(
            request
                .replace_templates
                .get("sections/announcement-bar.liquid")
                .map(String::as_str),
            Some("{% render 'eval' %}")
        );
        assert!(request
            .replace_templates
            .get("snippets/eval.liquid")
            .unwrap()
            .contains(r#""type": "display""#));
    }

    #[tokio::test]
    async fn adds_successful_assignments_to_session() {
        let renderer = MockRenderer::new(vec![
            response(
                200,
                "<div>\nLiquid syntax error (snippets/eval line 1): bad\n</div>",
            ),
            response(
                200,
                "<div>\n[{ \"type\": \"context\", \"value\": \"assign x = 1\" }]\n</div>",
            ),
        ]);
        let mut config = config("assign x = 1");

        let result = evaluate(&renderer, &mut config).await.unwrap();

        assert_eq!(result, None);
        assert_eq!(
            config.repl_session,
            vec![SessionItem {
                r#type: "context".into(),
                value: "{% assign x = 1 %}".into(),
            }]
        );
    }

    #[tokio::test]
    async fn translates_smart_assignment() {
        let renderer = MockRenderer::new(vec![
            response(
                200,
                "<div>\nLiquid syntax error (snippets/eval line 1): bad\n</div>",
            ),
            response(
                200,
                "<div>\nLiquid syntax error (snippets/eval line 1): Unknown tag 'x'\n</div>",
            ),
            response(
                200,
                "<div>\n[{ \"type\": \"context\", \"value\": \"\" }]\n</div>",
            ),
        ]);
        let mut config = config("x = 1");

        let result = evaluate(&renderer, &mut config).await.unwrap();

        assert_eq!(result, None);
        assert_eq!(config.diagnostics, vec!["> assign x = 1"]);
        assert_eq!(
            config.repl_session,
            vec![SessionItem {
                r#type: "context".into(),
                value: "{% assign x = 1 %}".into(),
            }]
        );
    }

    #[tokio::test]
    async fn aborts_for_expired_rate_limited_and_not_found_sessions() {
        for (response, expected) in [
            (
                response(401, "Unauthorized"),
                "Session expired. Please initiate a new one.",
            ),
            (
                response(429, "Too many"),
                "Evaluations limit reached. Try again later.",
            ),
        ] {
            let renderer = MockRenderer::new(vec![response]);
            let mut config = config("shop.id");
            assert_eq!(
                evaluate(&renderer, &mut config)
                    .await
                    .unwrap_err()
                    .to_string(),
                expected
            );
        }

        let mut not_found = response(200, "Not Found");
        not_found
            .headers
            .insert("server-timing".into(), "pageType;desc=\"404\"".into());
        let renderer = MockRenderer::new(vec![not_found]);
        let mut config = config("shop.id");
        assert_eq!(
            evaluate(&renderer, &mut config)
                .await
                .unwrap_err()
                .to_string(),
            "Page not found. Please provide a valid --url value!"
        );
    }
}
