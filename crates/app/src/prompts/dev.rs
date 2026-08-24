//! Dev prompts (update URLs, mkcert).

use super::Prompter;
use crate::error::AppError;
use crate::services::dev::urls::ApplicationUrls;

pub fn prompt_update_urls(
    prompter: &dyn Prompter,
    using_dev_sessions: bool,
    current_app_url: &str,
    current_redirect_urls: &[String],
    new_urls: &ApplicationUrls,
) -> Result<bool, AppError> {
    let _ = (
        using_dev_sessions,
        current_app_url,
        current_redirect_urls,
        new_urls,
    );
    let message = if using_dev_sessions {
        "Have Shopify override your app URLs when running `app dev` against your dev store? This won't affect your app on other stores"
    } else {
        "Have Shopify automatically update your app's URL in order to create a preview experience?"
    };
    prompter.confirm(message)
}

pub fn prompt_generate_certificate(prompter: &dyn Prompter) -> Result<bool, AppError> {
    prompter.confirm("--use-localhost requires a certificate for `localhost`. Generate it now?")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prompts::InjectedPrompter;

    #[test]
    fn update_urls_uses_confirm() {
        let p = InjectedPrompter::new();
        p.push_confirm(true);
        let urls = ApplicationUrls {
            application_url: "https://new.example".into(),
            redirect_url_whitelist: vec![],
            app_proxy: None,
        };
        assert!(prompt_update_urls(&p, true, "https://old", &[], &urls).unwrap());
    }

    #[test]
    fn generate_certificate_confirm() {
        let p = InjectedPrompter::new();
        p.push_confirm(false);
        assert!(!prompt_generate_certificate(&p).unwrap());
    }
}
