<p align="center"><a href="https://kitsunetechnologies.org/work"><img src="https://raw.githubusercontent.com/KitsuneTech1/.github/main/assets/kitsune-banner.svg" alt="Built by Kitsune Technologies" width="760"></a></p>

# Kitsune License Server

Self-hosted software licensing: Ed25519-signed activation tokens, SQLite, an admin web UI, and a terminal activator with an animated ASCII mascot.

[![License: MIT](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-2021-orange.svg)](https://www.rust-lang.org/)

## Features

- **Online activation.** A client calls `/activate` with a license key and a device fingerprint. The server checks the key's status, expiry, and seat limit, records the activation, and returns a signed token.
- **Ed25519-signed tokens.** Successful activation and validation responses include a token signed with the server's Ed25519 key. The client verifies the signature against an embedded public key, so a forged "valid" response or a fake server cannot pass.
- **Hashed keys at rest.** License keys are stored as SHA-256 hashes, never in plaintext. A database leak does not expose usable keys.
- **Seat limits, revocation, expiry.** Each key has a seat limit, an optional expiry, and can be revoked. Individual device activations can be revoked without touching the key itself.
- **Admin web UI.** Create products, generate or import license keys, set seat limits and expiry, and view activations per key. Gated by a bearer token (deny-by-default if unset).
- **Embeddable devices page.** A view-only page an end user can open with their own key to see their activated devices (OS, hostname, last seen, status). No secrets, no controls.
- **Terminal activator.** A CLI (`ktsn-activate`) that prompts for a key, activates it, verifies the signature, and gates an install. Draws an animated ASCII fox while it works: typewriter draw-on, then an ember-to-gold shimmer sweeping down the art while the request is in flight.
- **Runtime guard.** The client crate also exposes a `Guard` that runs in the background inside a protected program, periodically re-validates the license, and flips to "not entitled" on the next check after a key is revoked, a device is kicked, or the license expires. It keeps trusting a cached license through brief network outages, up to a configurable grace window, and self-enforces license expiry even fully offline.

## Architecture

Three crates in one workspace.

**`server/`** is the license server itself: Axum + SQLite (via `sqlx`), Ed25519 signing (via `ed25519-dalek`), the admin API, and the two static web pages (admin UI, embeddable devices page) served straight out of the binary via `include_str!`.

**`client/`** is `kitsune-license-client`, the embeddable SDK a protected program links against. It handles activation, one-shot validation, on-disk license caching, and the background `Guard`. It depends on nothing from `server/`, only on the wire format both sides agree on.

**`cli/`** is `ktsn-activate`, a terminal activator built on top of `client/`. It owns the interactive prompt, the ASCII fox animation, and the `--status` / `--watch` modes for checking or continuously monitoring a saved license.

## Run it yourself

This walks through building the project from nothing. It assumes you have never used Rust before.

### What you need

- **The Rust toolchain**, installed via rustup. Go to [https://rustup.rs](https://rustup.rs) and follow the instructions for your OS (it works the same way on Linux, macOS, and Windows). Once it's installed, open a new terminal and check it worked:
  ```bash
  cargo --version
  ```
  If that prints a version number, you're set.
- **A C compiler.** The server bundles SQLite and compiles it from source as part of the build, which needs a real C toolchain, not just Rust:
  - Linux: `sudo apt install build-essential` (Debian/Ubuntu) or your distro's equivalent (gcc/clang + make).
  - macOS: `xcode-select --install` (Xcode Command Line Tools).
  - Windows: install the "Desktop development with C++" workload from the Visual Studio Build Tools. The rustup installer checks for this and will tell you if it's missing.
- **Git**, to clone the repo. Not required if you use the "Download ZIP" option below instead.
- **OS support:** everything in this workspace (Axum, Tokio, sqlx with bundled SQLite, crossterm for the terminal UI) is cross-platform. It builds and runs on Linux, macOS, and Windows.

### Step by step

1. **Get the code.**
   ```bash
   git clone https://github.com/KitsuneTech1/kitsune-license-server.git
   cd kitsune-license-server
   ```
   No Git, or don't want to use the command line? On the GitHub repo page click **Code -> Download ZIP**, unzip it anywhere, then open a terminal in that folder.

2. **Build it.**
   ```bash
   cargo build --release
   ```
   The first build takes several minutes. It's compiling the entire dependency tree from source, including SQLite itself. That is normal, just let it run. Every build after this one only recompiles what changed, so it's much faster.

   When it finishes, the binaries are at:
   - `target/release/kls` (`target\release\kls.exe` on Windows) - the license server
   - `target/release/ktsn-activate` (`target\release\ktsn-activate.exe` on Windows) - the terminal activator

3. **Run the server.**

   Linux/macOS:
   ```bash
   KLS_ADMIN_TOKEN=some-long-random-token ./target/release/kls
   ```
   Windows (PowerShell):
   ```powershell
   $env:KLS_ADMIN_TOKEN="some-long-random-token"
   .\target\release\kls.exe
   ```
   On first run it creates a `data/` folder next to wherever you launch it from, holding the SQLite database (`data/kls.db`) and the server's Ed25519 signing key (`data/ed25519.seed`). Everything is configured through environment variables (see `deploy/.env.example`):

   | Variable | Default | Purpose |
   |---|---|---|
   | `KLS_BIND` | `0.0.0.0:8080` | listen address |
   | `KLS_DATA_DIR` | `data` | directory for the SQLite DB and key seed |
   | `KLS_DB` | `<data_dir>/kls.db` | SQLite path override |
   | `KLS_KEY` | `<data_dir>/ed25519.seed` | signing key seed path |
   | `KLS_ADMIN_TOKEN` | (empty) | bearer token for the admin API; empty disables it |

   You should see log lines like this:
   ```
   INFO kls: server public key: Fo20vUMA4HPCdpxPEsccfPBSMExyXVoJhtouJ6bTRnw
   INFO kls: listening on 0.0.0.0:8080
   ```
   Copy the public key from that line, you'll need it in step 6. Leave this terminal running.

4. **Open the admin UI.** Go to `http://localhost:8080/` in a browser. Paste the admin token you set in step 3 (`some-long-random-token`) into the "admin token" field and click **Connect**.

5. **Create a product and a license key.** Under Products, type a name (e.g. "My App") in "New product name" and click **Create**. Select the product, then under Keys leave "Custom key" blank and click **Generate**. Copy the key it shows you.

6. **Activate with the CLI.** Open a second terminal in the repo root and run:

   Linux/macOS:
   ```bash
   KTSN_SERVER=http://localhost:8080 KTSN_PUBLIC_KEY=<pubkey-from-step-3> ./target/release/ktsn-activate
   ```
   Windows (PowerShell):
   ```powershell
   $env:KTSN_SERVER="http://localhost:8080"
   $env:KTSN_PUBLIC_KEY="<pubkey-from-step-3>"
   .\target\release\ktsn-activate.exe
   ```
   It draws an ASCII fox, then prompts:
   ```
   Enter your license key:
   ```
   Paste the key from step 5 and press enter. On success it prints `Activated for '<product>'.` and caches the key locally, so `ktsn-activate --status` (one check) or `ktsn-activate --watch` (keeps checking) can re-verify it later without asking for the key again.

### Deploy for real

`deploy/` has a working example setup: an unprivileged LXC, a systemd unit, and Caddy in front with Authentik SSO on the admin side and public carve-outs for the client-facing endpoints. The docs already use placeholders like `example.com` and `192.0.2.71` so you can swap in your own domain and IP.

- `deploy/DEPLOY.md` - the full walkthrough, start to finish.
- `deploy/install.sh` - installs Rust if it's missing, builds the release binary, creates a system user, writes `.env` with a generated admin token, installs and starts the systemd service.
- `deploy/kls.service` - the systemd unit `install.sh` installs (runs as an unprivileged `kls` user, restarts on failure).
- `deploy/caddy-license.md` - the Caddy route: public paths (`/activate`, `/validate`, `/heartbeat`, `/devices`, `/pubkey`, `/health`, `/embed`) proxy straight through, everything else (admin UI and `/api/*`) goes behind Authentik forward_auth.
- `deploy/.env.example` - the environment file template `install.sh` copies into place.

### It worked if...

- `curl http://localhost:8080/health` returns `{"ok":true}`.
- The admin UI loads at `http://localhost:8080/` and "Connect" accepts your token without an error.
- `ktsn-activate` prints `Activated for '<product>'.` and exits without an error message.

### Troubleshooting

- **"linker `cc` not found" or a `libsqlite3-sys` build failure.** You're missing a C compiler. Install `build-essential` (Linux), the Xcode Command Line Tools (macOS), or the Visual Studio C++ build tools (Windows), see "What you need" above, then run `cargo build --release` again.
- **The first `cargo build --release` seems stuck.** It isn't, it's compiling SQLite and the rest of the dependency tree from scratch, which takes several minutes on most machines. Later builds are fast.
- **Admin UI won't connect, or the server logs a warning about an empty admin token.** Check that the token you pasted in the UI matches `KLS_ADMIN_TOKEN` exactly. If you never set `KLS_ADMIN_TOKEN`, the server disables the admin API on purpose (deny-by-default) and logs a warning about it on startup.
- **`ktsn-activate` exits with "no server public key embedded (set KTSN_PUBLIC_KEY)".** Set `KTSN_PUBLIC_KEY` to the value printed in the server's startup log (or fetch it any time from `GET /pubkey`).

## API sketch

Public, called by protected programs:

| Endpoint | Purpose |
|---|---|
| `POST /activate` | Activate a key on a device. Returns a signed token, seat usage, and expiry. |
| `POST /validate` | Periodic re-check. Returns a refreshed token while still valid, or a reason (`key_revoked`, `device_revoked`, `expired`, `not_activated`, `invalid_key`). |
| `POST /heartbeat` | Lightweight last-seen ping for an already-active device. |
| `POST /devices` | View-only device list for a key, for the embeddable page. No key hashes or fingerprints returned. |
| `GET /pubkey` | The server's Ed25519 public key. |
| `GET /health` | Liveness check. |
| `GET /embed` | The embeddable devices page (calls `/devices` client-side). |

Admin, bearer-token gated:

| Endpoint | Purpose |
|---|---|
| `GET/POST /api/products` | List products, or create one. |
| `GET/POST /api/products/{id}/keys` | List a product's keys, or generate/import keys (batch, seat limit, expiry, or a specific custom key). |
| `POST /api/keys/{id}/revoke` | Revoke a key. |
| `GET /api/keys/{id}/activations` | List a key's activations. |
| `POST /api/activations/{id}/revoke` | Revoke a single device activation without touching the key. |

## License

MIT. Copyright (c) 2026 Kitsune Technologies LLC. Do what you like with it, commercially or otherwise, just keep the copyright notice. Full text in [LICENSE](LICENSE).
