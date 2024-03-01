use std::time::Duration;

use anyhow::Context;
use serde::{Deserialize, Deserializer, Serialize};
use tracing::{instrument, Level};
use url::Url;

#[derive(Debug, Clone)]
pub struct Client {
    base_url: Url,
    authorization: redact::Secret<String>,
}

impl Client {
    pub fn new(base_url: &Url, authorization: &redact::Secret<String>) -> Self {
        Self {
            base_url: base_url.clone(),
            authorization: authorization.clone(),
        }
    }

    #[instrument(level = Level::DEBUG)]
    pub fn get_channel_json_value(&self, id: &str) -> Result<serde_json::Value, anyhow::Error> {
        self.get("channels")
            .query("forHandle", &id)
            .query("part", "contentDetails,snippet")
            .call()
            .context("call to the youtube api")?
            .into_json()
            .context("deserializing youtube API response")
    }

    #[instrument( skip(self), level = Level::INFO)]
    pub fn get_playlist_items(
        &self,
        id: &String,
        page: &Option<String>,
    ) -> Result<PlaylistItemsListResponsePayload, anyhow::Error> {
        let req = self.get("playlistItems");

        let req = match page {
            None => req,
            Some(page) => req.query("pageToken", page),
        };

        req.query("part", "snippet,contentDetails")
            .query("maxResults", "50")
            .query("playlistId", &id)
            .call()
            .context("http error")?
            .into_json()
            .context("deserializing youtube API response")
    }

    #[instrument(level = Level::DEBUG)]
    pub fn get_videos(&self, ids: &[String]) -> Result<Vec<Video>, anyhow::Error> {
        #[derive(Deserialize)]
        struct ResponsePayload {
            items: Vec<Video>,
        }

        self.get("videos")
            .query("id", &ids.join(","))
            .query("part", "contentDetails,snippet")
            .call()
            .context("call to the youtube API")?
            .into_json()
            .map(|list: ResponsePayload| list.items)
            .context("deserializing json value to video")
    }

    fn get(&self, path: &str) -> ureq::Request {
        let agent = ureq::agent();

        let url = self.base_url.join(path).unwrap();

        agent
            .get(url.as_str())
            .query("key", &self.authorization.expose_secret())
            .set("Accept", "application/json")
    }
}

/// A partial representation of a YouTube Video from the API
/// (partial as in only fields we are interested in are implemented)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Video {
    pub id: String,
    #[serde(rename = "contentDetails")]
    pub content_details: VideoContentDetails,
    pub snippet: VideoSnippet,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VideoContentDetails {
    #[serde(deserialize_with = "deserialize_content_details_duration")]
    pub duration: Duration,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VideoSnippet {
    pub title: String,
    pub description: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlaylistItemsListResponsePayload {
    pub items: Vec<PlaylistItem>,
    #[serde(rename = "nextPagetoken")]
    pub next_page_token: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlaylistItem {
    #[serde(rename = "contentDetails")]
    pub content_details: PlaylistItemContentDetails,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlaylistItemContentDetails {
    #[serde(rename = "videoId")]
    pub video_id: String,
}

/// This is very custom and since youtube does not accept videos longer than 12 hours (anymore)
/// we won't even try to parse longer durations (good enough until proven otherwise)
fn deserialize_content_details_duration<'de, D>(deserializer: D) -> Result<Duration, D::Error>
where
    D: Deserializer<'de>,
{
    let duration_str = dbg!(String::deserialize(deserializer)?);

    if !duration_str.starts_with("PT") {
        return Err(serde::de::Error::custom(
            "iso 8601 duration must start with 'PT'",
        ));
    }

    let mut duration = Duration::default();

    let duration_str = &duration_str[2..];

    let duration_str = match (duration_str).split_once('H') {
        None => &duration_str,
        Some((hours, duration_str)) => {
            let hours: u64 = dbg!(hours.parse().map_err(serde::de::Error::custom)?);
            duration += Duration::from_secs(hours * 3600);
            duration_str
        }
    };

    let duration_str = match (duration_str).split_once('M') {
        None => duration_str,
        Some((minutes, duration_str)) => {
            let minutes: u64 = minutes.parse().map_err(serde::de::Error::custom)?;
            duration += Duration::from_secs(minutes * 60);
            duration_str
        }
    };

    match (duration_str).split_once('S') {
        None => duration_str,
        Some((seconds, duration_str)) => {
            let seconds = seconds.parse().map_err(serde::de::Error::custom)?;
            duration += Duration::from_secs(seconds);
            duration_str
        }
    };

    Ok(duration)
}
