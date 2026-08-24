use super::compat::{BridgeArgs, BridgeCommand};
use clap::Subcommand;
use cli_core::command::TopicCommand;
use cli_core::error::CliError;

#[derive(Debug, Subcommand)]
pub enum HydrogenSubcommand {
    Build(BridgeArgs),
    Check(BridgeArgs),
    Codegen(BridgeArgs),
    #[command(name = "customer-account-push")]
    CustomerAccountPush(BridgeArgs),
    #[command(subcommand)]
    Debug(HydrogenDebugSubcommand),
    Deploy(BridgeArgs),
    Dev(BridgeArgs),
    #[command(subcommand)]
    Env(HydrogenEnvSubcommand),
    G(BridgeArgs),
    #[command(subcommand)]
    Generate(HydrogenGenerateSubcommand),
    Init(BridgeArgs),
    Link(BridgeArgs),
    List(BridgeArgs),
    Login(BridgeArgs),
    Logout(BridgeArgs),
    Preview(BridgeArgs),
    Setup(HydrogenSetupArgs),
    Shortcut(BridgeArgs),
    Unlink(BridgeArgs),
    Upgrade(BridgeArgs),
}

#[derive(Debug, Subcommand)]
pub enum HydrogenDebugSubcommand {
    Cpu(BridgeArgs),
}

#[derive(Debug, Subcommand)]
pub enum HydrogenEnvSubcommand {
    List(BridgeArgs),
    Pull(BridgeArgs),
    Push(BridgeArgs),
}

#[derive(Debug, Subcommand)]
pub enum HydrogenGenerateSubcommand {
    Route(BridgeArgs),
    Routes(BridgeArgs),
}

#[derive(Debug, clap::Args)]
pub struct HydrogenSetupArgs {
    #[command(subcommand)]
    pub command: Option<HydrogenSetupSubcommand>,

    #[command(flatten)]
    pub args: BridgeArgs,
}

#[derive(Debug, Subcommand)]
pub enum HydrogenSetupSubcommand {
    Css(BridgeArgs),
    Markets(BridgeArgs),
    Vite(BridgeArgs),
}

#[derive(Debug, clap::Args)]
pub struct HydrogenTopicArgs {
    #[command(subcommand)]
    pub command: HydrogenSubcommand,
}

#[derive(Debug)]
pub enum HydrogenTopic {
    Bridge(BridgeCommand),
}

impl HydrogenTopic {
    fn bridge(command_id: &'static str, args: BridgeArgs) -> Self {
        Self::Bridge(BridgeCommand::new(command_id, args.args))
    }
}

#[async_trait::async_trait]
impl TopicCommand for HydrogenTopic {
    type Args = HydrogenTopicArgs;

    fn from_args(args: Self::Args) -> Self {
        match args.command {
            HydrogenSubcommand::Build(args) => Self::bridge("hydrogen:build", args),
            HydrogenSubcommand::Check(args) => Self::bridge("hydrogen:check", args),
            HydrogenSubcommand::Codegen(args) => Self::bridge("hydrogen:codegen", args),
            HydrogenSubcommand::CustomerAccountPush(args) => {
                Self::bridge("hydrogen:customer-account-push", args)
            }
            HydrogenSubcommand::Debug(HydrogenDebugSubcommand::Cpu(args)) => {
                Self::bridge("hydrogen:debug:cpu", args)
            }
            HydrogenSubcommand::Deploy(args) => Self::bridge("hydrogen:deploy", args),
            HydrogenSubcommand::Dev(args) => Self::bridge("hydrogen:dev", args),
            HydrogenSubcommand::Env(HydrogenEnvSubcommand::List(args)) => {
                Self::bridge("hydrogen:env:list", args)
            }
            HydrogenSubcommand::Env(HydrogenEnvSubcommand::Pull(args)) => {
                Self::bridge("hydrogen:env:pull", args)
            }
            HydrogenSubcommand::Env(HydrogenEnvSubcommand::Push(args)) => {
                Self::bridge("hydrogen:env:push", args)
            }
            HydrogenSubcommand::G(args) => Self::bridge("hydrogen:g", args),
            HydrogenSubcommand::Generate(HydrogenGenerateSubcommand::Route(args)) => {
                Self::bridge("hydrogen:generate:route", args)
            }
            HydrogenSubcommand::Generate(HydrogenGenerateSubcommand::Routes(args)) => {
                Self::bridge("hydrogen:generate:routes", args)
            }
            HydrogenSubcommand::Init(args) => Self::bridge("hydrogen:init", args),
            HydrogenSubcommand::Link(args) => Self::bridge("hydrogen:link", args),
            HydrogenSubcommand::List(args) => Self::bridge("hydrogen:list", args),
            HydrogenSubcommand::Login(args) => Self::bridge("hydrogen:login", args),
            HydrogenSubcommand::Logout(args) => Self::bridge("hydrogen:logout", args),
            HydrogenSubcommand::Preview(args) => Self::bridge("hydrogen:preview", args),
            HydrogenSubcommand::Setup(args) => match args.command {
                Some(HydrogenSetupSubcommand::Css(args)) => {
                    Self::bridge("hydrogen:setup:css", args)
                }
                Some(HydrogenSetupSubcommand::Markets(args)) => {
                    Self::bridge("hydrogen:setup:markets", args)
                }
                Some(HydrogenSetupSubcommand::Vite(args)) => {
                    Self::bridge("hydrogen:setup:vite", args)
                }
                None => Self::bridge("hydrogen:setup", args.args),
            },
            HydrogenSubcommand::Shortcut(args) => Self::bridge("hydrogen:shortcut", args),
            HydrogenSubcommand::Unlink(args) => Self::bridge("hydrogen:unlink", args),
            HydrogenSubcommand::Upgrade(args) => Self::bridge("hydrogen:upgrade", args),
        }
    }

    async fn execute(self) -> Result<(), CliError> {
        match self {
            Self::Bridge(command) => command.run().await,
        }
    }
}
