use clap::{Parser, Subcommand};

/// A (hopefully) easy to use CLI to manage audio pipelines
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// To run on first use
    Init,
}

fn main() {
    let _cli = Cli::parse();
}
