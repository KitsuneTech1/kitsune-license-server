//! SQLite connection + schema bootstrap.
use std::path::Path;
use std::str::FromStr;

use anyhow::{Context, Result};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::SqlitePool;

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS products (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    slug        TEXT    NOT NULL UNIQUE,
    name        TEXT    NOT NULL,
    created_at  INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS license_keys (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    product_id  INTEGER NOT NULL REFERENCES products(id) ON DELETE CASCADE,
    key_hash    TEXT    NOT NULL UNIQUE,
    key_prefix  TEXT    NOT NULL,
    seat_limit  INTEGER NOT NULL DEFAULT 1,
    status      TEXT    NOT NULL DEFAULT 'active',
    note        TEXT,
    expires_at  INTEGER,             -- NULL = never expires
    created_at  INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_keys_product ON license_keys(product_id);

CREATE TABLE IF NOT EXISTS activations (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    key_id              INTEGER NOT NULL REFERENCES license_keys(id) ON DELETE CASCADE,
    device_fingerprint  TEXT    NOT NULL,
    os                  TEXT,
    hostname            TEXT,
    app_version         TEXT,
    activated_at        INTEGER NOT NULL,
    last_seen           INTEGER NOT NULL,
    status              TEXT    NOT NULL DEFAULT 'active',
    UNIQUE(key_id, device_fingerprint)
);
CREATE INDEX IF NOT EXISTS idx_act_key ON activations(key_id);
"#;

pub async fn connect(path: &Path) -> Result<SqlitePool> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let url = format!("sqlite://{}", path.display());
    let opts = SqliteConnectOptions::from_str(&url)
        .with_context(|| format!("bad sqlite url {url}"))?
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal);
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(opts)
        .await
        .context("opening sqlite pool")?;
    sqlx::raw_sql(SCHEMA)
        .execute(&pool)
        .await
        .context("applying schema")?;
    migrate(&pool).await;
    Ok(pool)
}

/// Additive, idempotent migrations for DBs created before a column existed.
/// Each ALTER is ignored if the column is already present.
async fn migrate(pool: &SqlitePool) {
    for stmt in ["ALTER TABLE license_keys ADD COLUMN expires_at INTEGER"] {
        let _ = sqlx::query(stmt).execute(pool).await;
    }
}
