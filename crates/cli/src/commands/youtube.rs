use clap::Subcommand;
use url::Url;

#[derive(Subcommand, Debug)]
pub(crate) enum YoutubeCommand {
    // Retrieve information about a channel
    GetChannel {
        /// The handle for the youtube channel you want to fetch
        handle: String,
        youtube_authorization: redact::Secret<String>,
    },

    GetPlaylistItems {
        playlist_id: String,
        youtube_authorization: redact::Secret<String>,
        page_token: Option<String>,
    },
}

impl YoutubeCommand {
    pub(crate) fn run(self) -> anyhow::Result<()> {
        use YoutubeCommand::*;

        match self {
            GetChannel {
                handle,
                youtube_authorization,
            } => get_channel(youtube_authorization, &handle),
            GetPlaylistItems {
                playlist_id,
                youtube_authorization,
                page_token,
            } => get_playlist_items(youtube_authorization, playlist_id, page_token),
        }
    }
}

fn get_playlist_items(
    youtube_authorization: redact::Secret<String>,
    playlist_id: String,
    page_token: Option<String>,
) -> Result<(), anyhow::Error> {
    let youtube_default_url = Url::parse("https://www.googleapis.com/youtube/v3/").unwrap();

    let playlist_items_payload = youtube::Client::new(&youtube_default_url, &youtube_authorization)
        .get_playlist_items(&playlist_id, &page_token)?;

    println!(
        "playlist_items:\n\tnext_page:\t{:?}",
        playlist_items_payload.next_page_token
    );

    Ok(())
}

fn get_channel(youtube_authorization: redact::Secret<String>, handle: &str) -> anyhow::Result<()> {
    let youtube_default_url = Url::parse("https://www.googleapis.com/youtube/v3/").unwrap();

    let channel_details = youtube::Client::new(&youtube_default_url, &youtube_authorization)
        .get_channel_json_value(handle)?;

    let items = &channel_details["items"];

    for item in items.as_array().unwrap().iter() {
        println!(
            "found channel:\n\thandle:\t{}\n\twith channel_id:\t{}\n\tupload_playlist_id:\t{}",
            item["snippet"]["title"],
            item["id"],
            item["contentDetails"]["relatedPlaylists"]["uploads"]
        )
    }

    Ok(())
}
