use anyhow::Context;
use serde_json::json;
use sqlx::SqlitePool;
use std::process::Stdio;
use time::OffsetDateTime;
use tokio::{
    process::Command,
    select,
    sync::mpsc::{Receiver, Sender},
    task::JoinSet,
};
use tracing::{error, info, instrument};
use uuid::Uuid;

use crate::models::{self, StepStatus};

#[instrument]
pub(crate) async fn download_video(video_id: &str, audio_path: &str) -> anyhow::Result<()> {
    let cmd = Command::new("yt-dlp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .arg("--extract-audio")
        .args(["--audio-format", "mp3"])
        .args(["-o", audio_path])
        .arg(format!("https://www.youtube.com/watch?v={video_id}"))
        .spawn()
        .context("spawning yt-dlp command")?;

    info!(video_id, audio_path, "started downloading video");

    let out = cmd
        .wait_with_output()
        .await
        .context("io error in yt-dlp call")?;

    if !out.status.success() {
        error!(
            video_id,
            audio_path,
            stderr = String::from_utf8(out.stderr).expect("yt-dlp sent invalid utf8 to stderr"),
            "failed to download audio from youtube video"
        );

        anyhow::bail!("yt-dlp failed")
    };

    info!(video_id, audio_path, "downloaded audio from youtube video");

    Ok(())
}

#[instrument(skip(youtube_dlp_arg_rx, youtube_dlp_res_tx))]
pub(crate) async fn download_youtube_video_worker(
    video_download_work_dir: String,
    youtube_dlp_arg_rx: &mut Receiver<String>,
    youtube_dlp_res_tx: Sender<super::whisper::Arg>,
    youtube_dlp_max_jobs: usize,
    pipeline_id: uuid::Uuid,
    db: SqlitePool,
) -> () {
    info!("starting youtube video_worker ");
    let mut js = JoinSet::new();

    loop {
        select! {
            arg = youtube_dlp_arg_rx.recv(), if js.len() < youtube_dlp_max_jobs => {
                let Some(video_id ) = arg else {
                    info!("received None arg, draining the joinset");
                    while let Some(_) = js.join_next().await {}
                    info!("joinset drained");
                    break;
                };

                spawn_video_download(video_id, video_download_work_dir.clone(), youtube_dlp_res_tx.clone(), &mut js, pipeline_id, db.clone()).await

            },

            _ = js.join_next() => {},
        }
    }
}

pub(crate) async fn spawn_video_download(
    video_id: String,
    video_download_work_dir: String,
    youtube_dlp_res_tx: Sender<super::whisper::Arg>,
    js: &mut JoinSet<()>,
    pipeline_id: uuid::Uuid,
    db: SqlitePool,
) -> () {
    info!(video_id, "downloading youtube video");

    js.spawn(async move {
        let step = {
            let now = OffsetDateTime::now_utc();

            models::Step {
                id: Uuid::new_v4(),
                pipeline_id: pipeline_id,
                name: String::from("video_download"),
                status: StepStatus::Queued,
                arg: json!({"video_id": video_id}).to_string(),
                state: "".into(),
                created_at: now,
                updated_at: now,
                finished_at: None,
            }
        };

        step.insert(&mut *db.acquire().await.expect("could not acquire db connection"))
            .await
            .expect("could not write to database");

        let audio_path = audio_path(&video_download_work_dir, &video_id);

        download_video(&video_id, &audio_path)
            .await
            .expect("could not download video");
        youtube_dlp_res_tx
            .send(super::whisper::Arg {
                video_id,
                audio_path,
            })
            .await
            .expect("could not send result");

        {
            let step = models::Step {
                status: StepStatus::Queued,
                finished_at: Some(OffsetDateTime::now_utc()),
                ..step
            };

            step.update(&mut *db.acquire().await.expect("could not acquire db connection"))
                .await
                .expect("could not write to database")
        }
    });
}

fn audio_path(audio_work_dir: &str, video_id: &str) -> String {
    format!("{audio_work_dir}/{video_id}.mp3")
}
