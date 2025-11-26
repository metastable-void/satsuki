pub mod user_repo;

// src/db/mod.rs (add this)
use sqlx::SqlitePool;

pub type Db = SqlitePool;

pub async fn init_db(path: &std::path::Path) -> anyhow::Result<Db> {
    let url = format!("sqlite://{}", path.display());
    let pool = SqlitePool::connect(&url).await?;
    sqlx::migrate!().run(&pool).await?;
    Ok(pool)
}
