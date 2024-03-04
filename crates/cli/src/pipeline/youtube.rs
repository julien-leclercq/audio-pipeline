use anyhow::{anyhow, Context};
use serde_json::json;
use sqlx::types::time::OffsetDateTime;
use tracing::info;
use url::Url;
use uuid::Uuid;
use youtube::Client;

use crate::{
    models::{self, StepStatus},
    youtube::{get_playlist_id, get_playlist_items, get_video_info},
};

#[tracing::instrument(skip(db, youtube_ids))]
pub(crate) async fn video_info_worker(
    db: sqlx::SqlitePool,
    youtube_base_url: Url,
    youtube_authorization: redact::Secret<String>,
    youtube_channel_handle: String,
    youtube_ids: Vec<String>,
    pipeline_id: uuid::Uuid,
) -> () {
    info!("started fetching video info");

    let client = Client::new(&youtube_base_url, &youtube_authorization);

    let video_info_chunks = (youtube_ids).chunks(50);

    for video_ids in video_info_chunks {
        let steps: Vec<_> = {
            let now = OffsetDateTime::now_utc();

            let mut conn = db.acquire().await.expect("acquiring database conn");
            let step_iter = video_ids.iter().map(|video_id| models::Step {
                id: Uuid::new_v4(),
                pipeline_id: pipeline_id,
                name: String::from("video_info"),
                status: StepStatus::Queued,
                arg: json!({"video_id": video_id}).to_string(),
                state: "".into(),
                created_at: now.clone(),
                updated_at: now.clone(),
                finished_at: None,
            });

            for step in step_iter.clone() {
                step.insert(&mut *conn).await.expect("insert step")
            }

            step_iter.collect()
        };

        info!("fetching video info");
        let info = get_video_info(&client, video_ids).expect("msg");
        info!("fetched video info");

        for info in info {
            let youtube_video = models::YoutubeVideo {
                id: uuid::Uuid::new_v4(),
                youtube_id: info.id,
                youtube_channel_id: youtube_channel_handle.clone(),
                title: info.snippet.title,
                description: info.snippet.description,
                duration_secs: info
                    .content_details
                    .duration
                    .as_secs()
                    .try_into()
                    .expect("youtube durations should never exceed u32::MAX"),
                created_at: OffsetDateTime::now_utc(),
                updated_at: OffsetDateTime::now_utc(),
            };

            let mut conn = db.acquire().await.expect("could not acquire sqlite conn");

            youtube_video
                .insert(&mut *conn)
                .await
                .or_else(|root_err| {
                    let err = root_err
                        .downcast::<sqlx::Error>()
                        .context("expected a sqlx error")?;

                    match err {
                        sqlx::Error::Database(database_err)
                            if database_err.is_unique_violation() =>
                        {
                            Ok(())
                        }
                        err => Err(anyhow!(err)),
                    }
                })
                .expect("insert youtube info to db");
        }

        let steps = steps.into_iter().map(|step| models::Step {
            status: StepStatus::Processed,
            finished_at: Some(OffsetDateTime::now_utc()),
            ..step
        });

        let mut conn = db.acquire().await.expect("acquiring database conn");

        for step in steps {
            step.update(&mut *conn)
                .await
                .expect("could not write to database")
        }
    }

    info!("finished fetching video info");
}

pub(crate) fn get_channel_video_ids(
    youtube_base_url: &Url,
    youtube_authorization: &redact::Secret<String>,
    youtube_channel_handle: &str,
) -> anyhow::Result<Vec<String>> {
    let client = youtube::Client::new(youtube_base_url, youtube_authorization);
    let playlist_id =
        get_playlist_id(&client, &youtube_channel_handle).context("fetching upload playlist id")?;

    info!(playlist_id = playlist_id, "found playlist_id");

    let mut next_page = None;

    let mut res_video_ids = Vec::new();

    loop {
        let (video_ids, new_next_page) = get_playlist_items(&client, &playlist_id, &next_page)
            .context("fetching playlist items page")?;

        info!(next_page = new_next_page, "got playlist items");

        next_page = new_next_page;

        video_ids
            .iter()
            .for_each(|video_id| res_video_ids.push(video_id.clone()));

        if next_page.is_none() {
            break;
        }
    }

    Ok(res_video_ids)
}
