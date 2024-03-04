use anyhow::Context;
use tracing::instrument;

pub use youtube::Video;

#[instrument]
pub(crate) fn get_playlist_id(
    client: &youtube::Client,
    youtube_channel_handle: &str,
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
pub(crate) fn get_playlist_items(
    client: &youtube::Client,
    playlist_id: &String,
    page: &Option<String>,
) -> anyhow::Result<(Vec<String>, Option<String>)> {
    let response = client.get_playlist_items(playlist_id, page)?;

    let next_page_token = response.next_page_token;

    let video_ids = response
        .items
        .into_iter()
        .map(|playlist_item| playlist_item.content_details.video_id)
        .collect();

    Ok((video_ids, next_page_token))
}

#[instrument]
pub(crate) fn get_video_info(
    client: &youtube::Client,
    video_ids: &[String],
) -> anyhow::Result<Vec<Video>> {
    client.get_videos(video_ids)
}
