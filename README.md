<p align="center"><a href="https://kitsunetechnologies.org/work"><img src="https://raw.githubusercontent.com/KitsuneTech1/.github/main/assets/kitsune-banner.svg" alt="Built by Kitsune Technologies" width="760"></a></p>

# License Keeper (kitsune-license-server)

> Production-grade, self-hosted software licensing server and client SDK written in Rust. Features Ed25519 cryptographic signatures, SHA-256 hashed keys at rest, embedded SQLite, web admin desk, and a terminal activator with animated ASCII art.

<p align="center">
  <a href="https://github.com/KitsuneTech1/kitsune-license-server/releases"><img src="https://img.shields.io/github/v/release/KitsuneTech1/kitsune-license-server?style=flat-square&color=blue" alt="Release"></a>
  <a href="https://github.com/KitsuneTech1/kitsune-license-server/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/KitsuneTech1/kitsune-license-server/ci.yml?branch=master&label=CI&style=flat-square" alt="CI Status"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-MIT-green.svg?style=flat-square" alt="License"></a>
  <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/Rust-2021%20Edition-DEA584?style=flat-square&logo=rust&logoColor=white" alt="Rust Version"></a>
  <a href="Cargo.toml"><img src="https://img.shields.io/badge/Crypto-Ed25519%20Signatures-blueviolet?style=flat-square" alt="Ed25519"></a>
  <a href="Cargo.toml"><img src="https://img.shields.io/badge/Database-Embedded%20SQLite-003B57?style=flat-square&logo=sqlite&logoColor=white" alt="SQLite Database"></a>
</p>

<p align="center">
  <a href="https://github.com/KitsuneTech1/kitsune-license-server/releases/latest"><strong>📦 Download Latest Release ZIP</strong></a> •
  <a href="#-quick-start-noob-friendly"><strong>⚡ Quick Start Guide</strong></a> •
  <a href="#architecture--cryptographic-verification"><strong>🔒 Architecture & Crypto</strong></a>
</p>

---

## Architecture & Cryptographic Verification

```mermaid
flowchart TD
    ClientApp([Protected Client App]) -->|Embeds| Guard[kitsune-license-client SDK]
    Guard -->|POST /activate {key, fingerprint}| Server[Axum License Server: kls]
    
    subgraph Server Security Boundary [Rust Axum Engine + Embedded SQLite]
        Server -->|SHA-256 Hash Lookup| DB[(SQLite Database)]
        Server -->|Sign Entitlement Token| Crypto[Ed25519 Private Key]
        Crypto --> Token[Signed Ed25519 License Token]
    end
    
    Token -->|Verified locally by public key| Guard
    Guard -->|Persist to disk with signature| Cache[(Local License Cache)]
    
    Admin([System Administrator]) -->|Bearer Auth / Web UI| AdminAPI[Admin Control Desk]
    AdminAPI -->|Manage Seats & Products| DB
```

---

## Features

- **Ed25519 Cryptographic Verification**: Every activation returns a cryptographically signed token. Clients verify the signature locally with the server's public key; man-in-the-middle or spoofed servers cannot forge valid activations.
- **Hashed Keys at Rest**: All keys are stored as SHA-256 digests. Database dumps reveal zero plaintext license keys.
- **Three-Tier Workspace**:
  - **`server/` (`kls`)**: High-performance Axum REST server with embedded SQLite and zero external database dependencies.
  - **`client/` (`kitsune-license-client`)**: Lightweight client crate that handles device fingerprinting, signature verification, background lease guarding, and offline grace windows.
  - **`cli/` (`ktsn-activate`)**: Interactive terminal activation wizard with shimmering ASCII art and real-time validation.
- **Admin Control Desk**: Clean, embedded web interface for generating license keys, setting seat limits, setting expiration dates, and revoking specific devices.
- **Device Management**: Embeddable self-service portal where customers view their registered hardware without administrative credentials.

---

## 🚀 Quick Start (Noob-Friendly)

### 1. Download & Launch
1. Download [**License Keeper ZIP**](https://github.com/KitsuneTech1/kitsune-license-server/releases/latest) and extract it.
2. Ensure you have [Rust installed](https://rustup.rs/).
3. Double-click **`start.bat`** (Windows) or execute `./start.sh` (macOS/Linux).
4. The server compiles and starts at `http://localhost:8080`.

---

## 💻 Developer & SDK Guide

### Build All Binaries
```bash
cargo build --release
```

Artifacts generated:
- `target/release/kls`: Server daemon and admin utility
- `target/release/ktsn-activate`: Terminal activation CLI

### Running the Server
```bash
# Generate a new server Ed25519 keypair and database
cargo run --bin kls -- server --listen 127.0.0.1:8080 --admin-token "your-admin-token"
```

Open `http://127.0.0.1:8080/admin` and log in with your admin token.

### Protecting an Application (Rust SDK)

Add the client to your `Cargo.toml`:
```toml
[dependencies]
kitsune-license-client = { path = "path/to/client" }
```

In your application:
```rust
use kitsune_license_client::{Guard, GuardConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let guard = Guard::init(GuardConfig {
        server_url: "https://licenses.example.com".to_string(),
        server_public_key: "ed25519-public-key-hex".to_string(),
        product_slug: "my-pro-tool".to_string(),
        ..Default::default()
    }).await?;

    if guard.is_entitled() {
        println!("License verified! Launching application...");
    } else {
        eprintln!("License invalid or expired.");
    }

    Ok(())
}
```

---

## 🧪 Testing & Verification

Run the full Rust test suite across all workspace crates:

```bash
cargo test
```

Tests cover:
- Device revocation and seat limit race-condition prevention.
- Signature forgery detection and expired grace periods.
- XSS prevention on browser management portals.

---

## License

MIT. Copyright (c) 2026 Kitsune Technologies LLC. Full text in [LICENSE](LICENSE).

<p align="center"><sub>Part of <b>Buildtober 2026</b>: 31 open source projects in 31 days. Built by <a href="https://kitsunetechnologies.org/work">Kitsune Technologies</a>. More at <a href="https://github.com/KitsuneTech1">github.com/KitsuneTech1</a>.</sub></p>
