use app::services::{
    generate_extension, GenerateExtensionOptions,
};
use app::{load_app, LoadAppOptions};
use cli_core::command::BaseCommand;
use cli_core::error::CliError;
use std::path::PathBuf;

#[derive(Debug)]
pub struct GenerateExtension {
    path: String,
    config: Option<String>,
    name: String,
    extension_type: String,
    template: String,
    flavor: Option<String>,
    local: bool,
    clone_url: Option<String>,
}

impl GenerateExtension {
    pub fn new(
        path: String,
        config: Option<String>,
        name: String,
        extension_type: String,
        template: String,
        flavor: Option<String>,
        local: bool,
        clone_url: Option<String>,
    ) -> Self {
        Self {
            path,
            config,
            name,
            extension_type,
            template,
            flavor,
            local,
            clone_url,
        }
    }
}

#[async_trait::async_trait]
impl BaseCommand for GenerateExtension {
    fn name() -> &'static str {
        "extension"
    }
    fn topic() -> &'static str {
        "app generate"
    }
    fn description() -> &'static str {
        "Generate a new app extension"
    }

    async fn run(&self) -> Result<(), CliError> {
        let app = load_app(LoadAppOptions {
            directory: PathBuf::from(&self.path),
            config_name: self.config.clone(),
            ignore_unknown_extensions: false,
        })
        .map_err(|e| CliError::abort(e.to_string()))?;

        let generated = generate_extension(
            &app,
            GenerateExtensionOptions {
                name: self.name.clone(),
                extension_type: self.extension_type.clone(),
                flavor: self.flavor.clone(),
                template: self.template.clone(),
                local_template: self.local,
                clone_url: self.clone_url.clone(),
            },
        )
        .map_err(|e| CliError::abort(e.to_string()))?;

        println!(
            "Generated {} extension at {}",
            generated.extension_type,
            generated.directory.display()
        );
        Ok(())
    }
}
