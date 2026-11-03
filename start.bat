@echo off
setlocal
echo ========================================================
echo   License Keeper (kitsune-license-server)
echo   Kitsune Technologies LLC
echo ========================================================
echo.

where cargo >nul 2>nul
if %ERRORLEVEL% neq 0 (
    echo [ERROR] Rust / Cargo was not found on your system.
    echo Please install Rust from https://rustup.rs/
    pause
    exit /b 1
)

echo Starting License Keeper server on http://localhost:8080 ...
start http://localhost:8080
cargo run --bin kls -- server --listen 127.0.0.1:8080
pause
