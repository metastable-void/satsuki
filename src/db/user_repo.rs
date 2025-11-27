//! Repository functions for manipulating rows in the `users` table.
use chrono::{DateTime, Utc};
use sqlx::{Row, SqlitePool};

/// Application-level representation of a stored user.
#[derive(Debug, Clone)]
pub struct User {
    pub id: i64,
    pub subdomain: String,
    pub password_hash: String,
    pub external_ns: bool,
    pub external_ns1: Option<String>,
    pub external_ns2: Option<String>,
    pub external_ns3: Option<String>,
    pub external_ns4: Option<String>,
    pub external_ns5: Option<String>,
    pub external_ns6: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_login_at: Option<DateTime<Utc>>,
}

/// Determine whether a subdomain already has a user row.
pub async fn exists(db: &SqlitePool, subdomain: &str) -> sqlx::Result<bool> {
    let cnt: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users WHERE subdomain = ?")
        .bind(subdomain)
        .fetch_one(db)
        .await?;
    Ok(cnt.0 > 0)
}

/// Fetch a user and all NS metadata for the given subdomain.
pub async fn find_by_subdomain(db: &SqlitePool, subdomain: &str) -> sqlx::Result<Option<User>> {
    let row = sqlx::query(
        r#"
        SELECT
            id,
            subdomain,
            password_hash,
            external_ns,
            external_ns1,
            external_ns2,
            external_ns3,
            external_ns4,
            external_ns5,
            external_ns6,
            created_at,
            updated_at,
            last_login_at
        FROM users
        WHERE subdomain = ?
        "#,
    )
    .bind(subdomain)
    .fetch_optional(db)
    .await?;

    let Some(row) = row else {
        return Ok(None);
    };

    Ok(Some(User {
        id: row.get("id"),
        subdomain: row.get("subdomain"),
        password_hash: row.get("password_hash"),
        external_ns: row.get::<i64, _>("external_ns") != 0,
        external_ns1: row.get("external_ns1"),
        external_ns2: row.get("external_ns2"),
        external_ns3: row.get("external_ns3"),
        external_ns4: row.get("external_ns4"),
        external_ns5: row.get("external_ns5"),
        external_ns6: row.get("external_ns6"),
        created_at: row.get::<DateTime<Utc>, _>("created_at"),
        updated_at: row.get::<DateTime<Utc>, _>("updated_at"),
        last_login_at: row.get("last_login_at"),
    }))
}

/// Create a new user row when signup completes successfully.
pub async fn insert(db: &SqlitePool, subdomain: &str, password_hash: &str) -> sqlx::Result<i64> {
    let now = Utc::now();

    let res = sqlx::query(
        r#"
        INSERT INTO users (
            subdomain,
            password_hash,
            external_ns,
            external_ns1,
            external_ns2,
            external_ns3,
            external_ns4,
            external_ns5,
            external_ns6,
            created_at,
            updated_at,
            last_login_at
        ) VALUES (?, ?, 0, NULL, NULL, NULL, NULL, NULL, NULL, ?, ?, NULL)
        "#,
    )
    .bind(subdomain)
    .bind(password_hash)
    .bind(now)
    .bind(now)
    .execute(db)
    .await?;

    Ok(res.last_insert_rowid())
}

/// Persist the user's NS mode and up to six external nameservers.
pub async fn set_external_ns(
    db: &SqlitePool,
    user_id: i64,
    external_ns: bool,
    ns1: Option<String>,
    ns2: Option<String>,
    ns3: Option<String>,
    ns4: Option<String>,
    ns5: Option<String>,
    ns6: Option<String>,
) -> sqlx::Result<()> {
    let now = Utc::now();
    sqlx::query(
        r#"
        UPDATE users
        SET
            external_ns = ?,
            external_ns1 = ?,
            external_ns2 = ?,
            external_ns3 = ?,
            external_ns4 = ?,
            external_ns5 = ?,
            external_ns6 = ?,
            updated_at = ?
        WHERE id = ?
        "#,
    )
    .bind(if external_ns { 1 } else { 0 })
    .bind(ns1)
    .bind(ns2)
    .bind(ns3)
    .bind(ns4)
    .bind(ns5)
    .bind(ns6)
    .bind(now)
    .bind(user_id)
    .execute(db)
    .await?;

    Ok(())
}

/// Update the user's last successful login timestamp.
pub async fn update_last_login(db: &SqlitePool, user_id: i64) -> sqlx::Result<()> {
    let now = Utc::now();
    sqlx::query(
        r#"
        UPDATE users
        SET last_login_at = ?, updated_at = ?
        WHERE id = ?
        "#,
    )
    .bind(now)
    .bind(now)
    .bind(user_id)
    .execute(db)
    .await?;

    Ok(())
}
