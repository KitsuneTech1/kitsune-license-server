//! Kitsune License Server — online activation with Ed25519-signed tokens.
mod admin;
mod config;
mod crypto;
mod db;
mod error;
mod models;
mod public;

use std::sync::Arc;

use axum::response::Html;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::json;

const ADMIN_HTML: &str = include_str!("../web/admin.html");
const DEVICES_HTML: &str = include_str!("../web/devices.html");
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

use crate::config::Config;
use crate::crypto::Keys;
use crate::models::{AppStateInner, Shared};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,kitsune_license_server=debug")),
        )
        .init();

    let cfg = Config::from_env();
    if cfg.admin_token.is_empty() {
        tracing::warn!("KLS_ADMIN_TOKEN is empty — the admin API is disabled (deny-by-default)");
    }

    let pool = db::connect(&cfg.db_path).await?;
    let keys = Keys::load_or_generate(&cfg.key_path)?;
    let public_key_b64 = keys.public_key_b64();
    tracing::info!("server public key: {public_key_b64}");

    let state: Shared = Arc::new(AppStateInner {
        pool,
        keys,
        public_key_b64,
        admin_token: cfg.admin_token,
    });

    let app = router(state.clone()).layer(TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind(&cfg.bind).await?;
    tracing::info!("listening on {}", cfg.bind);
    axum::serve(listener, app).await?;
    Ok(())
}

fn router(state: Shared) -> Router {
    let public_key = state.public_key_b64.clone();
    Router::new()
        .route("/health", get(|| async { Json(json!({ "ok": true })) }))
        .route(
            "/pubkey",
            get(move || {
                let pk = public_key.clone();
                async move { Json(json!({ "public_key": pk })) }
            }),
        )
        .route("/", get(|| async { Html(ADMIN_HTML) }))
        .route("/admin", get(|| async { Html(ADMIN_HTML) }))
        .route("/embed", get(|| async { Html(DEVICES_HTML) }))
        .route("/activate", post(public::activate))
        .route("/validate", post(public::validate))
        .route("/heartbeat", post(public::heartbeat))
        .route("/devices", post(public::devices))
        .route(
            "/api/products",
            get(admin::list_products).post(admin::create_product),
        )
        .route(
            "/api/products/{id}/keys",
            get(admin::list_keys).post(admin::generate_keys),
        )
        .route("/api/keys/{id}/revoke", post(admin::revoke_key))
        .route("/api/keys/{id}/activations", get(admin::list_key_activations))
        .route("/api/activations/{id}/revoke", post(admin::revoke_activation))
        .with_state(state)
}

#[cfg(test)]
mod tests;
