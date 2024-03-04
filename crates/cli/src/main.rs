use clap::Parser;

mod commands;
mod database;
mod models;
mod pipeline;
mod youtube;

/// A (hopefully) easy to use CLI to manage audio pipelines
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: commands::Command,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    cli.command.run()
}
