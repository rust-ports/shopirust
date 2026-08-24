use super::registry;
use cli_core::command::BaseCommand;
use cli_core::error::CliError;

pub fn searchable_commands() -> Vec<String> {
    registry::visible_command_ids()
        .map(|id| id.replace(':', " "))
        .collect()
}

pub fn search_commands(query: &str) -> Vec<String> {
    let q = query.to_lowercase();
    searchable_commands()
        .into_iter()
        .filter(|command| command.contains(&q))
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
    fn finds_visible_hydrogen_commands() {
        let hits = search_commands("hydrogen dev");
        assert_eq!(hits, vec!["hydrogen dev"]);
    }

    #[test]
    fn empty_query_lists_all_visible_registry_commands() {
        assert_eq!(
            search_commands("").len(),
            registry::visible_command_ids().count()
        );
    }

    #[test]
    fn hidden_commands_are_not_searchable() {
        let hits = search_commands("doctor-release");
        assert!(hits.is_empty());
    }

    #[test]
    fn visible_plugin_subcommands_are_searchable() {
        let hits = search_commands("plugins install");
        assert_eq!(hits, vec!["plugins install"]);
    }
}
