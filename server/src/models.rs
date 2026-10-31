//! Database row types and the shared application state.
use std::sync::Arc;

use serde::Serialize;
use sqlx::FromRow;
use sqlx::SqlitePool;

use crate::crypto::Keys;

#[derive(Debug, FromRow, Serialize)]
pub struct Product {
    pub id: i64,
    pub slug: String,
    pub name: String,
    pub created_at: i64,
}

#[derive(Debug, FromRow, Serialize)]
pub struct LicenseKey {
    pub id: i64,
    pub product_id: i64,
    #[serde(skip_serializing)]
    #[allow(dead_code)] // populated by FromRow; looked up via SQL, not read directly
    pub key_hash: String,
    pub key_prefix: String,
    pub seat_limit: i64,
    pub status: String,
    pub note: Option<String>,
    /// Unix seconds when the *license* expires; `None` = never.
    pub expires_at: Option<i64>,
    pub created_at: i64,
}

#[derive(Debug, FromRow, Serialize)]
pub struct Activation {
    pub id: i64,
    pub key_id: i64,
    pub device_fingerprint: String,
    pub os: Option<String>,
    pub hostname: Option<String>,
    pub app_version: Option<String>,
    pub activated_at: i64,
    pub last_seen: i64,
    pub status: String,
}

/// Shared, cheaply-cloneable application state.
pub struct AppStateInner {
    pub pool: SqlitePool,
    pub keys: Keys,
    pub public_key_b64: String,
    pub admin_token: String,
}

pub type Shared = Arc<AppStateInner>;
