CREATE TABLE youtube_videos(
    id TEXT PRIMARY KEY,
    youtube_id TEXT,
    youtube_channel_id TEXT NOT NULL,
    title TEXT NOT NULL,
    description TEXT NOT NULL,
    duration_secs INT NOT NULL,
    created_at TIMESTAMP WITHOUT TIME ZONE,
    updated_at TIMESTAMP WITHOUT TIME ZONE,
    CONSTRAINT youtube_id_unique UNIQUE(youtube_id)
)