//! Embeddable license client for Kitsune-protected programs.
//!
//! Two layers:
//!  * one-shot calls — [`activate`], [`validate`] — that talk to the server and
//!    verify the Ed25519-signed token against your embedded public key, and
//!  * a [`Guard`] that runs in the background and *continuously* re-checks that
//!    the key is still active, flipping [`Guard::is_active`] and firing a
//!    callback the moment a key is revoked, expired, or its device kicked.
//!
//! The Guard remembers the last good state (persisted), so a protected app keeps
//! running across brief network outages — but stops trusting a stale state once
//! the offline `grace` window is exceeded, and self-enforces the license expiry
//! carried in the signed token even with no network at all.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use data_encoding::{BASE64URL_NOPAD, HEXLOWER};
use ed25519_dalek::{Signature, VerifyingKey};
use serde_json::Value;
use sha2::{Digest, Sha256};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("network error: {0}")]
    Network(String),
    #[error("server rejected activation: {0}")]
    Server(String),
    #[error("server response failed signature verification")]
    Verify,
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("bad response: {0}")]
    Bad(String),
}

/// Where the protected program points, and the trust anchor it carries.
#[derive(Clone)]
pub struct Config {
    pub server: String,
    /// base64url Ed25519 public key of the license server (embedded in your app).
    pub public_key: String,
    /// Optional: require the token's product slug to match.
    pub product: Option<String>,
    /// Directory for the device id + cached license (created on demand).
    pub config_dir: PathBuf,
}

impl Config {
    pub fn new(server: impl Into<String>, public_key: impl Into<String>) -> Self {
        let config_dir = std::env::var("KTSN_CONFIG_DIR")
            .ok()
            .map(PathBuf::from)
            .or_else(|| std::env::var("APPDATA").ok().map(PathBuf::from))
            .or_else(|| std::env::var("HOME").ok().map(|h| PathBuf::from(h).join(".config")))
            .unwrap_or_else(|| PathBuf::from("."))
            .join("kitsune");
        Self { server: server.into(), public_key: public_key.into(), product: None, config_dir }
    }
}

/// A verified, locally-cached license.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct License {
    pub token: String,
    pub product: String,
    pub device: String,
    pub token_exp: i64,
    pub lic_exp: Option<i64>,
}

/// Current verdict about a license.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Active,
    KeyRevoked,
    DeviceRevoked,
    Expired,
    NotActivated,
    InvalidKey,
    /// Couldn't reach/verify the server; running on cached state.
    Offline,
    /// No check has completed yet.
    Unknown,
}

impl Status {
    fn from_server(s: &str) -> Status {
        match s {
            "active" => Status::Active,
            "key_revoked" => Status::KeyRevoked,
            "device_revoked" => Status::DeviceRevoked,
            "expired" => Status::Expired,
            "not_activated" => Status::NotActivated,
            "invalid_key" => Status::InvalidKey,
            _ => Status::Unknown,
        }
    }
    /// Does this status mean the program may run?
    pub fn is_entitled(self) -> bool {
        matches!(self, Status::Active | Status::Offline)
    }
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Verify `base64url(payload).base64url(sig)` against a base64url public key.
pub fn verify_token(public_key_b64: &str, token: &str) -> Option<Vec<u8>> {
    let (p, s) = token.split_once('.')?;
    let payload = BASE64URL_NOPAD.decode(p.as_bytes()).ok()?;
    let sig_bytes = BASE64URL_NOPAD.decode(s.as_bytes()).ok()?;
    let pk = BASE64URL_NOPAD.decode(public_key_b64.as_bytes()).ok()?;
    let pk: [u8; 32] = pk.try_into().ok()?;
    let vk = VerifyingKey::from_bytes(&pk).ok()?;
    let sig = Signature::from_slice(&sig_bytes).ok()?;
    vk.verify_strict(&payload, &sig).ok()?;
    Some(payload)
}

fn license_from_token(cfg: &Config, token: &str, device: &str) -> Result<License, Error> {
    let payload = verify_token(&cfg.public_key, token).ok_or(Error::Verify)?;
    let c: Value = serde_json::from_slice(&payload).map_err(|e| Error::Bad(e.to_string()))?;
    if c["device"].as_str() != Some(device) {
        return Err(Error::Bad("token device mismatch".into()));
    }
    if let Some(want) = cfg.product.as_deref() {
        if c["product"].as_str() != Some(want) {
            return Err(Error::Bad("wrong product".into()));
        }
    }
    Ok(License {
        token: token.to_string(),
        product: c["product"].as_str().unwrap_or_default().to_string(),
        device: device.to_string(),
        token_exp: c["exp"].as_i64().unwrap_or(0),
        lic_exp: c["lic_exp"].as_i64(),
    })
}

/// Stable per-install device id (random, persisted under `config_dir`).
pub fn device_fingerprint(cfg: &Config) -> std::io::Result<String> {
    let path = cfg.config_dir.join("device");
    if let Ok(existing) = std::fs::read_to_string(&path) {
        let t = existing.trim().to_string();
        if !t.is_empty() {
            return Ok(t);
        }
    }
    let mut seed = [0u8; 16];
    fill_random(&mut seed);
    let mut h = Sha256::new();
    h.update(seed);
    let id = HEXLOWER.encode(&h.finalize()[..16]);
    std::fs::create_dir_all(&cfg.config_dir)?;
    std::fs::write(&path, &id)?;
    Ok(id)
}

/// Activate a key online and return a verified [`License`] (also cached).
pub fn activate(cfg: &Config, key: &str) -> Result<License, Error> {
    let device = device_fingerprint(cfg)?;
    let body = serde_json::json!({
        "key": key.trim(), "device_fingerprint": device,
        "os": std::env::consts::OS, "app_version": env!("CARGO_PKG_VERSION"),
    });
    let resp = match ureq::post(&format!("{}/activate", cfg.server)).send_json(body) {
        Ok(r) => r,
        Err(ureq::Error::Status(_, r)) => {
            let msg = r
                .into_json::<Value>()
                .ok()
                .and_then(|v| v["message"].as_str().map(String::from))
                .unwrap_or_else(|| "request rejected".into());
            return Err(Error::Server(msg));
        }
        Err(e) => return Err(Error::Network(e.to_string())),
    };
    let v: Value = resp.into_json().map_err(|e| Error::Bad(e.to_string()))?;
    let token = v["token"].as_str().ok_or_else(|| Error::Bad("no token".into()))?;
    let lic = license_from_token(cfg, token, &device)?;
    save_license(cfg, &lic).ok();
    Ok(lic)
}

/// One-shot re-check. Returns the verdict and, when active, a refreshed license.
pub fn validate(cfg: &Config, key: &str, device: &str) -> Result<(Status, Option<License>), Error> {
    let body = serde_json::json!({ "key": key.trim(), "device_fingerprint": device });
    let v: Value = match ureq::post(&format!("{}/validate", cfg.server)).send_json(body) {
        Ok(r) => r.into_json().map_err(|e| Error::Bad(e.to_string()))?,
        Err(ureq::Error::Status(_, _)) => return Ok((Status::Offline, None)),
        Err(e) => return Err(Error::Network(e.to_string())),
    };
    if v["valid"].as_bool() == Some(true) {
        if let Some(tok) = v["token"].as_str() {
            // only trust "active" if the signature actually verifies
            if let Ok(lic) = license_from_token(cfg, tok, device) {
                save_license(cfg, &lic).ok();
                return Ok((Status::Active, Some(lic)));
            }
        }
        return Ok((Status::Offline, None));
    }
    let status = Status::from_server(v["status"].as_str().unwrap_or("unknown"));
    Ok((status, None))
}

pub fn save_license(cfg: &Config, lic: &License) -> std::io::Result<()> {
    std::fs::create_dir_all(&cfg.config_dir)?;
    std::fs::write(cfg.config_dir.join("license.json"), serde_json::to_vec_pretty(lic)?)
}

pub fn load_license(cfg: &Config) -> Option<License> {
    let data = std::fs::read(cfg.config_dir.join("license.json")).ok()?;
    serde_json::from_slice(&data).ok()
}

// ---------------------------------------------------------------------------
// Guard — the continuous checker ("plugin").
// ---------------------------------------------------------------------------

struct GuardState {
    active: AtomicBool,
    status: Mutex<Status>,
    stop: AtomicBool,
}

/// A background thread that keeps a license honest. Construct with
/// [`Guard::start`]; query [`Guard::is_active`] / [`Guard::status`] from your
/// app's hot paths; call [`Guard::stop`] on shutdown.
pub struct Guard {
    state: Arc<GuardState>,
    handle: Option<JoinHandle<()>>,
}

impl Guard {
    /// Start watching `key`. `interval` is how often to phone home; `grace` is
    /// how long to keep trusting cached state while the server is unreachable.
    /// `on_change` fires whenever the status transitions.
    pub fn start<F>(
        cfg: Config,
        key: String,
        interval: Duration,
        grace: Duration,
        on_change: F,
    ) -> Self
    where
        F: Fn(Status) + Send + 'static,
    {
        // Seed from any cached license so we're entitled offline immediately,
        // unless the cached license has already expired.
        let cached = load_license(&cfg);
        let mut cached_lic_exp = cached.as_ref().and_then(|l| l.lic_exp);
        let seed_active = match &cached {
            Some(l) => l.lic_exp.map(|e| now() < e).unwrap_or(true),
            None => false,
        };
        let state = Arc::new(GuardState {
            active: AtomicBool::new(seed_active),
            status: Mutex::new(if seed_active { Status::Offline } else { Status::Unknown }),
            stop: AtomicBool::new(false),
        });

        let st = state.clone();
        let grace_secs = grace.as_secs() as i64;
        let handle = std::thread::spawn(move || {
            let device = device_fingerprint(&cfg).unwrap_or_default();
            let mut last_ok = if seed_active { now() } else { 0 };
            let mut prev = *st.status.lock().unwrap();

            let apply = |st: &GuardState, active: bool, status: Status, prev: &mut Status| {
                st.active.store(active, Ordering::SeqCst);
                *st.status.lock().unwrap() = status;
                if status != *prev {
                    *prev = status;
                    on_change(status);
                }
            };

            loop {
                if st.stop.load(Ordering::SeqCst) {
                    break;
                }
                // local, network-free expiry enforcement on the cached token
                if let Some(le) = cached_lic_exp {
                    if now() >= le {
                        apply(&st, false, Status::Expired, &mut prev);
                        sleep_interruptible(interval, &st.stop);
                        continue;
                    }
                }
                match validate(&cfg, &key, &device) {
                    Ok((Status::Active, Some(lic))) => {
                        cached_lic_exp = lic.lic_exp;
                        last_ok = now();
                        apply(&st, true, Status::Active, &mut prev);
                    }
                    Ok((Status::Offline, _)) | Ok((Status::Unknown, _)) | Err(_) => {
                        if now() - last_ok > grace_secs {
                            apply(&st, false, Status::Offline, &mut prev);
                        } // else keep the previous entitlement
                    }
                    Ok((bad, _)) => apply(&st, false, bad, &mut prev),
                }
                sleep_interruptible(interval, &st.stop);
            }
        });

        Guard { state, handle: Some(handle) }
    }

    /// May the program run right now?
    pub fn is_active(&self) -> bool {
        self.state.active.load(Ordering::SeqCst)
    }

    /// The most recent verdict.
    pub fn status(&self) -> Status {
        *self.state.status.lock().unwrap()
    }

    /// Stop the background checker and join its thread.
    pub fn stop(mut self) {
        self.state.stop.store(true, Ordering::SeqCst);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        self.state.stop.store(true, Ordering::SeqCst);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

fn sleep_interruptible(total: Duration, stop: &AtomicBool) {
    let step = Duration::from_millis(200);
    let mut slept = Duration::ZERO;
    while slept < total {
        if stop.load(Ordering::SeqCst) {
            return;
        }
        std::thread::sleep(step.min(total - slept));
        slept += step;
    }
}

/// Non-cryptographic per-install nonce source (the value is hashed afterwards).
fn fill_random(buf: &mut [u8]) {
    let mut x = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x9e3779b97f4a7c15)
        ^ (buf.as_ptr() as u64);
    for b in buf.iter_mut() {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        *b = (x & 0xff) as u8;
    }
}
