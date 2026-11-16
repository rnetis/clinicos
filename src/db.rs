use std::str::FromStr;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::{Row, SqlitePool};

pub async fn connect(url: &str) -> Result<SqlitePool, sqlx::Error> {
    let opts = SqliteConnectOptions::from_str(url)?
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .busy_timeout(std::time::Duration::from_secs(10))
        .foreign_keys(true)
        .pragma("temp_store", "memory");

    let pool = SqlitePoolOptions::new()
        .max_connections(16)
        .min_connections(1)
        .acquire_timeout(std::time::Duration::from_secs(10))
        .connect_with(opts)
        .await?;

    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}

pub async fn get_setting(pool: &SqlitePool, key: &str) -> Option<String> {
    sqlx::query("SELECT value FROM settings WHERE key = ?")
        .bind(key)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
        .and_then(|r| r.try_get::<String, _>("value").ok())
}

pub async fn set_setting(pool: &SqlitePool, key: &str, value: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO settings (key, value) VALUES (?, ?)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
    )
    .bind(key)
    .bind(value)
    .execute(pool)
    .await
    .map(|_| ())
}

/// Generate the next human-readable code (P-1001, INV-1001) inside a transaction so
/// concurrent bookings cannot collide on the same sequence number.
pub async fn next_sequence(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    table: &str,
    column: &str,
    prefix: &str,
    start: i64,
) -> Result<String, sqlx::Error> {
    let sql = format!(
        "SELECT COALESCE(MAX(CAST(SUBSTR({column}, {len}) AS INTEGER)), ?) + 1 AS next
         FROM {table}",
        len = prefix.len() + 1
    );
    let row = sqlx::query(&sql).bind(start).fetch_one(&mut **tx).await?;
    let n: i64 = row.try_get("next")?;
    Ok(format!("{prefix}{n}"))
}

pub async fn audit(
    pool: &SqlitePool,
    user_id: Option<&str>,
    user_name: &str,
    role: &str,
    action: &str,
    entity: &str,
    entity_id: &str,
    details: &str,
    outcome: &str,
    ip: &str,
) {
    let id = uuid::Uuid::new_v4().to_string();
    let at = crate::util::now();
    if let Err(err) = sqlx::query(
        "INSERT INTO audit_log (id, at, user_id, user_name, role, action, entity, entity_id, details, outcome, ip)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(at)
    .bind(user_id)
    .bind(user_name)
    .bind(role)
    .bind(action)
    .bind(entity)
    .bind(entity_id)
    .bind(details)
    .bind(outcome)
    .bind(ip)
    .execute(pool)
    .await
    {
        tracing::error!(error = %err, "failed to write audit entry");
    }
}

pub async fn notify(
    pool: &SqlitePool,
    to_role: Option<&str>,
    to_user: Option<&str>,
    kind: &str,
    crit: bool,
    text_fa: &str,
    text_en: &str,
    link: &str,
) {
    let id = uuid::Uuid::new_v4().to_string();
    if let Err(err) = sqlx::query(
        "INSERT INTO notifications (id, at, to_role, to_user, type, crit, read, text_fa, text_en, link)
         VALUES (?, ?, ?, ?, ?, ?, 0, ?, ?, ?)",
    )
    .bind(id)
    .bind(crate::util::now())
    .bind(to_role)
    .bind(to_user)
    .bind(kind)
    .bind(crit as i32)
    .bind(text_fa)
    .bind(text_en)
    .bind(link)
    .execute(pool)
    .await
    {
        tracing::error!(error = %err, "failed to write notification");
    }
}
