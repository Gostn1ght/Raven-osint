# Ravens Nexus

A licensed OSINT (open-source intelligence) platform consisting of:

- **`ravens-server`** — Rust/Axum backend. Handles license tokens, HWID binding, sessions, bans, tiers, quotas, and runs all OSINT modules server-side. Includes a built-in admin panel.
- **`ravens-client`** — Rust/Tauri + Svelte desktop app (not runnable on Replit — requires a local Tauri build).

## Running on Replit

Only the server runs here. The workflow `Start application` runs:
```
cargo run --release -p ravens-server
```
The server listens on port **3000**.

### Key URLs (once running)
- Admin panel: `/admin-ui/admin.html`
- Health check: `/health`
- API base: `/api`

## Required Environment Secrets

Set these as Replit Secrets (never commit real values):

| Secret | Purpose |
|--------|---------|
| `RAVENS_ADMIN_SECRET` | Admin panel login password |
| `RAVENS_JWT_SECRET` | Signs session tokens (32+ hex chars) |
| `RAVENS_HMAC_SECRET` | Signs server responses (32+ hex chars) |

If any secret is unset, the server **auto-generates a random value at startup and prints it to the log** — fine for local dev, but the value changes on every restart.

### Optional secrets (enable premium OSINT modules)
| Secret | Module |
|--------|--------|
| `NVIDIA_NIM_API_KEY` | AI module |
| `INTELX_API_KEY` | IntelX module |
| `HIBP_API_KEY` | HaveIBeenPwned (raises rate limits) |
| `VERIPHONE_API_KEY` | Phone OSINT |
| `DISCORD_BOT_TOKEN` | Discord gateway integration |

## Build fix applied

`tokio-tungstenite` was switched from `native-tls` to `rustls-tls-webpki-roots` so it builds on Replit without a system OpenSSL installation.

## Tiers

| Tier | Requests/day | Modules |
|------|-------------|---------|
| FREE | 2 | social, ip_geo, whois |
| PRO | 1000 | + hibp, dorks, paste, darkweb, phone |
| ELITE | ∞ | + intelx, ai |
| ADMIN | ∞ | all |

## User preferences
