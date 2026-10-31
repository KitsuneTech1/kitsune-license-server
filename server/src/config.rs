//! Environment configuration. All `KLS_*` with sensible dev defaults.
use std::path::PathBuf;

pub struct Config {
    pub bind: String,
    pub db_path: PathBuf,
    pub key_path: PathBuf,
    pub admin_token: String,
}

impl Config {
    pub fn from_env() -> Self {
        let bind = std::env::var("KLS_BIND").unwrap_or_else(|_| "0.0.0.0:8080".into());
        let data_dir =
            PathBuf::from(std::env::var("KLS_DATA_DIR").unwrap_or_else(|_| "data".into()));
        let db_path = std::env::var("KLS_DB")
            .map(PathBuf::from)
            .unwrap_or_else(|_| data_dir.join("kls.db"));
        let key_path = std::env::var("KLS_KEY")
            .map(PathBuf::from)
            .unwrap_or_else(|_| data_dir.join("ed25519.seed"));
        let admin_token = std::env::var("KLS_ADMIN_TOKEN").unwrap_or_default();
        Config {
            bind,
            db_path,
            key_path,
            admin_token,
        }
    }
}

/// Current unix time in seconds.
pub fn now() -> i64 {
    time::OffsetDateTime::now_utc().unix_timestamp()
}

/// Token lifetime: clients should re-validate within this window (30 days).
pub const TOKEN_TTL_SECS: i64 = 30 * 24 * 60 * 60;
