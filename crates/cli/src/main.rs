use clap::{Parser, Subcommand};
use tokio::runtime::Runtime;

// mod database;
mod pipeline;

/// A (hopefully) easy to use CLI to manage audio pipelines
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// To run on first use
    Init,

    /// Youtube related operations
    Youtube {
        #[command(subcommand)]
        command: YoutubeCommand,
    },

    /// Youtube pipeline
    YoutubePipeline {
        youtube_base_url: url::Url,
        youtube_authorization: redact::Secret<String>,
        youtube_channel_handle: String,
        whisper_concurrent_jobs: usize,
        whisper_model: String,
        whisper_threads: usize,
        work_dir: String,
    },
}

#[derive(Subcommand, Debug)]
enum YoutubeCommand {
    // Retrieve information about a channel
    GetChannel {
        /// The handle for the youtube channel you want to fetch
        handle: String,
        youtube_authorization: redact::Secret<String>,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init => init(),
        Commands::Youtube { command } => youtube(command),
        Commands::YoutubePipeline {
            youtube_base_url,
            youtube_authorization,
            youtube_channel_handle,
            whisper_concurrent_jobs,
            whisper_model,
            whisper_threads,
            work_dir,
            ..
        } => {
            tracing_subscriber::fmt().compact().init();

            let rt = Runtime::new()?;
            rt.block_on(pipeline::basic_whole_youtube_channel_pipeline(
                youtube_base_url,
                youtube_authorization,
                youtube_channel_handle,
                whisper_concurrent_jobs,
                whisper_model,
                whisper_threads,
                work_dir,
            ))
        }
    }
}

fn init() -> anyhow::Result<()> {
    // let config = todo!("parse config");

    // database::init(config)?;

    todo!("")
}

fn youtube(command: YoutubeCommand) -> anyhow::Result<()> {
    use YoutubeCommand::*;

    match command {
        GetChannel {
            handle,
            youtube_authorization,
        } => youtube_get_channel(youtube_authorization, handle),
    }
}

fn youtube_get_channel(
    youtube_authorization: redact::Secret<String>,
    handle: String,
) -> anyhow::Result<()> {
    use url::Url;

    let youtube_default_url = Url::parse("https://www.googleapis.com/youtube/v3/").unwrap();

    let channel_details = youtube::Client::new(youtube_default_url, youtube_authorization)
        .get_channel_json_value(handle)?;

    let items = &channel_details["items"];

    for item in items.as_array().unwrap().iter() {
        println!(
            "found channel:\n\t{}\n\twith channel_id: {}\n\tupload_playlist_id: {}",
            item["snippet"]["title"],
            item["id"],
            item["contentDetails"]["relatedPlaylists"]["uploads"]
        )
    }

    Ok(())
}
