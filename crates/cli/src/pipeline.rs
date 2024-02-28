use anyhow::Context;
use std::{
    borrow::Borrow,
    collections::{HashMap, VecDeque},
};
use tokio::{
    join,
    process::Command,
    select,
    sync::mpsc::{channel as mpsc_channel, Receiver, Sender},
    task::JoinSet,
};
use tracing::{error, info, instrument};
use url::Url;
use youtube::Video;

mod whisper;

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
) -> anyhow::Result<()> {
    let client = youtube::Client::new(youtube_base_url, youtube_authorization);
    let playlist_id =
        get_playlist_id(&client, youtube_channel_handle).context("fetching upload playlist id")?;

    info!(playlist_id = playlist_id, "found playlist_id");

    let mut next_page = None;

    let mut video_info_queue = VecDeque::new();
    let mut video_download_queue = VecDeque::new();

    loop {
        let (video_ids, new_next_page) =
            get_playlist_items(&client, &playlist_id, next_page.borrow())
                .context("fetching playlist items page")?;

        info!(next_page = new_next_page, "got playlist items");

        next_page = new_next_page;

        video_ids
            .iter()
            .for_each(|video_id| video_download_queue.push_back(video_id.clone()));

        video_info_queue.push_back(video_ids);

        if next_page.is_none() {
            break;
        }
    }

    info!(
        playlist_len = video_download_queue.len(),
        "finished fetching playlist items"
    );

    let mut video_info = HashMap::new();

    let video_info_join = tokio::spawn(async move {
        info!("started fetching video info");

        for video_ids in video_info_queue {
            info!("fetching video info");
            let info = get_video_info(&client, video_ids).expect("msg");
            info!("fetched video info");
            for info in info {
                video_info.insert(info.id.clone(), info);
            }
        }

        info!("finished fetching video info");
        video_info
    });

    info!("spawned youtube downloads task");

    let (youtube_dlp_arg_tx, mut youtube_dlp_arg_rx) =
        mpsc_channel::<String>(whisper_max_concurrent_jobs * 2);
    // experimenting a worker to worker approach
    // let (youtube_dlp_res_tx, mut youtube_dlp_res_rx) =
    //     mpsc_channel::<String>(whisper_max_concurrent_jobs * 2);
    let (whisper_arg_tx, mut whisper_arg_rx) =
        mpsc_channel::<whisper::Arg>(whisper_max_concurrent_jobs);

    let whisper_work_dir = whisper::whisper_work_dir(&work_dir);
    let audio_work_dir = audio_workdir(&work_dir);

    let whisper_config = whisper::Config {
        work_dir: whisper_work_dir,
        max_concurrent_jobs: whisper_max_concurrent_jobs,
        threads: whisper_threads,
        model: whisper_model,
    };

    let whisper_join = whisper::worker(&audio_work_dir, &mut whisper_arg_rx, whisper_config);

    info!("started whisper worker");

    let video_download_work_dir = audio_workdir(&work_dir);
    let video_download_join = youtube_video_worker(
        video_download_work_dir,
        &mut youtube_dlp_arg_rx,
        whisper_arg_tx,
        whisper_max_concurrent_jobs * 2,
    );

    info!("started youtube video download worker");
    video_download_queue = dbg!(video_download_queue);

    let dispatching_loop = async {
        info!("starting dispatching loop");

        while let Some(video_id) = video_download_queue.pop_front() {
            info!(video_id, "enqueuing next video");
            youtube_dlp_arg_tx
                .send(video_id)
                .await
                .expect("send video_id")
        }

        info!("every videos are enqueued!");
        drop(youtube_dlp_arg_tx);
    };

    let _video_info = video_info_join.await.expect("join video info task");

    info!("collected all info");

    let ((), (), ()) = join!(dispatching_loop, video_download_join, whisper_join,);

    Ok(())
}

#[instrument(skip(youtube_dlp_arg_rx, youtube_dlp_res_tx))]
async fn youtube_video_worker(
    video_download_work_dir: String,
    youtube_dlp_arg_rx: &mut Receiver<String>,
    youtube_dlp_res_tx: Sender<whisper::Arg>,
    youtube_dlp_max_jobs: usize,
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

                info!(video_id, "downloading youtube video");

                let work_dir = video_download_work_dir.clone();
                let youtube_dlp_res_tx = youtube_dlp_res_tx.clone();

                js.spawn(async move {
                    let audio_path = audio_path(&work_dir, &video_id);

                    download_video(&video_id, &audio_path)
                        .await
                        .expect("could not download video");
                    youtube_dlp_res_tx
                        .send(whisper::Arg{video_id, audio_path})
                        .await
                        .expect("could not send result")
                });
            },

            _ = js.join_next() => {},
        }
    }
}

#[instrument]
fn get_playlist_id(
    client: &youtube::Client,
    youtube_channel_handle: String,
) -> anyhow::Result<String> {
    let channel_info = client
        .get_channel_json_value(youtube_channel_handle)
        .context("fetching channel info")?;

    let serde_json::Value::String(val) =
        channel_info["items"][0]["contentDetails"]["relatedPlaylists"]["uploads"].clone()
    else {
        anyhow::bail!("channel id should be a string {channel_info:?}")
    };

    Ok(val)
}

#[instrument]
fn get_playlist_items(
    client: &youtube::Client,
    playlist_id: &String,
    page: &Option<String>,
) -> anyhow::Result<(Vec<String>, Option<String>)> {
    let response = client.get_playlist_items(playlist_id, page)?;

    let next_page_token = match &response["nextPageToken"] {
        serde_json::Value::Null => None,
        serde_json::Value::String(next_page_token) => Some(next_page_token.clone()),
        _ => unreachable!(),
    };

    let video_ids = response["items"]
        .as_array()
        .expect("items should be an array")
        .iter()
        .map(|playlist_item| {
            playlist_item["contentDetails"]["videoId"]
                .as_str()
                .expect(&format!(
                    "videoID should be a string but got {playlist_item:?}"
                ))
                .to_string()
        })
        .collect();

    Ok((video_ids, next_page_token))
}

#[instrument]
fn get_video_info(client: &youtube::Client, video_ids: Vec<String>) -> anyhow::Result<Vec<Video>> {
    client.get_videos(video_ids)
}

fn audio_workdir(work_dir: &str) -> String {
    format!("{work_dir}/audio_files")
}

fn audio_path(audio_work_dir: &str, video_id: &str) -> String {
    format!("{audio_work_dir}/{video_id}.mp3")
}

#[instrument]
async fn download_video(video_id: &str, audio_path: &str) -> anyhow::Result<()> {
    let cmd = Command::new("yt-dlp")
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
