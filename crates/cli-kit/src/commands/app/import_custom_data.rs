use app::services::import_custom_data_from_json_file;
use app::{load_app, LoadAppOptions};
use cli_core::command::BaseCommand;
use cli_core::error::CliError;
use std::path::PathBuf;

#[derive(Debug)]
pub struct ImportCustomDataDefinitions {
    path: String,
    config: Option<String>,
    /// JSON dump of metafield/metaobject definitions.
    definitions_file: String,
    include_existing: bool,
}

impl ImportCustomDataDefinitions {
    pub fn new(
        path: String,
        config: Option<String>,
        definitions_file: String,
        include_existing: bool,
    ) -> Self {
        Self {
            path,
            config,
            definitions_file,
            include_existing,
        }
    }
}

#[async_trait::async_trait]
impl BaseCommand for ImportCustomDataDefinitions {
    fn name() -> &'static str {
        "import-custom-data-definitions"
    }
    fn topic() -> &'static str {
        "app"
    }
    fn description() -> &'static str {
        "Import metafield and metaobject definitions into the app TOML"
    }

    async fn run(&self) -> Result<(), CliError> {
        let app = load_app(LoadAppOptions {
            directory: PathBuf::from(&self.path),
            config_name: self.config.clone(),
            ignore_unknown_extensions: true,
        })
        .map_err(|e| CliError::abort(e.to_string()))?;

        let result = import_custom_data_from_json_file(
            &app.configuration_path,
            PathBuf::from(&self.definitions_file).as_path(),
            self.include_existing,
        )
        .map_err(|e| CliError::abort(e.to_string()))?;

        println!(
            "Imported {} metafield(s) and {} metaobject(s) into {}",
            result.metafield_count,
            result.metaobject_count,
            app.configuration_path.display()
        );
        Ok(())
    }
}
