# Ravens Nexus — Overhaul Report

**Date:** 2026-07-23
**Scope:** study the repo · fix the visuals · fix the functionality · move all functionality to the server (token + account‑key model) · fix the server · add security · test everything.

---

## 1. What the project is

Ravens Nexus is a licensed **OSINT (open‑source intelligence) platform** with two parts:

| Component | Stack | Role |
|-----------|-------|------|
| `ravens-server` | Rust · Axum | License authority **and** OSINT engine + admin panel (`ui/admin.html`) |
| `ravens-client` | Rust · Tauri · Svelte 4 · Vite | Desktop app the operator runs |

Accounts are **license tokens** (`RVN-…`, the "account keys"). Tiers **FREE / PRO / ELITE / ADMIN** gate which OSINT modules and how many daily requests a token gets. Tokens bind to a device (**HWID**); sessions and hardware can be banned from the admin panel.

---

## 2. State of the repo when I started

The branch did **not build or work**. Concrete findings:

### Repo corruption (last commit)
- `Cargo.toml` (workspace root) and `FIX_BUILD.txt` had been overwritten with **PNG image data**.
- `.env.example` contained a **Cargo.lock** dump; the real `.replit` had been replaced by the workspace manifest; `Cargo.lock` was empty.
- → Recovered every file from the last good commit (`43d8d36`).

### Server would not compile
- `main.rs` — `admin_create_news` built a `NewsItem` **missing the `attachments` field**.
- `payment.rs` — four calls to `format(...)` instead of `format!(...)`; a `HashMap::get(&token)` type mismatch.
- `discord.rs` — used `.split()/.send()/.next()` **without importing** `futures::{SinkExt, StreamExt}`.
- `db.rs` — used the **compile‑time‑checked `sqlx::query_as!` macros**, which need a live database at build time; it was also entirely **dead code** (never called).

### Functionality was fake / broken
- The desktop scan was a **simulation**: `startScan()` called `setTimeout()` and printed "данные получены" — no real recon. The real OSINT code lived in the **client** (`osint.rs`) but the UI never called it.
- The Svelte UI invoked `executeAction` while the Rust command was named `execute_action` → **command not found** at runtime.
- Window‑control buttons called `minimize_window` / `maximize_window` / `close_window` — **commands that didn't exist**.
- When the user changed the server URL in Settings, the Rust HTTP client kept using the **old startup URL**.

### Security theatre / hard‑coded secrets
- A **long production admin secret was hard‑coded** in `main.rs`; JWT/HMAC secrets hard‑coded in `security.rs`.
- IntelX and other **API keys were hard‑coded and shipped inside the client**.
- The anti‑replay / signature middleware existed but was **never wired to any route**.
- A hard‑coded personal Replit dev URL was baked into the client.

---

## 3. What I changed

### 3.1 Architecture — OSINT moved to the server (the headline change)
The client is now a **thin client**. Reconnaissance runs on the backend:

```
Client                              Server
──────                              ──────
activate(token, hwid)  ───────────▶ bind HWID, create session,
                       ◀─────────── issue session token + salt + allowed modules + quota

sign request (salt) ─ POST /api/osint/run ─▶  verify token+HWID+signature+nonce,
  {module, target}                            rate-limit, enforce tier + quota,
                       ◀──────────────────── RUN THE MODULE, return events (signed)
```

- New endpoint **`POST /api/osint/run`** (`main.rs`) → `osint::run_module()`.
- Ported all 10 modules to the server (`ravens-server/src/osint.rs`): `social, ip_geo, whois, hibp, dorks, paste, darkweb, phone, intelx, ai`.
- **All third‑party API keys now live in server env** (`INTELX_API_KEY`, `NVIDIA_NIM_API_KEY`, `HIBP_API_KEY`, `VERIPHONE_API_KEY`) and never reach the client. Modules degrade gracefully when a key is absent.
- Deleted the client‑side `osint.rs` and its local scan commands.

### 3.2 Server fixes & hardening
- Fixed every compile error above; server now builds with **0 warnings**.
- Replaced the broken `sqlx`/`db.rs` layer with a **dependency‑light JSON snapshot store** (`store.rs`) — atomic writes, loaded on boot, saved on every mutation. Licenses, bans and news now **persist across restarts** (verified).
- Secrets are **env‑driven**; if unset, a **random per‑process secret is generated and logged** — there is no hard‑coded production key anymore.
- Daily quota reset, HWID binding, session/HWID bans, tiers, news, uploads, Discord state, admin CRUD — all retained and wired to persistence.

### 3.3 Security measures (now actually enforced on `/api/osint/run`)
1. **Session tokens** — HMAC‑signed, HWID‑bound, 60‑min expiry, env‑keyed.
2. **Per‑request signatures** — client signs `hwid:nonce:timestamp` with a per‑session salt; server verifies.
3. **Anti‑replay** — timestamp freshness window **+ one‑time nonce cache** (a captured request cannot be replayed).
4. **Rate limiting** — 30 req/min per HWID, sliding window.
5. **Tier + quota enforcement** server‑side (client can't grant itself modules or requests).
6. **HWID‑ban / session‑ban** checks on every call; constant‑time secret comparisons.
7. **Signed responses** (anti‑tamper) — verification is best‑effort on the client since TLS already guarantees integrity.
8. Input validation (empty/oversized targets), log hygiene (HWID truncated in logs).

### 3.4 Client rewiring
- Real scans: `startScan()` now calls the Tauri `osint_run` command per module and renders server results; `GEO_PIN` events drop pins on the map; the AI module receives collected findings.
- Added the missing **window‑control commands** and a **`set_server_url`** command so the UI's server URL is authoritative.
- Removed the hard‑coded Replit URL (defaults to `http://localhost:3000`, overridable in Settings / `RAVENS_SERVER_URL`).

### 3.5 Visual redesign (2026 refresh)
- **Client** (`App.svelte`): rebuilt UI — Inter for chrome + JetBrains Mono for data, animated aurora background, glassmorphism panels, gradient accent nav/buttons, refined console, news grid, settings, map, about; five live accent themes; compact mode. Frontend builds clean.
- **Admin panel** (`ui/admin.html`): modernized tokens — gradient brand/stat text, glass sidebar & cards, gradient primary buttons, refined login. All existing admin functionality untouched.

---

## 4. Test results

### Server unit tests — `cargo test -p ravens-server` → **4/4 pass**
token round‑trip & tamper rejection · request‑signature scheme · nonce‑replay rejection · rate‑limit enforcement.

### End‑to‑end harness (Node, replicates the client's signing) → **27/27 pass**
health · admin token creation · activation **+ verified signed response** · **authenticated server‑side OSINT run** · bad‑signature rejection (401) · stale‑timestamp rejection (401) · nonce‑replay rejection (401) · tier enforcement (FREE blocked from `ai`, 403) · quota exhaustion (FREE 2/day → 402) · HWID binding (2nd device blocked) · invalid‑token rejection (401).

### Live recon proof (server‑side, real external call)
`ip_geo` on `1.1.1.1` returned real data: Cloudflare / Australia / South Brisbane, ASN 13335, `GEO_PIN:-27.4766,153.0166`, rDNS `one.one.one.one`.

### Persistence
Quota counts and licenses survived a full server restart (6 licenses reloaded from `data/ravens_state.json`).

### Client
Frontend production build succeeds (`vite build`). The Tauri desktop app was launched via `tauri dev`.

---

## 5. How to run

### Server
```bash
cd ravens-server
# set RAVENS_ADMIN_SECRET / RAVENS_JWT_SECRET / RAVENS_HMAC_SECRET (see .env.example)
cargo run -p ravens-server --release
```
- Admin panel: `http://<host>:3000/admin-ui/admin.html` (log in with `RAVENS_ADMIN_SECRET`).
- Health: `http://<host>:3000/health`.

### Client
```bash
cd ravens-client
npm install
npm run tauri dev      # or: npm run tauri build
```
In the app: **Settings → Server**, set the server URL, paste a token, **Activate**, then use the **OSINT** tab.

Demo tokens seeded on first run: `RVN-ADMIN-DEMO-0000-0000` (ADMIN) + one FREE + one PRO (see server log / admin panel).

---

## 6. Notes & recommendations
- **Set the three secrets in production.** Unset = random per‑process (dev only). Rotate the demo tokens.
- Response‑signing uses a shared secret, so it's advisory on the client; **HTTPS is the real transport guarantee** — terminate TLS in front of the server (Replit does this).
- Rate limiting / nonce cache / sessions are **in‑memory**; for multi‑instance deployments move them to Redis.
- The `payment` module remains a **stub** (fake provider URLs) — it compiles and validates keys but performs no real charges.
- Frontend build shows only cosmetic Svelte a11y warnings (section `<label>`s not tied to inputs).
