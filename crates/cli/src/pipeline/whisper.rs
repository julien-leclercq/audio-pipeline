use serde::Serialize;
use sqlx::SqlitePool;
use std::process::Stdio;
use time::OffsetDateTime;
use tokio::{
    sync::mpsc::Receiver,
    task::{AbortHandle, JoinSet},
};
use tracing::info;
use uuid::Uuid;

use crate::models::{self, StepStatus};

#[derive(Clone, Debug, Serialize)]
pub(super) struct Arg {
    pub video_id: String,
    pub audio_path: String,
}

#[derive(Clone, Debug)]
pub(super) struct Config {
    pub work_dir: String,
    pub max_concurrent_jobs: usize,
    pub threads: usize,
    pub model: String,
}

#[tracing::instrument(skip(whisper_arg_rx, config))]
pub(super) async fn worker(
    audio_work_dir: &str,
    whisper_arg_rx: &mut Receiver<Arg>,
    config: Config,
    pipeline_id: Uuid,
    db: SqlitePool,
) -> () {
    tracing::info!("starting whisper worker");

    let mut js = JoinSet::new();

    loop {
        tokio::select! {
            _ = js.join_next() => {},

            maybe_video_id = whisper_arg_rx.recv(),  if js.len() < config.max_concurrent_jobs => {
                let Some(arg) = maybe_video_id else {
                    break;
                };

                tracing::info!(arg.video_id, arg.audio_path, "whisper worker video received");

                spawn_whisper_task(arg, &mut js, config.clone(), pipeline_id, db.clone()).await;
            },

            else => {
                info!("whisper worker select else clause");
                break;
            }
        }
    }
}

async fn spawn_whisper_task(
    arg: Arg,
    js: &mut JoinSet<()>,
    config: Config,
    pipeline_id: Uuid,
    db: SqlitePool,
) -> AbortHandle {
    js.spawn(run_whisper(arg, config, pipeline_id, db))
}

async fn run_whisper(arg: Arg, config: Config, pipeline_id: Uuid, db: SqlitePool) {
    let now = OffsetDateTime::now_utc();

    let step = models::Step {
        id: Uuid::new_v4(),
        pipeline_id: pipeline_id,
        name: String::from("whisper"),
        status: StepStatus::Queued,
        arg: serde_json::to_string(&arg).expect("every arg should be serializable"),
        state: "".into(),
        created_at: now,
        updated_at: now,
        finished_at: None,
    };

    step.insert(&mut *db.acquire().await.expect("could not acquire db connection"))
        .await
        .expect("could not write to database");
    let whisper_output_dir = &whisper_output_dir(&config.work_dir, &arg.video_id);

    let child = tokio::process::Command::new("whisper")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .args(["--verbose", "False"])
        .args(["--model", &config.model])
        .args(["--output_dir", whisper_output_dir])
        .args(["--threads", &config.threads.to_string()])
        .arg(arg.audio_path)
        .spawn()
        .expect("failed to spawn whisper command");

    tracing::info!(arg.video_id, "started whisper command");

    let out = child
        .wait_with_output()
        .await
        .expect("could not wait on whisper command to complete");

    info!("whisper command terminated");

    if !out.status.success() {
        tracing::error!(
            arg.video_id,
            whisper_err = String::from_utf8(out.stderr)
                .expect("whisper sent unvalid utf8 to stderr (naughty command)"),
            "whisper task failed"
        );

        let step = models::Step {
            status: StepStatus::Error,
            finished_at: Some(OffsetDateTime::now_utc()),
            ..step
        };

        step.update(&mut *db.acquire().await.expect("could not acquire db connection"))
            .await
            .expect("could not write to database");

        return;
    }

    info!("whisper command successed");
}

pub(super) fn whisper_work_dir(work_dir: &str) -> String {
    format!("{work_dir}/whisper_output")
}

fn whisper_output_dir(whisper_work_dir: &str, video_id: &str) -> String {
    format!("{whisper_work_dir}/{video_id}")
}
