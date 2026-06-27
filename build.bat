@echo off
setlocal enabledelayedexpansion
chcp 65001 >nul 2>&1
title Ravens Nexus — Build

echo.
echo  ██████╗  █████╗ ██╗   ██╗███████╗███╗   ██╗███████╗
echo  ██╔══██╗██╔══██╗██║   ██║██╔════╝████╗  ██║██╔════╝
echo  ██████╔╝███████║██║   ██║█████╗  ██╔██╗ ██║███████╗
echo  ██╔══██╗██╔══██║╚██╗ ██╔╝██╔══╝  ██║╚██╗██║╚════██║
echo  ██║  ██║██║  ██║ ╚████╔╝ ███████╗██║ ╚████║███████║
echo  ╚═╝  ╚═╝╚═╝  ╚═╝  ╚═══╝  ╚══════╝╚═╝  ╚═══╝╚══════╝
echo  NEXUS — Windows Build Script
echo.

:: ── Check prerequisites ──────────────────────────────────────────────────────
echo [1/5] Checking prerequisites...

where cargo >nul 2>&1
if errorlevel 1 (
    echo   [MISSING] cargo — install Rust from https://rustup.rs
    echo   After install, restart this terminal and run build.bat again.
    pause
    exit /b 1
)
echo   [OK] cargo

where node >nul 2>&1
if errorlevel 1 (
    echo   [MISSING] node — install from https://nodejs.org
    pause
    exit /b 1
)
echo   [OK] node

where pnpm >nul 2>&1
if errorlevel 1 (
    echo   [INSTALLING] pnpm...
    npm install -g pnpm
    if errorlevel 1 ( echo pnpm install failed & pause & exit /b 1 )
)
echo   [OK] pnpm

:: Check tauri-cli
cargo tauri --version >nul 2>&1
if errorlevel 1 (
    echo   [INSTALLING] tauri-cli...
    cargo install tauri-cli@^2
    if errorlevel 1 ( echo tauri-cli install failed & pause & exit /b 1 )
)
echo   [OK] tauri-cli

:: ── Copy .env if missing ──────────────────────────────────────────────────────
echo.
echo [2/5] Checking config...
if not exist ravens-server\.env (
    if exist .env.example (
        copy .env.example ravens-server\.env >nul
        echo   Created ravens-server\.env from .env.example
        echo   *** Edit ravens-server\.env and set JWT_SECRET before first run ***
    )
) else (
    echo   [OK] ravens-server\.env exists
)

:: ── Build server ─────────────────────────────────────────────────────────────
echo.
echo [3/5] Building ravens-nexus-server (release)...
cargo build --release -p ravens-server
if errorlevel 1 (
    echo.
    echo   [FAIL] Server build failed. Check errors above.
    pause
    exit /b 1
)
echo   [OK] Server built: target\release\ravens-nexus-server.exe

:: ── Copy server binary into Tauri resources ───────────────────────────────────
echo.
echo [4/5] Copying server binary to Tauri resources...
if not exist ravens-client\src-tauri mkdir ravens-client\src-tauri
copy /y target\release\ravens-nexus-server.exe ravens-client\src-tauri\ravens-nexus-server.exe >nul
echo   [OK] Copied to ravens-client\src-tauri\ravens-nexus-server.exe

:: ── Build Tauri client ────────────────────────────────────────────────────────
echo.
echo [5/5] Building Tauri client (release)...
cd ravens-client
call pnpm install
if errorlevel 1 ( echo pnpm install failed & pause & exit /b 1 )

call cargo tauri build
if errorlevel 1 (
    echo.
    echo   [FAIL] Tauri build failed. Check errors above.
    cd ..
    pause
    exit /b 1
)
cd ..

:: ── Done ─────────────────────────────────────────────────────────────────────
echo.
echo ══════════════════════════════════════════════════════════
echo   BUILD COMPLETE!
echo ══════════════════════════════════════════════════════════
echo.

set BUNDLE=ravens-client\src-tauri\target\release\bundle

:: Find NSIS installer
for /r "%BUNDLE%\nsis" %%f in (*.exe) do (
    echo   Installer:  %%f
)
:: Find MSI
for /r "%BUNDLE%\msi" %%f in (*.msi) do (
    echo   MSI:        %%f
)

echo.
echo   To run the server manually:
echo     ravens-server\.env   ^<-- edit with your settings first
echo     target\release\ravens-nexus-server.exe
echo.
pause
