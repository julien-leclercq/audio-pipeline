use anyhow::Context;
use sqlx::{query, Executor, Sqlite};
use time::OffsetDateTime;
use tracing::instrument;
use uuid::Uuid;

/// Model for the `youtube_videos` table
#[derive(Debug, Clone)]
pub(crate) struct YoutubeVideo {
    pub id: Uuid,
    pub youtube_id: String,
    pub youtube_channel_id: String,
    pub title: String,
    pub description: String,
    pub duration_secs: u32,
    pub created_at: time::OffsetDateTime,
    pub updated_at: time::OffsetDateTime,
}

impl YoutubeVideo {
    #[instrument(skip(conn))]
    pub(crate) async fn insert<'c>(
        &self,
        conn: impl Executor<'c, Database = Sqlite>,
    ) -> anyhow::Result<()> {
        let yt_video_id = self.id.to_string();

        query!(
            r#"
            INSERT INTO youtube_videos (id, youtube_id, youtube_channel_id, title, description, duration_secs, created_at, updated_at)
            VALUES ($1,$2,$3,$4,$5,$6,$7, $8)
            "#,
            yt_video_id,
            self.youtube_id,
            self.youtube_channel_id,
            self.title,
            self.description,
            self.duration_secs,
            self.created_at,
            self.updated_at,
        )
        .execute(conn).await.context("could not write youtube info to sqlite db").map(|_| ())
    }
}

#[derive(sqlx::Type, Debug, Clone)]
pub enum StepStatus {
    Queued,
    Processed,
    Error,
}

#[derive(sqlx::Type, Debug, Clone)]
pub struct Step {
    pub id: Uuid,
    pub pipeline_id: Uuid,
    pub name: String,
    pub state: String,
    pub status: StepStatus,
    pub arg: String,
    pub created_at: time::OffsetDateTime,
    pub updated_at: time::OffsetDateTime,
    pub finished_at: Option<time::OffsetDateTime>,
}

impl Step {
    #[instrument(skip(conn))]
    pub(crate) async fn insert<'c>(
        &self,
        conn: impl Executor<'c, Database = Sqlite>,
    ) -> anyhow::Result<()> {
        let step_id = self.id.to_string();
        let pipeline_id = self.pipeline_id.to_string();

        query!(
            r#"
            INSERT INTO steps (
                id, pipeline_id, name, state, status, arg, created_at, updated_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            step_id,
            pipeline_id,
            self.name,
            self.state,
            self.status,
            self.arg,
            self.created_at,
            self.updated_at,
        )
        .execute(conn)
        .await
        .context("could not write step to sqlite db")
        .map(|_| ())
    }

    #[instrument(skip(conn))]
    pub(crate) async fn update<'c>(
        &self,
        conn: impl Executor<'c, Database = Sqlite>,
    ) -> anyhow::Result<()> {
        let step = Self {
            updated_at: OffsetDateTime::now_utc(),
            ..self.clone()
        };

        let step_id = step.id.to_string();
        let pipeline_id = step.pipeline_id.to_string();

        query!(
            r#"
            UPDATE steps
            SET pipeline_id = ?,
                name = ?,
                state = ?,
                status = ?,
                arg = ?,
                created_at = ?,
                updated_at =?,
                finished_at = ?
            WHERE
                id = ?
            "#,
            pipeline_id,
            self.name,
            self.state,
            self.status,
            self.arg,
            self.created_at,
            self.updated_at,
            self.finished_at,
            step_id,
        )
        .execute(conn)
        .await
        .context("could not write step to sqlite db")
        .map(|_| ())
    }
}

#[derive(Debug, Clone)]
pub struct Pipeline {
    pub id: Uuid,
    pub kind: String,
    pub args: String,
    pub status: StepStatus,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
    pub finished_at: Option<OffsetDateTime>,
}

impl Pipeline {
    #[instrument(skip(conn))]
    pub(crate) async fn insert<'c>(
        &self,
        conn: impl Executor<'c, Database = Sqlite>,
    ) -> anyhow::Result<()> {
        let pipeline_id = self.id.to_string();

        query!(
            r#"
            INSERT INTO pipelines (
                id, kind, args, status, created_at, updated_at
            )
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
            pipeline_id,
            self.kind,
            self.args,
            self.status,
            self.created_at,
            self.updated_at,
        )
        .execute(conn)
        .await
        .context("could not write youtube info to sqlite db")
        .map(|_| ())
    }
}
