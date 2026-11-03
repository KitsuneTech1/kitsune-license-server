#!/usr/bin/env bash
echo "========================================================"
echo "  License Keeper (kitsune-license-server)"
echo "  Kitsune Technologies LLC"
echo "========================================================"
echo ""

if ! command -v cargo >/dev/null 2>&1; then
    echo "[ERROR] Rust / Cargo was not found on your system."
    echo "Please install Rust from https://rustup.rs/"
    exit 1
fi

echo "Starting License Keeper server on http://localhost:8080 ..."
if command -v xdg-open >/dev/null 2>&1; then
    xdg-open http://localhost:8080 &
elif command -v open >/dev/null 2>&1; then
    open http://localhost:8080 &
fi

cargo run --bin kls -- server --listen 127.0.0.1:8080
