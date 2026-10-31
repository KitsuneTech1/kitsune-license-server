//! Public phone-home endpoints: /activate, /validate, /heartbeat.
use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::config::{now, TOKEN_TTL_SECS};
use crate::crypto::hash_key;
use crate::error::AppError;
use crate::models::{Activation, LicenseKey, Shared};

#[derive(Deserialize)]
pub struct ActivateReq {
    pub key: String,
    pub device_fingerprint: String,
    #[serde(default)]
    pub os: Option<String>,
    #[serde(default)]
    pub hostname: Option<String>,
    #[serde(default)]
    pub app_version: Option<String>,
}

#[derive(Serialize)]
pub struct ActivateResp {
    pub token: String,
    pub public_key: String,
    pub product: String,
    pub seat_limit: i64,
    pub seats_used: i64,
    pub expires_at: i64,
}

fn make_token(
    st: &Shared,
    key_id: i64,
    product: &str,
    device: &str,
    iat: i64,
    lic_exp: Option<i64>,
) -> (String, i64) {
    // The token is short-lived (re-check window); the license may itself expire
    // sooner, carried as `lic_exp` so an offline client can self-enforce.
    let exp = match lic_exp {
        Some(le) => (iat + TOKEN_TTL_SECS).min(le),
        None => iat + TOKEN_TTL_SECS,
    };
    let payload = json!({
        "v": 1, "key_id": key_id, "product": product,
        "device": device, "iat": iat, "exp": exp, "lic_exp": lic_exp
    });
    let token = st.keys.sign_token(payload.to_string().as_bytes());
    (token, exp)
}

/// Is this key past its license expiry?
fn is_expired(key: &LicenseKey, now: i64) -> bool {
    matches!(key.expires_at, Some(e) if now >= e)
}

async fn seats_used(st: &Shared, key_id: i64) -> Result<i64, AppError> {
    let (n,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM activations WHERE key_id=? AND status='active'")
            .bind(key_id)
            .fetch_one(&st.pool)
            .await?;
    Ok(n)
}

async fn lookup_key(st: &Shared, key: &str) -> Result<LicenseKey, AppError> {
    let hash = hash_key(key);
    sqlx::query_as::<_, LicenseKey>("SELECT * FROM license_keys WHERE key_hash = ?")
        .bind(hash)
        .fetch_optional(&st.pool)
        .await?
        .ok_or_else(|| AppError::NotFound("invalid license key".into()))
}

pub async fn activate(
    State(st): State<Shared>,
    Json(req): Json<ActivateReq>,
) -> Result<Json<ActivateResp>, AppError> {
    if req.device_fingerprint.trim().is_empty() {
        return Err(AppError::BadRequest("device_fingerprint is required".into()));
    }
    let key = lookup_key(&st, &req.key).await?;
    if key.status != "active" {
        return Err(AppError::Forbidden("license key has been revoked".into()));
    }
    let now = now();
    if is_expired(&key, now) {
        return Err(AppError::Forbidden("license key has expired".into()));
    }
    let (product,): (String,) = sqlx::query_as("SELECT slug FROM products WHERE id = ?")
        .bind(key.product_id)
        .fetch_one(&st.pool)
        .await?;

    let existing = sqlx::query_as::<_, Activation>(
        "SELECT * FROM activations WHERE key_id = ? AND device_fingerprint = ?",
    )
    .bind(key.id)
    .bind(&req.device_fingerprint)
    .fetch_optional(&st.pool)
    .await?;

    match existing {
        Some(a) if a.status == "active" => {
            sqlx::query(
                "UPDATE activations SET last_seen=?, os=?, hostname=?, app_version=? WHERE id=?",
            )
            .bind(now)
            .bind(&req.os)
            .bind(&req.hostname)
            .bind(&req.app_version)
            .bind(a.id)
            .execute(&st.pool)
            .await?;
        }
        Some(_) => {
            return Err(AppError::Forbidden(
                "this device's activation was revoked".into(),
            ));
        }
        None => {
            if seats_used(&st, key.id).await? >= key.seat_limit {
                return Err(AppError::Conflict(format!(
                    "seat limit reached ({} of {})",
                    key.seat_limit, key.seat_limit
                )));
            }
            sqlx::query(
                "INSERT INTO activations
                 (key_id, device_fingerprint, os, hostname, app_version, activated_at, last_seen, status)
                 VALUES (?,?,?,?,?,?,?, 'active')",
            )
            .bind(key.id)
            .bind(&req.device_fingerprint)
            .bind(&req.os)
            .bind(&req.hostname)
            .bind(&req.app_version)
            .bind(now)
            .bind(now)
            .execute(&st.pool)
            .await?;
        }
    }

    let (token, exp) = make_token(&st, key.id, &product, &req.device_fingerprint, now, key.expires_at);
    Ok(Json(ActivateResp {
        token,
        public_key: st.public_key_b64.clone(),
        product,
        seat_limit: key.seat_limit,
        seats_used: seats_used(&st, key.id).await?,
        expires_at: exp,
    }))
}

#[derive(Deserialize)]
pub struct CheckReq {
    pub key: String,
    pub device_fingerprint: String,
}

#[derive(Serialize)]
pub struct ValidateResp {
    pub valid: bool,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<i64>,
}

/// Periodic re-check. Returns a refreshed token while still valid; otherwise a
/// reason so the client can react (revoked key/device, never activated).
pub async fn validate(
    State(st): State<Shared>,
    Json(req): Json<CheckReq>,
) -> Result<Json<ValidateResp>, AppError> {
    let key = match lookup_key(&st, &req.key).await {
        Ok(k) => k,
        Err(_) => {
            return Ok(Json(ValidateResp {
                valid: false,
                status: "invalid_key".into(),
                token: None,
                product: None,
                expires_at: None,
            }))
        }
    };
    if key.status != "active" {
        return Ok(Json(ValidateResp {
            valid: false,
            status: "key_revoked".into(),
            token: None,
            product: None,
            expires_at: None,
        }));
    }
    if is_expired(&key, now()) {
        return Ok(Json(ValidateResp {
            valid: false,
            status: "expired".into(),
            token: None,
            product: None,
            expires_at: key.expires_at,
        }));
    }
    let act = sqlx::query_as::<_, Activation>(
        "SELECT * FROM activations WHERE key_id=? AND device_fingerprint=?",
    )
    .bind(key.id)
    .bind(&req.device_fingerprint)
    .fetch_optional(&st.pool)
    .await?;
    match act {
        Some(a) if a.status == "active" => {
            let (product,): (String,) = sqlx::query_as("SELECT slug FROM products WHERE id=?")
                .bind(key.product_id)
                .fetch_one(&st.pool)
                .await?;
            let now = now();
            sqlx::query("UPDATE activations SET last_seen=? WHERE id=?")
                .bind(now)
                .bind(a.id)
                .execute(&st.pool)
                .await?;
            let (token, exp) =
                make_token(&st, key.id, &product, &req.device_fingerprint, now, key.expires_at);
            Ok(Json(ValidateResp {
                valid: true,
                status: "active".into(),
                token: Some(token),
                product: Some(product),
                expires_at: Some(exp),
            }))
        }
        Some(_) => Ok(Json(ValidateResp {
            valid: false,
            status: "device_revoked".into(),
            token: None,
            product: None,
            expires_at: None,
        })),
        None => Ok(Json(ValidateResp {
            valid: false,
            status: "not_activated".into(),
            token: None,
            product: None,
            expires_at: None,
        })),
    }
}

/// View-only device list for a key (for the embeddable end-user page). Returns
/// no secrets — no key hash, no device fingerprints — just what a customer would
/// see about their own activations. Anyone holding the key may view it.
pub async fn devices(
    State(st): State<Shared>,
    Json(req): Json<DevicesReq>,
) -> Result<Json<serde_json::Value>, AppError> {
    let key = lookup_key(&st, &req.key).await?;
    let (product,): (String,) = sqlx::query_as("SELECT slug FROM products WHERE id=?")
        .bind(key.product_id)
        .fetch_one(&st.pool)
        .await?;
    let acts = sqlx::query_as::<_, Activation>(
        "SELECT * FROM activations WHERE key_id=? ORDER BY activated_at DESC",
    )
    .bind(key.id)
    .fetch_all(&st.pool)
    .await?;
    let devices: Vec<_> = acts
        .iter()
        .map(|a| {
            json!({
                "os": a.os, "hostname": a.hostname, "app_version": a.app_version,
                "activated_at": a.activated_at, "last_seen": a.last_seen, "status": a.status
            })
        })
        .collect();
    let used = acts.iter().filter(|a| a.status == "active").count();
    Ok(Json(json!({
        "product": product,
        "key_status": key.status,
        "seat_limit": key.seat_limit,
        "seats_used": used,
        "expires_at": key.expires_at,
        "devices": devices
    })))
}

#[derive(Deserialize)]
pub struct DevicesReq {
    pub key: String,
}

/// Lightweight last-seen ping for an active device.
pub async fn heartbeat(
    State(st): State<Shared>,
    Json(req): Json<CheckReq>,
) -> Result<Json<serde_json::Value>, AppError> {
    let key = lookup_key(&st, &req.key).await?;
    let updated = sqlx::query(
        "UPDATE activations SET last_seen=?
         WHERE key_id=? AND device_fingerprint=? AND status='active'",
    )
    .bind(now())
    .bind(key.id)
    .bind(&req.device_fingerprint)
    .execute(&st.pool)
    .await?;
    Ok(Json(json!({ "ok": updated.rows_affected() == 1 })))
}
