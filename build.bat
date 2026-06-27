@echo off
setlocal enabledelayedexpansion
title Ravens Nexus Build

echo.
echo  RAVENS NEXUS - Windows Build Script
echo  =====================================
echo.

:: ── Check: cargo (Rust) ──────────────────────────────────────────────────────
echo [1/5] Checking prerequisites...

where cargo >nul 2>nul
if %errorlevel% neq 0 (
    echo   MISSING: cargo
    echo   Install Rust from: https://rustup.rs
    echo   Then restart this terminal and run build.bat again.
    goto :fail
)
echo   OK: cargo

where node >nul 2>nul
if %errorlevel% neq 0 (
    echo   MISSING: node
    echo   Install Node.js from: https://nodejs.org
    goto :fail
)
echo   OK: node

where pnpm >nul 2>nul
if %errorlevel% neq 0 (
    echo   pnpm not found, installing...
    npm install -g pnpm
    if %errorlevel% neq 0 goto :fail
)
echo   OK: pnpm

cargo tauri --version >nul 2>nul
if %errorlevel% neq 0 (
    echo   tauri-cli not found, installing (may take a few minutes)...
    cargo install tauri-cli --version ^>=2.0.0,^<3.0.0
    if %errorlevel% neq 0 goto :fail
)
echo   OK: tauri-cli

:: ── Config ───────────────────────────────────────────────────────────────────
echo.
echo [2/5] Checking config...

if not exist ravens-server\.env (
    if exist .env.example (
        copy .env.example ravens-server\.env >nul
        echo   Created ravens-server\.env from .env.example
        echo   IMPORTANT: Edit ravens-server\.env and set JWT_SECRET before running!
    ) else (
        echo   WARNING: No .env found. Server may not start without config.
    )
) else (
    echo   OK: ravens-server\.env
)

:: ── Build server ─────────────────────────────────────────────────────────────
echo.
echo [3/5] Building server binary...

cargo build --release -p ravens-server
if %errorlevel% neq 0 (
    echo   FAILED: server build
    goto :fail
)
echo   OK: target\release\ravens-nexus-server.exe

:: ── Copy server into Tauri resources ─────────────────────────────────────────
echo.
echo [4/5] Copying server binary to Tauri resources...

if not exist ravens-client\src-tauri mkdir ravens-client\src-tauri
copy /y target\release\ravens-nexus-server.exe ravens-client\src-tauri\ravens-nexus-server.exe >nul
echo   OK: copied to ravens-client\src-tauri\

:: ── Build Tauri client ────────────────────────────────────────────────────────
echo.
echo [5/5] Building Tauri desktop client...

cd ravens-client
call pnpm install
if %errorlevel% neq 0 ( cd .. & goto :fail )

call cargo tauri build
if %errorlevel% neq 0 ( cd .. & goto :fail )
cd ..

:: ── Done ─────────────────────────────────────────────────────────────────────
echo.
echo =============================================
echo   BUILD COMPLETE!
echo =============================================
echo.

for /r "ravens-client\src-tauri\target\release\bundle\nsis" %%f in (*.exe) do (
    echo   Installer: %%f
)
for /r "ravens-client\src-tauri\target\release\bundle\msi" %%f in (*.msi) do (
    echo   MSI:       %%f
)

echo.
echo   Next steps:
echo   1. Edit ravens-server\.env  (set JWT_SECRET, ADMIN_TOKEN)
echo   2. Double-click the installer above to install Ravens Nexus
echo.
pause
exit /b 0

:fail
echo.
echo =============================================
echo   BUILD FAILED - see errors above
echo =============================================
echo.
pause
exit /b 1
