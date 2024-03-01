use anyhow::Context;
use sqlx::{migrate::MigrateDatabase, sqlite::SqlitePool};

pub(crate) struct Config {
    pub database_url: String,
}

impl Config {
    pub(crate) async fn connect_db(&self) -> anyhow::Result<SqlitePool> {
        SqlitePool::connect(&self.database_url)
            .await
            .context("connect to sqlite from config")
    }
}

pub(crate) async fn init(config: Config) -> anyhow::Result<()> {
    if !sqlx::Sqlite::database_exists(&config.database_url).await? {
        sqlx::Sqlite::create_database(&config.database_url).await?;
    }

    let sqlite_pool = config.connect_db().await?;

    sqlx::migrate!()
        .run(&sqlite_pool)
        .await
        .context("migrating database")
}
