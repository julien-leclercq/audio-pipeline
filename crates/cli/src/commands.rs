use clap::Subcommand;
use tokio::runtime::Runtime;

use crate::database;

mod pipeline;
mod youtube;

#[derive(Subcommand, Debug)]
pub(crate) enum Command {
    /// To run on first use
    Init { database_url: String },

    /// Youtube related operations
    Youtube {
        #[command(subcommand)]
        command: youtube::YoutubeCommand,
    },

    /// Pipeline related commands
    Pipeline {
        #[command(subcommand)]
        command: pipeline::PipelineCommand,
    },
}

impl Command {
    pub(crate) fn run(self) -> anyhow::Result<()> {
        match self {
            Self::Init { database_url } => init(database_url),
            Self::Youtube { command } => command.run(),
            Self::Pipeline { command } => command.run(),
        }
    }
}

fn init(database_url: String) -> anyhow::Result<()> {
    tracing_subscriber::fmt().compact().init();

    let config = database::Config { database_url };

    let rt = Runtime::new()?;

    rt.block_on(database::init(config))
}
