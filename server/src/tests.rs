//! End-to-end tests driving the real router with a temp DB.
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

use crate::crypto::{verify_token, Keys};
use crate::models::AppStateInner;

async fn build_app() -> (axum::Router, String) {
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("kls_test_{n}"));
    std::fs::create_dir_all(&dir).unwrap();
    let pool = crate::db::connect(&dir.join("t.db")).await.unwrap();
    let keys = Keys::load_or_generate(&dir.join("seed")).unwrap();
    let public_key_b64 = keys.public_key_b64();
    let state = Arc::new(AppStateInner {
        pool,
        keys,
        public_key_b64: public_key_b64.clone(),
        admin_token: "test-admin".into(),
    });
    (crate::router(state), public_key_b64)
}

async fn send(app: &axum::Router, method: &str, uri: &str, token: Option<&str>, body: Value) -> (StatusCode, Value) {
    let mut b = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json");
    if let Some(t) = token {
        b = b.header("authorization", format!("Bearer {t}"));
    }
    let req = b.body(Body::from(serde_json::to_vec(&body).unwrap())).unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let val: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, val)
}

#[tokio::test]
async fn full_activation_lifecycle() {
    let (app, pubkey) = build_app().await;

    // admin API requires the token
    let (st, _) = send(&app, "GET", "/api/products", None, json!({})).await;
    assert_eq!(st, StatusCode::UNAUTHORIZED);

    // create a product
    let (st, prod) = send(&app, "POST", "/api/products", Some("test-admin"),
        json!({ "name": "Survarium Server" })).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(prod["slug"], "survarium-server");
    let pid = prod["id"].as_i64().unwrap();

    // generate one 2-seat key
    let (st, gen) = send(&app, "POST", &format!("/api/products/{pid}/keys"),
        Some("test-admin"), json!({ "count": 1, "seat_limit": 2 })).await;
    assert_eq!(st, StatusCode::OK);
    let key = gen["keys"][0].as_str().unwrap().to_string();
    assert!(key.starts_with("KTSN-"));

    // activate device A — token must verify with the published public key
    let (st, a) = send(&app, "POST", "/activate", None,
        json!({ "key": key, "device_fingerprint": "device-A", "os": "windows" })).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(a["seats_used"], 1);
    let token = a["token"].as_str().unwrap();
    let payload = verify_token(&pubkey, token).expect("token verifies");
    let claims: Value = serde_json::from_slice(&payload).unwrap();
    assert_eq!(claims["product"], "survarium-server");
    assert_eq!(claims["device"], "device-A");

    // re-activating the same device is idempotent (still 1 seat)
    let (st, a2) = send(&app, "POST", "/activate", None,
        json!({ "key": key, "device_fingerprint": "device-A" })).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(a2["seats_used"], 1);

    // second device fills the last seat
    let (st, b) = send(&app, "POST", "/activate", None,
        json!({ "key": key, "device_fingerprint": "device-B" })).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(b["seats_used"], 2);

    // third device is over the seat limit
    let (st, _) = send(&app, "POST", "/activate", None,
        json!({ "key": key, "device_fingerprint": "device-C" })).await;
    assert_eq!(st, StatusCode::CONFLICT);

    // an unknown key is rejected
    let (st, _) = send(&app, "POST", "/activate", None,
        json!({ "key": "KTSN-AAAA-BBBB-CCCC-DDDD", "device_fingerprint": "x" })).await;
    assert_eq!(st, StatusCode::NOT_FOUND);

    // validate device A: valid + fresh token
    let (st, v) = send(&app, "POST", "/validate", None,
        json!({ "key": key, "device_fingerprint": "device-A" })).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(v["valid"], true);
    assert!(verify_token(&pubkey, v["token"].as_str().unwrap()).is_some());

    // find key id and revoke the whole key
    let (_, keys) = send(&app, "GET", &format!("/api/products/{pid}/keys"),
        Some("test-admin"), json!({})).await;
    let kid = keys[0]["id"].as_i64().unwrap();
    let (st, _) = send(&app, "POST", &format!("/api/keys/{kid}/revoke"),
        Some("test-admin"), json!({})).await;
    assert_eq!(st, StatusCode::OK);

    // now validation reports the revocation, and re-activation is forbidden
    let (st, v) = send(&app, "POST", "/validate", None,
        json!({ "key": key, "device_fingerprint": "device-A" })).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(v["valid"], false);
    assert_eq!(v["status"], "key_revoked");

    let (st, _) = send(&app, "POST", "/activate", None,
        json!({ "key": key, "device_fingerprint": "device-A" })).await;
    assert_eq!(st, StatusCode::FORBIDDEN);
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

#[tokio::test]
async fn expiry_and_custom_keys() {
    let (app, pubkey) = build_app().await;
    let (_, prod) = send(&app, "POST", "/api/products", Some("test-admin"),
        json!({ "name": "App" })).await;
    let pid = prod["id"].as_i64().unwrap();

    // a custom, already-expired key
    let (st, gen) = send(&app, "POST", &format!("/api/products/{pid}/keys"), Some("test-admin"),
        json!({ "custom_key": "VIP-GOLD-2026", "expires_at": unix_now() - 3600 })).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(gen["keys"][0], "VIP-GOLD-2026");

    // expired key cannot activate
    let (st, _) = send(&app, "POST", "/activate", None,
        json!({ "key": "VIP-GOLD-2026", "device_fingerprint": "d" })).await;
    assert_eq!(st, StatusCode::FORBIDDEN);
    // and validate reports it
    let (st, v) = send(&app, "POST", "/validate", None,
        json!({ "key": "VIP-GOLD-2026", "device_fingerprint": "d" })).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(v["status"], "expired");

    // a custom key with a future expiry activates, and the token carries lic_exp
    let (st, _) = send(&app, "POST", &format!("/api/products/{pid}/keys"), Some("test-admin"),
        json!({ "custom_key": "vip plat 2027", "expires_in_days": 30 })).await;
    assert_eq!(st, StatusCode::OK);
    let (st, a) = send(&app, "POST", "/activate", None,
        json!({ "key": "vip plat 2027", "device_fingerprint": "d" })).await;
    assert_eq!(st, StatusCode::OK);
    let payload = verify_token(&pubkey, a["token"].as_str().unwrap()).unwrap();
    let claims: Value = serde_json::from_slice(&payload).unwrap();
    assert!(claims["lic_exp"].as_i64().unwrap() > unix_now());

    // a duplicate custom key is rejected
    let (st, _) = send(&app, "POST", &format!("/api/products/{pid}/keys"), Some("test-admin"),
        json!({ "custom_key": "VIP-GOLD-2026" })).await;
    assert_eq!(st, StatusCode::CONFLICT);
}

#[tokio::test]
async fn device_revocation_blocks_reactivation() {
    let (app, _pubkey) = build_app().await;
    let (_, prod) = send(&app, "POST", "/api/products", Some("test-admin"),
        json!({ "name": "Tool" })).await;
    let pid = prod["id"].as_i64().unwrap();
    let (_, gen) = send(&app, "POST", &format!("/api/products/{pid}/keys"),
        Some("test-admin"), json!({ "count": 1, "seat_limit": 3 })).await;
    let key = gen["keys"][0].as_str().unwrap().to_string();

    send(&app, "POST", "/activate", None,
        json!({ "key": key, "device_fingerprint": "dev" })).await;
    let (_, keys) = send(&app, "GET", &format!("/api/products/{pid}/keys"),
        Some("test-admin"), json!({})).await;
    let kid = keys[0]["id"].as_i64().unwrap();
    let (_, acts) = send(&app, "GET", &format!("/api/keys/{kid}/activations"),
        Some("test-admin"), json!({})).await;
    let aid = acts[0]["id"].as_i64().unwrap();

    let (st, _) = send(&app, "POST", &format!("/api/activations/{aid}/revoke"),
        Some("test-admin"), json!({})).await;
    assert_eq!(st, StatusCode::OK);

    // revoked device cannot reactivate (key itself still valid)
    let (st, _) = send(&app, "POST", "/activate", None,
        json!({ "key": key, "device_fingerprint": "dev" })).await;
    assert_eq!(st, StatusCode::FORBIDDEN);
}
