use std::{path::Path, str::FromStr, time::Duration};

use sqlx::{
    SqlitePool,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
};

pub async fn connect(database_path: &str) -> anyhow::Result<SqlitePool> {
    if database_path != ":memory:"
        && let Some(parent) = Path::new(database_path).parent()
    {
        tokio::fs::create_dir_all(parent).await?;
    }

    let options = if database_path == ":memory:" {
        SqliteConnectOptions::from_str("sqlite::memory:")?.foreign_keys(true)
    } else {
        SqliteConnectOptions::new()
            .filename(database_path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .busy_timeout(Duration::from_secs(10))
            .foreign_keys(true)
    };
    let pool = SqlitePoolOptions::new()
        .max_connections(if database_path == ":memory:" { 1 } else { 8 })
        .connect_with(options)
        .await?;
    sqlx::migrate!().run(&pool).await?;
    Ok(pool)
}
