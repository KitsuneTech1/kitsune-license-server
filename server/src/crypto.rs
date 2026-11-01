//! License-key generation/hashing and Ed25519 token signing.
//!
//! Activation tokens are `base64url(payload).base64url(signature)`. The client
//! embeds the server's public key and verifies the signature, so a forged
//! "valid" response or a fake server cannot pass. Keys are stored only as
//! SHA-256 hashes, so a DB leak does not expose usable keys.

use std::path::Path;

use anyhow::{anyhow, Context, Result};
use data_encoding::{BASE32_NOPAD, BASE64URL_NOPAD, HEXLOWER};
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use rand::rngs::OsRng;
use rand::RngCore;
use sha2::{Digest, Sha256};

const KEY_PREFIX: &str = "KTSN";
const KEY_BYTES: usize = 10; // 80 bits entropy -> 16 base32 chars -> 4 groups of 4

/// Server signing identity. The 32-byte seed is persisted to disk.
pub struct Keys {
    signing: SigningKey,
}

impl Keys {
    /// Load the Ed25519 seed from `path`, or generate + persist a new one.
    pub fn load_or_generate(path: &Path) -> Result<Self> {
        if path.exists() {
            let bytes = std::fs::read(path).with_context(|| format!("reading {path:?}"))?;
            let seed: [u8; 32] = bytes
                .as_slice()
                .try_into()
                .map_err(|_| anyhow!("key file {path:?} is not 32 bytes"))?;
            restrict_key_permissions(path)?;
            Ok(Self { signing: SigningKey::from_bytes(&seed) })
        } else {
            let signing = SigningKey::generate(&mut OsRng);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).ok();
            }
            std::fs::write(path, signing.to_bytes())
                .with_context(|| format!("writing {path:?}"))?;
            restrict_key_permissions(path)?;
            Ok(Self { signing })
        }
    }

    /// base64url(no-pad) of the 32-byte public key — embedded in the client.
    pub fn public_key_b64(&self) -> String {
        BASE64URL_NOPAD.encode(self.signing.verifying_key().as_bytes())
    }

    /// Sign `payload` and return a `base64url(payload).base64url(sig)` token.
    pub fn sign_token(&self, payload: &[u8]) -> String {
        let sig = self.signing.sign(payload);
        format!(
            "{}.{}",
            BASE64URL_NOPAD.encode(payload),
            BASE64URL_NOPAD.encode(&sig.to_bytes())
        )
    }
}

/// Keep the signing seed readable only by the service account on Unix.
/// Windows uses ACLs rather than POSIX mode bits, so this is intentionally a
/// no-op there and the deployment account remains the authority boundary.
fn restrict_key_permissions(path: &Path) -> anyhow::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(path)?.permissions();
        permissions.set_mode(0o600);
        std::fs::set_permissions(path, permissions)?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

/// Verify a token against a base64url public key; returns the payload bytes.
/// Mirrors the client crate's verifier; used by tests and available for reuse.
#[allow(dead_code)]
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

/// Generate a fresh license key, e.g. `KTSN-AB12-CD34-EF56-GH78`.
pub fn generate_license_key() -> String {
    let mut buf = [0u8; KEY_BYTES];
    OsRng.fill_bytes(&mut buf);
    let enc = BASE32_NOPAD.encode(&buf); // 16 chars, alphabet A-Z2-7
    let groups: Vec<String> = enc
        .as_bytes()
        .chunks(4)
        .map(|c| String::from_utf8_lossy(c).into_owned())
        .collect();
    format!("{KEY_PREFIX}-{}", groups.join("-"))
}

/// Canonical, dash-free, prefix-free uppercase form of a key as typed.
/// Lenient: ignores case, dashes, spaces, and an optional `KTSN` prefix.
pub fn canonicalize_key(input: &str) -> String {
    let cleaned: String = input
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_uppercase())
        .collect();
    cleaned.strip_prefix(KEY_PREFIX).unwrap_or(&cleaned).to_string()
}

/// SHA-256 hash (hex) of the canonical key — what we store and look up by.
pub fn hash_key(input: &str) -> String {
    let mut h = Sha256::new();
    h.update(canonicalize_key(input).as_bytes());
    HEXLOWER.encode(&h.finalize())
}

/// Short, safe-to-display fragment of a key for the admin UI (never the secret).
/// Works for both generated `KTSN-…` keys and admin-supplied custom keys.
pub fn key_display_prefix(key: &str) -> String {
    let shown: String = key.trim().chars().take(9).collect();
    format!("{shown}…")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile_min::TempPath;

    #[test]
    fn key_format_and_canonicalization() {
        let key = generate_license_key();
        assert!(key.starts_with("KTSN-"));
        assert_eq!(key.matches('-').count(), 4); // KTSN + 4 groups
        // typing it back with messy formatting hashes identically
        let messy = format!("  {}  ", key.to_lowercase().replace('-', " "));
        assert_eq!(hash_key(&key), hash_key(&messy));
        // a different key hashes differently
        assert_ne!(hash_key(&key), hash_key(&generate_license_key()));
    }

    #[test]
    fn sign_and_verify_roundtrip() {
        let path = TempPath::new("kls_test_key");
        let keys = Keys::load_or_generate(path.as_ref()).unwrap();
        let pk = keys.public_key_b64();
        let payload = br#"{"key_id":7,"device":"abc"}"#;
        let token = keys.sign_token(payload);

        assert_eq!(verify_token(&pk, &token).as_deref(), Some(&payload[..]));
        // tampered payload fails
        let (_, sig) = token.split_once('.').unwrap();
        let forged = format!("{}.{}", BASE64URL_NOPAD.encode(b"{\"key_id\":8}"), sig);
        assert!(verify_token(&pk, &forged).is_none());
        // wrong public key fails
        let other = Keys::load_or_generate(TempPath::new("kls_test_key2").as_ref()).unwrap();
        assert!(verify_token(&other.public_key_b64(), &token).is_none());
    }

    #[test]
    fn persisted_key_is_stable() {
        let path = TempPath::new("kls_stable_key");
        let a = Keys::load_or_generate(path.as_ref()).unwrap().public_key_b64();
        let b = Keys::load_or_generate(path.as_ref()).unwrap().public_key_b64();
        assert_eq!(a, b);
    }

    #[cfg(unix)]
    #[test]
    fn persisted_key_is_owner_only() {
        let path = TempPath::new("kls_private_key");
        Keys::load_or_generate(path.as_ref()).unwrap();
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(path.as_ref()).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}

/// Tiny self-contained temp-path helper for tests (avoids an extra dependency).
#[cfg(test)]
mod tempfile_min {
    use std::path::{Path, PathBuf};

    pub struct TempPath(PathBuf);

    impl TempPath {
        pub fn new(tag: &str) -> Self {
            let mut p = std::env::temp_dir();
            let n = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            p.push(format!("{tag}_{n}"));
            let _ = std::fs::remove_file(&p);
            TempPath(p)
        }
    }
    impl AsRef<Path> for TempPath {
        fn as_ref(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for TempPath {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
}
