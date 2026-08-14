#[derive(Debug, Clone, clap::Args)]
pub struct GlobalFlags {
    #[arg(
        short = 'v',
        long = "verbose",
        global = true,
        help = "Increase verbosity"
    )]
    pub verbose: bool,

    #[arg(
        long = "no-color",
        alias = "no-colour",
        global = true,
        help = "Disable color output"
    )]
    pub no_color: bool,

    #[arg(
        short = 'p',
        long = "path",
        global = true,
        help = "Path to the project directory"
    )]
    pub path: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[derive(Debug, clap::Parser)]
    struct TestCli {
        #[command(flatten)]
        global: GlobalFlags,
    }

    #[test]
    fn test_verbose_flag() {
        let cli = TestCli::parse_from(["test", "--verbose"]);
        assert!(cli.global.verbose);
    }

    #[test]
    fn test_verbose_short() {
        let cli = TestCli::parse_from(["test", "-v"]);
        assert!(cli.global.verbose);
    }

    #[test]
    fn test_no_color_flag() {
        let cli = TestCli::parse_from(["test", "--no-color"]);
        assert!(cli.global.no_color);
    }

    #[test]
    fn test_no_color_alias() {
        let cli = TestCli::parse_from(["test", "--no-colour"]);
        assert!(cli.global.no_color);
    }

    #[test]
    fn test_path_flag() {
        let cli = TestCli::parse_from(["test", "--path", "/some/project"]);
        assert_eq!(cli.global.path.as_deref(), Some("/some/project"));
    }

    #[test]
    fn test_path_short() {
        let cli = TestCli::parse_from(["test", "-p", "/some/project"]);
        assert_eq!(cli.global.path.as_deref(), Some("/some/project"));
    }

    #[test]
    fn test_all_flags_default_false() {
        let cli = TestCli::parse_from(["test"]);
        assert!(!cli.global.verbose);
        assert!(!cli.global.no_color);
        assert!(cli.global.path.is_none());
    }
}
