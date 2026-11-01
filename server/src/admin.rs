//! Admin API (bearer-gated). In production the admin UI also sits behind
//! Authentik forward_auth; this token guards the API directly.
use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::config::now;
use crate::crypto::{canonicalize_key, generate_license_key, hash_key, key_display_prefix};
use crate::error::AppError;
use crate::models::{Activation, LicenseKey, Product, Shared};

/// Deny-by-default bearer check. An empty configured token rejects everything.
fn require_admin(st: &Shared, headers: &HeaderMap) -> Result<(), AppError> {
    if st.admin_token.is_empty() {
        return Err(AppError::Unauthorized);
    }
    let presented = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("");
    if constant_time_eq(presented.as_bytes(), st.admin_token.as_bytes()) {
        Ok(())
    } else {
        Err(AppError::Unauthorized)
    }
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

fn slugify(s: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash && !out.is_empty() {
            out.push('-');
            prev_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}

#[derive(Deserialize)]
pub struct NewProduct {
    pub name: String,
    #[serde(default)]
    pub slug: Option<String>,
}

pub async fn create_product(
    State(st): State<Shared>,
    headers: HeaderMap,
    Json(req): Json<NewProduct>,
) -> Result<Json<Product>, AppError> {
    require_admin(&st, &headers)?;
    if req.name.trim().is_empty() {
        return Err(AppError::BadRequest("name is required".into()));
    }
    let slug = req
        .slug
        .map(|s| slugify(&s))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| slugify(&req.name));
    if slug.is_empty() {
        return Err(AppError::BadRequest(
            "name or slug must contain a letter or digit".into(),
        ));
    }
    let res = sqlx::query("INSERT INTO products (slug, name, created_at) VALUES (?,?,?)")
        .bind(&slug)
        .bind(req.name.trim())
        .bind(now())
        .execute(&st.pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(d) if d.is_unique_violation() => {
                AppError::Conflict(format!("product slug '{slug}' already exists"))
            }
            other => other.into(),
        })?;
    let product = sqlx::query_as::<_, Product>("SELECT * FROM products WHERE id=?")
        .bind(res.last_insert_rowid())
        .fetch_one(&st.pool)
        .await?;
    Ok(Json(product))
}

pub async fn list_products(
    State(st): State<Shared>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    require_admin(&st, &headers)?;
    let products = sqlx::query_as::<_, Product>("SELECT * FROM products ORDER BY created_at DESC")
        .fetch_all(&st.pool)
        .await?;
    let mut out = Vec::new();
    for p in products {
        let (keys,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM license_keys WHERE product_id=?")
                .bind(p.id)
                .fetch_one(&st.pool)
                .await?;
        out.push(json!({ "id": p.id, "slug": p.slug, "name": p.name,
            "created_at": p.created_at, "key_count": keys }));
    }
    Ok(Json(json!(out)))
}

#[derive(Deserialize)]
pub struct GenKeys {
    #[serde(default = "one")]
    pub count: i64,
    #[serde(default = "one")]
    pub seat_limit: i64,
    #[serde(default)]
    pub note: Option<String>,
    /// Supply a specific key string instead of generating one (forces count=1).
    #[serde(default)]
    pub custom_key: Option<String>,
    /// Absolute expiry (unix seconds). Takes precedence over expires_in_days.
    #[serde(default)]
    pub expires_at: Option<i64>,
    /// Relative expiry from now, in days. `None`/0 with no expires_at = never.
    #[serde(default)]
    pub expires_in_days: Option<i64>,
}
fn one() -> i64 {
    1
}

async fn insert_key(
    st: &Shared,
    product_id: i64,
    key: &str,
    seat_limit: i64,
    note: &Option<String>,
    expires_at: Option<i64>,
) -> Result<(), AppError> {
    let hash = hash_key(key);
    sqlx::query(
        "INSERT INTO license_keys
         (product_id, key_hash, key_prefix, seat_limit, status, note, expires_at, created_at)
         VALUES (?,?,?,?, 'active', ?, ?, ?)",
    )
    .bind(product_id)
    .bind(&hash)
    .bind(key_display_prefix(key))
    .bind(seat_limit)
    .bind(note)
    .bind(expires_at)
    .bind(now())
    .execute(&st.pool)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(d) if d.is_unique_violation() => {
            AppError::Conflict("that key already exists".into())
        }
        other => other.into(),
    })?;
    Ok(())
}

/// Generate (or accept custom) keys for a product. Plaintext keys are returned
/// **once** here and never stored — only their hashes are persisted.
pub async fn generate_keys(
    State(st): State<Shared>,
    headers: HeaderMap,
    Path(product_id): Path<i64>,
    Json(req): Json<GenKeys>,
) -> Result<Json<Value>, AppError> {
    require_admin(&st, &headers)?;
    if req.seat_limit < 1 {
        return Err(AppError::BadRequest("seat_limit must be >= 1".into()));
    }
    let exists: Option<(i64,)> = sqlx::query_as("SELECT id FROM products WHERE id=?")
        .bind(product_id)
        .fetch_optional(&st.pool)
        .await?;
    if exists.is_none() {
        return Err(AppError::NotFound("product not found".into()));
    }

    // resolve expiry: absolute wins, else now + days, else never
    let expires_at =
        match (req.expires_at, req.expires_in_days) {
            (Some(ts), _) => Some(ts),
            (None, Some(d)) if d > 0 => {
                let delta = d
                    .checked_mul(86_400)
                    .ok_or_else(|| AppError::BadRequest("expires_in_days is too large".into()))?;
                Some(now().checked_add(delta).ok_or_else(|| {
                    AppError::BadRequest("expiry is too far in the future".into())
                })?)
            }
            (None, Some(d)) if d < 0 => {
                return Err(AppError::BadRequest(
                    "expires_in_days must be non-negative".into(),
                ));
            }
            _ => None,
        };

    // custom single key
    if let Some(custom) = req
        .custom_key
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        if canonicalize_key(custom).len() < 6 {
            return Err(AppError::BadRequest("custom_key too short".into()));
        }
        insert_key(
            &st,
            product_id,
            custom,
            req.seat_limit,
            &req.note,
            expires_at,
        )
        .await?;
        return Ok(Json(
            json!({ "count": 1, "keys": [custom], "expires_at": expires_at }),
        ));
    }

    if !(1..=1000).contains(&req.count) {
        return Err(AppError::BadRequest("count must be 1..=1000".into()));
    }
    let mut issued = Vec::new();
    for _ in 0..req.count {
        // retry on the astronomically-unlikely hash collision
        let key = loop {
            let key = generate_license_key();
            let dup: Option<(i64,)> =
                sqlx::query_as("SELECT id FROM license_keys WHERE key_hash=?")
                    .bind(hash_key(&key))
                    .fetch_optional(&st.pool)
                    .await?;
            if dup.is_none() {
                break key;
            }
        };
        insert_key(&st, product_id, &key, req.seat_limit, &req.note, expires_at).await?;
        issued.push(key);
    }
    Ok(Json(
        json!({ "count": issued.len(), "keys": issued, "expires_at": expires_at }),
    ))
}

pub async fn list_keys(
    State(st): State<Shared>,
    headers: HeaderMap,
    Path(product_id): Path<i64>,
) -> Result<Json<Value>, AppError> {
    require_admin(&st, &headers)?;
    let keys = sqlx::query_as::<_, LicenseKey>(
        "SELECT * FROM license_keys WHERE product_id=? ORDER BY created_at DESC",
    )
    .bind(product_id)
    .fetch_all(&st.pool)
    .await?;
    let mut out = Vec::new();
    for k in keys {
        let (used,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM activations WHERE key_id=? AND status='active'")
                .bind(k.id)
                .fetch_one(&st.pool)
                .await?;
        out.push(json!({
            "id": k.id, "prefix": k.key_prefix, "seat_limit": k.seat_limit,
            "seats_used": used, "status": k.status, "note": k.note,
            "expires_at": k.expires_at, "created_at": k.created_at
        }));
    }
    Ok(Json(json!(out)))
}

pub async fn revoke_key(
    State(st): State<Shared>,
    headers: HeaderMap,
    Path(key_id): Path<i64>,
) -> Result<Json<Value>, AppError> {
    require_admin(&st, &headers)?;
    let res = sqlx::query("UPDATE license_keys SET status='revoked' WHERE id=?")
        .bind(key_id)
        .execute(&st.pool)
        .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound("key not found".into()));
    }
    Ok(Json(json!({ "ok": true, "status": "revoked" })))
}

pub async fn list_key_activations(
    State(st): State<Shared>,
    headers: HeaderMap,
    Path(key_id): Path<i64>,
) -> Result<Json<Vec<Activation>>, AppError> {
    require_admin(&st, &headers)?;
    let acts = sqlx::query_as::<_, Activation>(
        "SELECT * FROM activations WHERE key_id=? ORDER BY activated_at DESC",
    )
    .bind(key_id)
    .fetch_all(&st.pool)
    .await?;
    Ok(Json(acts))
}

pub async fn revoke_activation(
    State(st): State<Shared>,
    headers: HeaderMap,
    Path(act_id): Path<i64>,
) -> Result<Json<Value>, AppError> {
    require_admin(&st, &headers)?;
    let res = sqlx::query("UPDATE activations SET status='revoked' WHERE id=?")
        .bind(act_id)
        .execute(&st.pool)
        .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound("activation not found".into()));
    }
    Ok(Json(json!({ "ok": true, "status": "revoked" })))
}
