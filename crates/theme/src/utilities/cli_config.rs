#[derive(Debug, Clone, Default)]
pub struct CliConfigOptions {
    pub verbose: bool,
    pub no_color: bool,
}

impl CliConfigOptions {
    pub fn new(verbose: bool, no_color: bool) -> Self {
        Self { verbose, no_color }
    }
}

pub fn configure_cli_environment(options: &CliConfigOptions) {
    if options.verbose {
        std::env::set_var("SHOPIFY_VERBOSE", "true");
    }

    if options.no_color {
        std::env::set_var("FORCE_COLOR", "0");
        colored::control::set_override(false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verbose_sets_env_var() {
        configure_cli_environment(&CliConfigOptions::new(true, false));
        assert_eq!(std::env::var("SHOPIFY_VERBOSE").unwrap(), "true");
    }

    #[test]
    fn no_color_disables_color_output() {
        configure_cli_environment(&CliConfigOptions::new(false, true));
        assert_eq!(std::env::var("FORCE_COLOR").unwrap(), "0");
        assert!(!colored::control::SHOULD_COLORIZE.should_colorize());
    }

    #[test]
    fn combined_options_set_both() {
        configure_cli_environment(&CliConfigOptions::new(true, true));
        assert_eq!(std::env::var("SHOPIFY_VERBOSE").unwrap(), "true");
        assert_eq!(std::env::var("FORCE_COLOR").unwrap(), "0");
    }
}
