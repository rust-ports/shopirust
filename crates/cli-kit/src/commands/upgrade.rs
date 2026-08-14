use cli_core::command::BaseCommand;
use cli_core::error::CliError;

pub struct Upgrade;

#[async_trait::async_trait]
impl BaseCommand for Upgrade {
    fn name() -> &'static str {
        "upgrade"
    }
    fn topic() -> &'static str {
        ""
    }
    fn description() -> &'static str {
        "Upgrade Shopify CLI"
    }
    async fn run(&self) -> Result<(), CliError> {
        println!(
            "Shopify CLI {} ({})\nTo upgrade a cargo install:\n  cargo install --path crates/cli-kit --force",
            env!("CARGO_PKG_VERSION"),
            crate::util::system::host_npm_platform_arch()
        );
        Ok(())
    }
}
