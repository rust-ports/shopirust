use cli_core::command::BaseCommand;
use cli_core::error::CliError;

/// Hidden demo command (upstream kitchen-sink).
pub struct KitchenSink;

#[async_trait::async_trait]
impl BaseCommand for KitchenSink {
    fn name() -> &'static str {
        "kitchen-sink"
    }
    fn topic() -> &'static str {
        ""
    }
    fn description() -> &'static str {
        "Render a sample of CLI UI primitives"
    }
    async fn run(&self) -> Result<(), CliError> {
        println!("Shopify CLI kitchen sink");
        println!("- info: ready");
        println!("- success: ok");
        println!("- warning: check your config");
        Ok(())
    }
}
