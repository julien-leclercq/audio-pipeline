use anyhow::Context;
use clap::Subcommand;
use tokio::runtime::Runtime;

use crate::{database, pipeline};

#[derive(Subcommand, Debug)]
pub(crate) enum PipelineCommand {
    /// Youtube pipeline
    Youtube {
        youtube_base_url: url::Url,
        youtube_authorization: redact::Secret<String>,
        youtube_channel_handle: String,
        whisper_concurrent_jobs: usize,
        whisper_model: String,
        whisper_threads: usize,
        work_dir: String,
        database_url: String,
    },
}

impl PipelineCommand {
    pub(crate) fn run(self) -> anyhow::Result<()> {
        match self {
            PipelineCommand::Youtube {
                youtube_base_url,
                youtube_authorization,
                youtube_channel_handle,
                whisper_concurrent_jobs,
                whisper_model,
                whisper_threads,
                work_dir,
                database_url,
                ..
            } => {
                tracing_subscriber::fmt().compact().init();

                let rt = Runtime::new()?;
                rt.block_on(start_youtube_pipeline(
                    youtube_base_url,
                    youtube_authorization,
                    youtube_channel_handle,
                    whisper_concurrent_jobs,
                    whisper_model,
                    whisper_threads,
                    work_dir,
                    database_url,
                ))
            }
        }
    }
}

pub(crate) async fn start_youtube_pipeline(
    youtube_base_url: url::Url,
    youtube_authorization: redact::Secret<String>,
    youtube_channel_handle: String,
    whisper_concurrent_jobs: usize,
    whisper_model: String,
    whisper_threads: usize,
    work_dir: String,
    database_url: String,
) -> anyhow::Result<()> {
    let db = database::Config { database_url }
        .connect_db()
        .await
        .context("connect to sqlite db")?;

    pipeline::basic_whole_youtube_channel_pipeline(
        youtube_base_url,
        youtube_authorization,
        youtube_channel_handle,
        whisper_concurrent_jobs,
        whisper_model,
        whisper_threads,
        work_dir,
        db,
    )
    .await
}
