use cli_core::command::BaseCommand;
use cli_core::error::CliError;

pub const SEARCHABLE_COMMANDS: &[&str] = &[
    "app init",
    "app dev",
    "app build",
    "app deploy",
    "app info",
    "app config link",
    "theme dev",
    "theme push",
    "theme pull",
    "store list",
    "auth login",
    "organization list",
    "cache clear",
    "upgrade",
    "version",
    "help",
];

pub fn search_commands(query: &str) -> Vec<&'static str> {
    let q = query.to_lowercase();
    SEARCHABLE_COMMANDS
        .iter()
        .copied()
        .filter(|c| c.contains(&q))
        .collect()
}

pub struct Search {
    query: String,
}

impl Search {
    pub fn new(query: String) -> Self {
        Self { query }
    }
}

#[async_trait::async_trait]
impl BaseCommand for Search {
    fn name() -> &'static str {
        "search"
    }
    fn topic() -> &'static str {
        ""
    }
    fn description() -> &'static str {
        "Search CLI commands"
    }
    async fn run(&self) -> Result<(), CliError> {
        let hits = search_commands(&self.query);
        if hits.is_empty() {
            println!("No commands matching '{}'.", self.query);
        } else {
            for hit in hits {
                println!("shopify {hit}");
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_app_dev() {
        let hits = search_commands("dev");
        assert!(hits.iter().any(|c| c.contains("app dev")));
        assert!(hits.iter().any(|c| c.contains("theme dev")));
    }

    #[test]
    fn empty_query_lists_all() {
        assert_eq!(search_commands("").len(), SEARCHABLE_COMMANDS.len());
    }
}
