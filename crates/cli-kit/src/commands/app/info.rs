use app::services::info::{app_info, AppInfoFormat, AppInfoOptions, AppInfoResult};
use cli_core::command::BaseCommand;
use cli_core::error::CliError;
use std::path::PathBuf;

#[derive(Debug)]
pub struct Info {
    path: String,
    config: Option<String>,
    json: bool,
    web_env: bool,
    client_id: Option<String>,
    reset: bool,
}

impl Info {
    pub fn new(
        path: String,
        config: Option<String>,
        json: bool,
        web_env: bool,
        client_id: Option<String>,
        reset: bool,
    ) -> Self {
        Self {
            path,
            config,
            json,
            web_env,
            client_id,
            reset,
        }
    }
}

#[async_trait::async_trait]
impl BaseCommand for Info {
    fn name() -> &'static str {
        "info"
    }

    fn topic() -> &'static str {
        "app"
    }

    fn description() -> &'static str {
        "Print basic information about your app and extensions"
    }

    async fn run(&self) -> Result<(), CliError> {
        let _ = (&self.client_id, self.reset);
        let result = app_info(AppInfoOptions {
            directory: PathBuf::from(&self.path),
            config_name: self.config.clone(),
            format: if self.json {
                AppInfoFormat::Json
            } else {
                AppInfoFormat::Text
            },
            web_env: self.web_env,
        })
        .map_err(|e| CliError::abort(e.to_string()))?;

        match result {
            AppInfoResult::Text(s) => println!("{s}"),
            AppInfoResult::Json(j) => {
                println!("{}", serde_json::to_string_pretty(&j).unwrap_or_default());
            }
        }
        Ok(())
    }
}
