#!/usr/bin/env bash
# Install/upgrade the Kitsune License Server on a Debian/Ubuntu LXC.
# Run as root on the LXC:   bash install.sh /path/to/source-tree
set -euo pipefail

APP=/opt/kitsune-license-server
SRC="${1:-$(cd "$(dirname "$0")/.." && pwd)}"

echo "==> source: $SRC"
id kls &>/dev/null || useradd --system --home "$APP" --shell /usr/sbin/nologin kls
mkdir -p "$APP/data"

# toolchain
if ! command -v cargo >/dev/null; then
  echo "==> installing rust toolchain"
  apt-get update -y && apt-get install -y curl build-essential pkg-config
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
  . "$HOME/.cargo/env"
fi

echo "==> building release binary (this takes a few minutes)"
( cd "$SRC" && cargo build --release -p kitsune-license-server )
install -m755 "$SRC/target/release/kls" "$APP/kls"

# first-run env with a generated admin token
if [ ! -f "$APP/.env" ]; then
  cp "$SRC/deploy/.env.example" "$APP/.env"
  TOKEN="$(openssl rand -hex 24)"
  sed -i "s/CHANGE_ME_LONG_RANDOM/$TOKEN/" "$APP/.env"
  echo "==> generated admin token (save it): $TOKEN"
fi

chown -R kls:kls "$APP"
chmod 600 "$APP/.env"

install -m644 "$SRC/deploy/kls.service" /etc/systemd/system/kls.service
systemctl daemon-reload
systemctl enable --now kls
sleep 1
systemctl --no-pager --full status kls | head -n 12
echo
echo "==> local health:"; curl -fsS http://127.0.0.1:8080/health && echo
echo "==> server public key (embed this in your CLI/guard):"
curl -fsS http://127.0.0.1:8080/pubkey
echo
echo "Done. Admin token is in $APP/.env (KLS_ADMIN_TOKEN)."
