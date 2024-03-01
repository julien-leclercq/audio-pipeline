use anyhow::Context;
use serde_json::json;
use sqlx::SqlitePool;
use tokio::{join, sync::mpsc::channel as mpsc_channel};
use tracing::info;
use url::Url;
use uuid::Uuid;

use crate::models;

mod whisper;
mod youtube;
mod yt_dlp;

/// Describe the pipeline we want to integrate
/// 1 - gather a list of video to be analyzed
/// 2 - get infos about those videos (ids, length)
/// 3 - download videos
/// 4 - pass them to whisper
/// 5 - profit
pub(crate) async fn basic_whole_youtube_channel_pipeline(
    youtube_base_url: Url,
    youtube_authorization: redact::Secret<String>,
    youtube_channel_handle: String,
    whisper_max_concurrent_jobs: usize,
    whisper_model: String,
    whisper_threads: usize,
    work_dir: String,
    db: SqlitePool,
) -> anyhow::Result<()> {
    let pipeline = models::Pipeline {
        id: Uuid::new_v4(),
        kind: String::from("youtube_whole_channel"),
        args: serde_json::to_string(&json!({
            "youtube_channel_handle": youtube_channel_handle,
            "whisper_max_concurrent_jobs": whisper_max_concurrent_jobs,
            "whisper_model": whisper_model,
            "whisper_threads": whisper_threads
        }))
        .expect("could not serialize args")
        .into(),
        status: models::StepStatus::Queued,
        created_at: time::OffsetDateTime::now_utc(),
        updated_at: time::OffsetDateTime::now_utc(),
        finished_at: None,
    };

    pipeline
        .insert(&mut *db.acquire().await.expect("could not acquire db connection"))
        .await
        .context("could not insert pipeline")?;

    let video_ids = youtube::get_channel_video_ids(
        &youtube_base_url,
        &youtube_authorization,
        &youtube_channel_handle,
    )
    .context("could not gather video ids for that channel")?;

    let video_info_join = youtube::video_info_worker(
        db.clone(),
        youtube_base_url,
        youtube_authorization,
        youtube_channel_handle,
        video_ids.clone(),
        pipeline.id,
    );

    info!("spawned youtube downloads task");

    let (youtube_dlp_arg_tx, mut youtube_dlp_arg_rx) =
        mpsc_channel::<String>(whisper_max_concurrent_jobs * 2);
    // experimenting a worker to worker approach
    // let (youtube_dlp_res_tx, mut youtube_dlp_res_rx) =
    //     mpsc_channel::<String>(whisper_max_concurrent_jobs * 2);
    let (whisper_arg_tx, mut whisper_arg_rx) =
        mpsc_channel::<whisper::Arg>(whisper_max_concurrent_jobs);

    let audio_work_dir = audio_workdir(&work_dir);

    let whisper_join = whisper::worker(
        &audio_work_dir,
        &mut whisper_arg_rx,
        whisper::Config {
            work_dir: whisper::whisper_work_dir(&work_dir),
            max_concurrent_jobs: whisper_max_concurrent_jobs,
            threads: whisper_threads,
            model: whisper_model,
        },
        pipeline.id,
        db.clone(),
    );

    info!("started whisper worker");

    let video_download_work_dir = audio_workdir(&work_dir);
    let video_download_join = yt_dlp::download_youtube_video_worker(
        video_download_work_dir,
        &mut youtube_dlp_arg_rx,
        whisper_arg_tx,
        whisper_max_concurrent_jobs * 2,
        pipeline.id,
        db,
    );

    info!("started youtube video download worker");

    let dispatching_loop = async {
        info!("starting dispatching loop");

        for video_id in video_ids {
            info!(video_id, "enqueuing next video");
            youtube_dlp_arg_tx
                .send(video_id)
                .await
                .expect("send video_id")
        }

        info!("every videos are enqueued!");
        drop(youtube_dlp_arg_tx);
    };

    info!("collected all info");

    let ((), (), (), ()) = join!(
        video_info_join,
        dispatching_loop,
        video_download_join,
        whisper_join,
    );

    Ok(())
}

fn audio_workdir(work_dir: &str) -> String {
    format!("{work_dir}/audio_files")
}
