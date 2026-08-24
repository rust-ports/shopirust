//! Liquid HTML templates for UI extension preview (post-purchase, errors).

use crate::error::AppError;
use crate::utilities::liquid::render_liquid_template;
use serde_json::json;

const POST_PURCHASE_INDEX: &str = r#"<html>
  <head>
  <meta name="viewport" content="width=device-width, initial-scale=1.0, height=device-height, viewport-fit=cover">
  <style>
      * { box-sizing: border-box; }
      body {
      min-height: 100vh;
      display: grid;
      margin: 0;
      align-content: center;
      justify-content: center;
      font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
      }
      html { background: black; color: white; }
      .content { max-width: 40rem; padding: 2rem; font-size: 1.5em; }
      code { color: mediumseagreen; }
  </style>
  </head>
  <body>
  <div class="content">
      <p>This page is served by your local UI Extension development server. Instead of visiting this page directly, you will need to connect your local development environment to a real checkout environment.<br>
      <br>
      If this is the first time you're testing a Post Purchase extension, please install the browser extension from <a href="https://github.com/Shopify/post-purchase-devtools/releases">https://github.com/Shopify/post-purchase-devtools/releases</a>.<br>
      <br>
      Once installed, simply enter your extension URL <a href="{{ url }}">{{ url }}</a>.</p>
  </div>
  </body>
</html>"#;

const GENERIC_ERROR: &str =
    r#"<html><body><h1>Extension error</h1><p>{{ message }}</p></body></html>"#;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewTemplate {
    Index,
    Error,
    TunnelError,
}

/// Render preview HTML for an extension surface.
pub fn get_html(
    extension_surface: Option<&str>,
    template: PreviewTemplate,
    url: &str,
    message: Option<&str>,
) -> Result<String, AppError> {
    let raw = match (extension_surface, template) {
        (Some("post_purchase"), PreviewTemplate::Index) => POST_PURCHASE_INDEX,
        (_, PreviewTemplate::Error | PreviewTemplate::TunnelError) => GENERIC_ERROR,
        _ => POST_PURCHASE_INDEX,
    };
    render_liquid_template(
        raw,
        &json!({
            "url": url,
            "message": message.unwrap_or(""),
        }),
    )
    .map_err(|e| AppError::message(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn post_purchase_embeds_url() {
        let html = get_html(
            Some("post_purchase"),
            PreviewTemplate::Index,
            "https://example.trycloudflare.com/extensions/abc",
            None,
        )
        .unwrap();
        assert!(html.contains("https://example.trycloudflare.com/extensions/abc"));
        assert!(html.contains("post-purchase-devtools"));
        assert!(!html.contains("post_purchase stub"));
    }

    #[test]
    fn error_template_embeds_message() {
        let html = get_html(None, PreviewTemplate::Error, "", Some("boom")).unwrap();
        assert!(html.contains("boom"));
    }
}
