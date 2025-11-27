//! Database initialization helpers and repositories.

pub mod user_repo;

// src/db/mod.rs (add this)
use sqlx::SqlitePool;
use sqlx::sqlite::SqliteConnectOptions;

pub type Db = SqlitePool;

/// Initialize the SQLite connection pool and run pending migrations.
pub async fn init_db(path: &std::path::Path) -> anyhow::Result<Db> {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true);
    let pool = SqlitePool::connect_with(options).await?;
    sqlx::migrate!().run(&pool).await?;
    Ok(pool)
}
