// ravens-server/src/main.rs
// Ravens Nexus License Server — Rust + Axum

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Json, Html},
    routing::{delete, get, post, put},
    Router,
};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};
use uuid::Uuid;
use tower_http::{cors::CorsLayer, trace::TraceLayer};

// ── Config ────────────────────────────────────────────────────────────────────
const ADMIN_SECRET: &str = "M27361HD652hs76766yde28hjdhj87w4h32hoi_sifu849wu3j4897riuyfgihfj__MAGAY__uhfuyqe3r8y9qr389YQR390uredqwfUIOJEAWFpidor276374R627QRDHIUAR";
const SERVER_PORT: u16 = 3000;

// ── Embedded Admin UI ─────────────────────────────────────────────────────────
const ADMIN_UI_HTML: &str = r##"<!doctype html>
<html lang="ru">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width,initial-scale=1" />
  <title>Ravens Nexus — Admin</title>
  <style>
    :root{
      --bg:#0a0b0f;--panel:#111318;--panel2:#0d0e13;--border:#1e2130;
      --text:#e5e7eb;--muted:#6b7280;--accent:#c0392b;--ok:#22c55e;--warn:#f59e0b;--err:#ef4444;--info:#3b82f6;
      --mono: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", "Courier New", monospace;
      --sans: ui-sans-serif, system-ui, -apple-system, Segoe UI, Roboto, Inter, Arial;
    }
    *{box-sizing:border-box}
    body{margin:0;background:radial-gradient(1000px 700px at 70% 20%, rgba(192,57,43,.10), transparent 55%), var(--bg); color:var(--text); font-family:var(--sans);}
    a{color:var(--info);text-decoration:none} a:hover{text-decoration:underline}

    .topbar{position:sticky;top:0;z-index:10;display:flex;align-items:center;gap:12px;padding:10px 14px;background:rgba(13,14,19,.92);backdrop-filter:blur(8px);border-bottom:1px solid var(--border)}
    .brand{display:flex;align-items:center;gap:10px}
    .logo{width:34px;height:34px;border-radius:10px;background:var(--accent);box-shadow:0 0 18px rgba(192,57,43,.35);display:grid;place-items:center;font-weight:900;font-family:var(--mono);letter-spacing:1px}
    .brand h1{margin:0;font-size:12px;letter-spacing:4px;font-family:var(--mono)}
    .brand .sub{font-size:9px;color:var(--muted);font-family:var(--mono);letter-spacing:2px}

    .pill{padding:4px 8px;border:1px solid var(--border);border-radius:999px;font-size:10px;font-family:var(--mono);color:var(--muted)}
    .pill.ok{border-color:rgba(34,197,94,.35);color:var(--ok)}
    .pill.err{border-color:rgba(239,68,68,.35);color:var(--err)}

    .wrap{display:grid;grid-template-columns:260px 1fr;min-height:calc(100vh - 56px)}
    .nav{border-right:1px solid var(--border);background:rgba(13,14,19,.85);padding:12px}
    .nav button{width:100%;text-align:left;background:transparent;border:1px solid transparent;color:var(--muted);padding:10px 10px;border-radius:8px;cursor:pointer;font-family:var(--mono);font-size:11px;letter-spacing:1px;display:flex;justify-content:space-between;align-items:center}
    .nav button:hover{background:var(--panel);color:var(--text);border-color:var(--border)}
    .nav button.active{background:rgba(192,57,43,.08);border-color:rgba(192,57,43,.35);color:var(--text)}

    .main{padding:16px;overflow:auto}
    .card{background:rgba(17,19,24,.92);border:1px solid var(--border);border-radius:14px;box-shadow:0 10px 30px rgba(0,0,0,.35)}
    .card .hd{padding:14px 14px 10px;border-bottom:1px solid var(--border);display:flex;align-items:center;justify-content:space-between;gap:10px}
    .card .hd h2{margin:0;font-size:12px;font-family:var(--mono);letter-spacing:2px}
    .card .bd{padding:14px}

    .row{display:flex;gap:10px;flex-wrap:wrap}
    .field{display:flex;flex-direction:column;gap:6px;min-width:220px;flex:1}
    label{font-size:10px;color:var(--muted);font-family:var(--mono);letter-spacing:1px}
    input,select,textarea{background:#0d0e13;border:1px solid var(--border);border-radius:10px;color:var(--text);padding:10px 10px;font-family:var(--mono);font-size:11px;outline:none}
    input:focus,select:focus,textarea:focus{border-color:rgba(192,57,43,.6);box-shadow:0 0 0 3px rgba(192,57,43,.12)}
    textarea{min-height:90px;resize:vertical}

    .btn{border:none;border-radius:12px;padding:10px 12px;font-family:var(--mono);font-size:11px;letter-spacing:1px;cursor:pointer;color:white;background:var(--accent);box-shadow:0 0 18px rgba(192,57,43,.25)}
    .btn.secondary{background:transparent;border:1px solid var(--border);color:var(--muted);box-shadow:none}
    .btn.danger{background:var(--err)}
    .btn:disabled{opacity:.5;cursor:not-allowed}

    table{width:100%;border-collapse:separate;border-spacing:0 8px}
    th{font-size:10px;color:var(--muted);font-family:var(--mono);letter-spacing:1px;text-align:left;padding:0 10px}
    td{background:#0d0e13;border:1px solid var(--border);padding:10px;border-left:none;border-right:none;font-family:var(--mono);font-size:11px}
    tr td:first-child{border-left:1px solid var(--border);border-top-left-radius:12px;border-bottom-left-radius:12px}
    tr td:last-child{border-right:1px solid var(--border);border-top-right-radius:12px;border-bottom-right-radius:12px}

    .k{color:var(--muted)}
    .tag{display:inline-flex;gap:6px;align-items:center;border:1px solid var(--border);border-radius:999px;padding:3px 8px;font-size:10px;color:var(--muted)}
    .tag .dot{width:6px;height:6px;border-radius:50%}

    .grid2{display:grid;grid-template-columns:1fr 1fr;gap:12px}
    @media (max-width: 980px){ .wrap{grid-template-columns:1fr} .nav{position:sticky;top:56px;display:flex;gap:8px;overflow:auto} .nav button{min-width:190px} .grid2{grid-template-columns:1fr} }

    .toast{position:fixed;right:14px;bottom:14px;background:#0d0e13;border:1px solid var(--border);border-radius:12px;padding:10px 12px;font-family:var(--mono);font-size:11px;max-width:360px;box-shadow:0 18px 50px rgba(0,0,0,.5);display:none}
    .toast.show{display:block}
    .muted{color:var(--muted)}

    /* Signature: subtle scanline */
    .scanlines:before{content:"";position:fixed;inset:0;pointer-events:none;background:linear-gradient(rgba(255,255,255,.04), transparent 2px);background-size:100% 3px;mix-blend-mode:overlay;opacity:.07;}
  </style>
</head>
<body class="scanlines">
  <div class="topbar">
    <div class="brand">
      <div class="logo">R</div>
      <div>
        <h1>RAVENS NEXUS</h1>
        <div class="sub">LICENSE SERVER · ADMIN CONSOLE</div>
      </div>
    </div>
    <div class="pill" id="healthPill">HEALTH: —</div>
    <div style="flex:1"></div>
    <div class="pill" id="authPill">AUTH: —</div>
  </div>

  <div class="wrap">
    <div class="nav">
      <button data-view="dashboard" class="active">⬡ DASHBOARD <span class="muted">/ RU+EN</span></button>
      <button data-view="tokens">⬡ TOKENS <span class="muted">/ Создать</span></button>
      <button data-view="users">⬡ USERS <span class="muted">/ Бан</span></button>
      <button data-view="plans">⬡ PLANS <span class="muted">/ Лимиты</span></button>
      <button data-view="audit">⬡ AUDIT <span class="muted">/ Логи</span></button>
      <button data-view="settings">⬡ SETTINGS <span class="muted">/ Admin</span></button>
    </div>

    <div class="main">
      <!-- Login overlay -->
      <div class="card" id="loginCard" style="max-width:520px;margin:0 auto;display:none">
        <div class="hd"><h2>ADMIN LOGIN</h2><span class="pill">LOCAL</span></div>
        <div class="bd">
          <div class="row">
            <div class="field">
              <label>Admin Secret</label>
              <input id="adminSecret" placeholder="M27361HD652hs76766yde28hjdhj87w4h32hoi_sifu849wu3j4897riuyfgihfj__MAGAY__uhfuyqe3r8y9qr389YQR390uredqwfUIOJEAWFpidor276374R627QRDHIUAR" />
              <div class="muted" style="font-size:11px">RU: Секрет нужен только админам. Не выдавай клиентам.</div>
              <div class="muted" style="font-size:11px">EN: Secret is admin-only. Never ship to clients.</div>
            </div>
          </div>
          <div style="margin-top:12px" class="row">
            <button class="btn" id="btnLogin">Login</button>
            <button class="btn secondary" id="btnLoginDemo">Use demo</button>
          </div>
        </div>
      </div>

      <!-- Views -->
      <div id="viewRoot"></div>
    </div>
  </div>

  <div class="toast" id="toast"></div>

<script>
  const state = {
    view: 'dashboard',
    adminSecret: '',
    baseUrl: location.origin,
    plans: [
      {id:'free', name:'FREE', requests:50, modules:['social','hibp','ip','whois','dorks'], ai:false, price:'0₽'},
      {id:'pro', name:'PRO', requests:1000, modules:['social','hibp','ip','whois','dorks','paste','darkweb','phone','intelx','ai'], ai:true, price:'990₽'},
      {id:'elite', name:'ELITE', requests:-1, modules:['*'], ai:true, price:'2990₽'},
    ],
    users: [],
    tokens: [],
    audit: [],
  };

  const $ = (sel, el=document) => el.querySelector(sel);
  const $$ = (sel, el=document) => Array.from(el.querySelectorAll(sel));

  function toast(msg){
    const t = $('#toast');
    t.textContent = msg;
    t.classList.add('show');
    setTimeout(()=>t.classList.remove('show'), 2200);
  }

  async function api(path, opts={}){
    const headers = Object.assign({'Content-Type':'application/json'}, opts.headers||{});
    if (state.adminSecret) headers['x-admin-secret'] = state.adminSecret;
    const res = await fetch(state.baseUrl + path, Object.assign({}, opts, {headers}));
    let data = null;
    try{ data = await res.json(); } catch { data = null; }
    if (!res.ok) throw new Error((data && data.error) ? data.error : `HTTP ${res.status}`);
    return data;
  }

  function setAuthPill(){
    const pill = $('#authPill');
    if (state.adminSecret){
      pill.textContent = 'AUTH: OK';
      pill.className = 'pill ok';
    } else {
      pill.textContent = 'AUTH: REQUIRED';
      pill.className = 'pill err';
    }
  }

  async function refreshHealth(){
    try{
      const d = await api('/health', {headers:{}});
      $('#healthPill').textContent = 'HEALTH: OK';
      $('#healthPill').className = 'pill ok';
    }catch(e){
      $('#healthPill').textContent = 'HEALTH: DOWN';
      $('#healthPill').className = 'pill err';
    }
  }

  function navInit(){
    $$('.nav button').forEach(btn=>{
      btn.addEventListener('click', ()=>{
        $$('.nav button').forEach(b=>b.classList.remove('active'));
        btn.classList.add('active');
        state.view = btn.dataset.view;
        render();
      });
    });
  }

  function requireAuth(){
    if (!state.adminSecret){
      $('#loginCard').style.display = 'block';
      $('#viewRoot').innerHTML = '';
      return true;
    }
    $('#loginCard').style.display = 'none';
    return false;
  }

  function fmtInf(n){ return n === -1 ? '∞' : String(n); }

  function renderDashboard(){
    return `
      <div class="grid2">
        <div class="card">
          <div class="hd"><h2>DASHBOARD</h2><span class="pill">RU+EN</span></div>
          <div class="bd" id="dashStats">Loading...</div>
        </div>
        <div class="card">
          <div class="hd"><h2>QUICK ACTIONS</h2><span class="pill">TOKENS</span></div>
          <div class="bd">
            <div class="row">
              <button class="btn" id="qaCreatePro">Create PRO token</button>
              <button class="btn secondary" id="qaRefresh">Refresh</button>
            </div>
            <div class="muted" style="margin-top:10px">RU: Быстрые действия для админа.</div>
            <div class="muted">EN: Quick admin shortcuts.</div>
          </div>
        </div>
      </div>

      <div style="height:12px"></div>

      <div class="card">
        <div class="hd"><h2>RECENT TOKENS</h2><span class="pill">LAST 50</span></div>
        <div class="bd" id="recentTokens"></div>
      </div>
    `;
  }

  function renderTokens(){
    return `
      <div class="card">
        <div class="hd"><h2>TOKENS</h2><span class="pill">CREATE / REVOKE</span></div>
        <div class="bd">
          <div class="row">
            <div class="field">
              <label>Tier</label>
              <select id="tokTier">
                <option value="free">FREE</option>
                <option value="pro">PRO</option>
                <option value="elite">ELITE</option>
                <option value="admin">ADMIN</option>
              </select>
            </div>
            <div class="field">
              <label>Expires (days) / Срок</label>
              <input id="tokExp" placeholder="30" />
            </div>
            <div class="field" style="flex:2">
              <label>Note / Комментарий</label>
              <input id="tokNote" placeholder="customer / invoice / etc" />
            </div>
          </div>
          <div class="row" style="margin-top:10px">
            <button class="btn" id="btnCreateToken">Create token</button>
            <button class="btn secondary" id="btnReloadTokens">Reload list</button>
          </div>
        </div>
      </div>

      <div style="height:12px"></div>

      <div class="card">
        <div class="hd"><h2>ALL TOKENS</h2><span class="pill">{state.tokens.length}</span></div>
        <div class="bd" id="tokensTable"></div>
      </div>
    `;
  }

  function renderUsers(){
    return `
      <div class="card">
        <div class="hd"><h2>USERS</h2><span class="pill">PLANNED</span></div>
        <div class="bd">
          <div class="muted">RU: Пользователи появятся после подключения SQLite и сущности users.</div>
          <div class="muted">EN: Users will appear after SQLite + users table is implemented.</div>
          <div style="height:10px"></div>
          <div class="row">
            <button class="btn secondary" id="btnUserScaffold">Scaffold users DB</button>
          </div>
        </div>
      </div>
    `;
  }

  function renderPlans(){
    const rows = state.plans.map(p=>`
      <tr>
        <td><b>${p.name}</b></td>
        <td>${fmtInf(p.requests)}</td>
        <td>${p.ai ? '✓' : '✗'}</td>
        <td>${p.price}</td>
        <td><button class="btn secondary" data-edit-plan="${p.id}">Edit</button></td>
      </tr>
    `).join('');
    return `
      <div class="card">
        <div class="hd"><h2>PLANS</h2><span class="pill">LOCAL CONFIG</span></div>
        <div class="bd">
          <table>
            <thead><tr><th>Plan</th><th>Requests</th><th>AI</th><th>Price</th><th>Action</th></tr></thead>
            <tbody>${rows}</tbody>
          </table>
          <div class="muted">RU: Сейчас планы в UI локальные (демо). На следующем шаге вынесем в SQLite и API.</div>
          <div class="muted">EN: Plans are local demo. Next step: store in SQLite + API.</div>
        </div>
      </div>
    `;
  }

  function renderAudit(){
    return `
      <div class="card">
        <div class="hd"><h2>AUDIT LOG</h2><span class="pill">PLANNED</span></div>
        <div class="bd">
          <div class="muted">RU: Логи появятся после добавления записи событий (activate/consume/admin) в SQLite.</div>
          <div class="muted">EN: Logs after writing events to SQLite.</div>
          <div style="height:10px"></div>
          <button class="btn secondary" id="btnAuditScaffold">Scaffold audit DB</button>
        </div>
      </div>
    `;
  }

  function renderSettings(){
    return `
      <div class="card">
        <div class="hd"><h2>SETTINGS</h2><span class="pill">ADMIN</span></div>
        <div class="bd">
          <div class="row">
            <div class="field">
              <label>Base URL</label>
              <input id="baseUrl" value="${state.baseUrl}" />
            </div>
            <div class="field">
              <label>Admin Secret</label>
              <input id="adminSecret2" value="${state.adminSecret}" placeholder="x-admin-secret" />
            </div>
          </div>
          <div class="row" style="margin-top:10px">
            <button class="btn" id="btnSaveSettings">Save</button>
            <button class="btn secondary" id="btnLogout">Logout</button>
          </div>
          <div class="muted" style="margin-top:10px">RU: Это локальные настройки браузера (localStorage).</div>
          <div class="muted">EN: Stored locally in browser (localStorage).</div>
        </div>
      </div>
    `;
  }

  function render(){
    setAuthPill();
    if (requireAuth()) return;

    const root = $('#viewRoot');
    if (state.view === 'dashboard') root.innerHTML = renderDashboard();
    if (state.view === 'tokens') root.innerHTML = renderTokens();
    if (state.view === 'users') root.innerHTML = renderUsers();
    if (state.view === 'plans') root.innerHTML = renderPlans();
    if (state.view === 'audit') root.innerHTML = renderAudit();
    if (state.view === 'settings') root.innerHTML = renderSettings();

    bindView();
  }

  async function loadTokens(){
    state.tokens = await api('/admin/tokens');
  }

  function tokensTableHtml(){
    const rows = state.tokens
      .sort((a,b)=> (b.created_at||'').localeCompare(a.created_at||''))
      .slice(0, 200)
      .map(t=>{
        const tierColor = t.tier === 'ADMIN' ? 'var(--warn)' : t.tier === 'ELITE' ? 'var(--accent)' : t.tier === 'PRO' ? 'var(--info)' : 'var(--muted)';
        const activeDot = t.active ? 'var(--ok)' : 'var(--err)';
        const rem = t.requests_limit === -1 ? '∞' : t.requests_remaining;
        return `
          <tr>
            <td><span class="tag"><span class="dot" style="background:${activeDot}"></span>${t.token}</span></td>
            <td><span style="color:${tierColor}"><b>${t.tier}</b></span></td>
            <td class="k">${t.hwid ? t.hwid.slice(0,10)+'…' : '—'}</td>
            <td>${rem}</td>
            <td class="k">${t.expires_at ? t.expires_at.slice(0,10) : '∞'}</td>
            <td>
              <button class="btn secondary" data-copy="${t.token}">Copy</button>
              <button class="btn danger" data-revoke="${t.token}">Revoke</button>
            </td>
          </tr>
        `;
      }).join('');

    return `
      <table>
        <thead>
          <tr>
            <th>Token</th><th>Tier</th><th>HWID</th><th>Remaining</th><th>Expires</th><th>Actions</th>
          </tr>
        </thead>
        <tbody>${rows}</tbody>
      </table>
      <div class="muted">RU: Token привязывается к HWID при первой активации. Revoke отключает токен.</div>
      <div class="muted">EN: Token binds to HWID on first activation. Revoke disables token.</div>
    `;
  }

  async function bindDashboard(){
    try{
      const stats = await api('/admin/stats');
      $('#dashStats').innerHTML = `
        <div class="row">
          <div class="field"><label>Total tokens</label><div style="font-family:var(--mono);font-size:18px">${stats.total_tokens}</div></div>
          <div class="field"><label>Active tokens</label><div style="font-family:var(--mono);font-size:18px;color:var(--ok)">${stats.active_tokens}</div></div>
          <div class="field"><label>By tier</label><div style="font-family:var(--mono);font-size:11px;color:var(--muted)">${Object.entries(stats.by_tier).map(([k,v])=>`${k}:${v}`).join(' · ')}</div></div>
        </div>
      `;
    }catch(e){
      $('#dashStats').innerHTML = `<div class="muted">${e.message}</div>`;
    }

    try{
      await loadTokens();
      $('#recentTokens').innerHTML = tokensTableHtml();
      $$('#recentTokens [data-revoke]').forEach(b=>b.addEventListener('click', revokeToken));
      $$('#recentTokens [data-copy]').forEach(b=>b.addEventListener('click', copyToken));
    }catch(e){
      $('#recentTokens').innerHTML = `<div class="muted">${e.message}</div>`;
    }

    $('#qaCreatePro')?.addEventListener('click', async ()=>{
      try{
        const r = await api('/admin/tokens', {method:'POST', body: JSON.stringify({tier:'pro', expires_days:30, note:'quick'})});
        toast('Created: ' + r.token);
        await bindDashboard();
      }catch(e){ toast(e.message); }
    });
    $('#qaRefresh')?.addEventListener('click', async ()=>{ await bindDashboard(); toast('Refreshed'); });
  }

  async function createToken(){
    const tier = $('#tokTier').value;
    const exp = $('#tokExp').value.trim();
    const note = $('#tokNote').value.trim();
    const payload = { tier, note: note || undefined, expires_days: exp ? Number(exp) : undefined };
    try{
      const r = await api('/admin/tokens', {method:'POST', body: JSON.stringify(payload)});
      toast('Created: ' + r.token);
      await loadTokens();
      $('#tokensTable').innerHTML = tokensTableHtml();
      bindTokenTableActions();
    }catch(e){ toast(e.message); }
  }

  async function revokeToken(e){
    const token = e.currentTarget.dataset.revoke;
    if (!confirm('Revoke token? ' + token)) return;
    try{ await api('/admin/tokens/' + encodeURIComponent(token), {method:'DELETE'}); toast('Revoked'); await loadTokens(); render(); }
    catch(err){ toast(err.message); }
  }

  async function copyToken(e){
    const token = e.currentTarget.dataset.copy;
    await navigator.clipboard.writeText(token);
    toast('Copied');
  }

  function bindTokenTableActions(){
    $$('#tokensTable [data-revoke]').forEach(b=>b.addEventListener('click', revokeToken));
    $$('#tokensTable [data-copy]').forEach(b=>b.addEventListener('click', copyToken));
  }

  async function bindTokens(){
    await loadTokens();
    $('#tokensTable').innerHTML = tokensTableHtml();
    bindTokenTableActions();
    $('#btnCreateToken').addEventListener('click', createToken);
    $('#btnReloadTokens').addEventListener('click', async ()=>{ await loadTokens(); $('#tokensTable').innerHTML = tokensTableHtml(); bindTokenTableActions(); toast('Reloaded'); });
  }

  function bindSettings(){
    $('#btnSaveSettings').addEventListener('click', ()=>{
      state.baseUrl = $('#baseUrl').value.trim() || location.origin;
      state.adminSecret = $('#adminSecret2').value.trim();
      localStorage.setItem('rvn_admin_secret', state.adminSecret);
      localStorage.setItem('rvn_base_url', state.baseUrl);
      toast('Saved');
      render();
    });
    $('#btnLogout').addEventListener('click', ()=>{
      state.adminSecret = '';
      localStorage.removeItem('rvn_admin_secret');
      toast('Logged out');
      render();
    });
  }

  function bindView(){
    if (state.view === 'dashboard') bindDashboard();
    if (state.view === 'tokens') bindTokens();
    if (state.view === 'settings') bindSettings();

    $('#btnUserScaffold')?.addEventListener('click', ()=>toast('Next step: SQLite users table'));
    $('#btnAuditScaffold')?.addEventListener('click', ()=>toast('Next step: SQLite audit table'));
  }

  function initLogin(){
    $('#btnLogin').addEventListener('click', ()=>{
      state.adminSecret = $('#adminSecret').value.trim();
      localStorage.setItem('rvn_admin_secret', state.adminSecret);
      toast('Auth set');
      render();
    });
    $('#btnLoginDemo').addEventListener('click', ()=>{
      state.adminSecret = 'M27361HD652hs76766yde28hjdhj87w4h32hoi_sifu849wu3j4897riuyfgihfj__MAGAY__uhfuyqe3r8y9qr389YQR390uredqwfUIOJEAWFpidor276374R627QRDHIUAR';
      localStorage.setItem('rvn_admin_secret', state.adminSecret);
      toast('Demo secret set');
      render();
    });
  }

  // Boot
  state.adminSecret = localStorage.getItem('rvn_admin_secret') || '';
  state.baseUrl = localStorage.getItem('rvn_base_url') || location.origin;
  navInit();
  initLogin();
  setAuthPill();
  refreshHealth();
  setInterval(refreshHealth, 5000);
  render();
</script>
</body>
</html>
"##;


// ── Constant-time compare (best-effort) ─────────────────────────────────────
fn ct_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() { return false; }
    let mut diff: u8 = 0;
    for (x, y) in a.as_bytes().iter().zip(b.as_bytes().iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

// ── Env ─────────────────────────────────────────────────────────────────────
fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}
fn env_or_u16(key: &str, default: u16) -> u16 {
    std::env::var(key).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

// ── Tier ──────────────────────────────────────────────────────────────────────
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    Free,
    Pro,
    Elite,
    Admin,
}

impl Tier {
    pub fn monthly_requests(&self) -> i64 {
        match self {
            Tier::Free  => 5,
            Tier::Pro   => 1000,
            Tier::Elite => -1, // unlimited
            Tier::Admin => -1,
        }
    }
    pub fn name(&self) -> &str {
        match self {
            Tier::Free  => "FREE",
            Tier::Pro   => "PRO",
            Tier::Elite => "ELITE",
            Tier::Admin => "ADMIN",
        }
    }
}

// ── License record ────────────────────────────────────────────────────────────
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct License {
    pub token: String,
    pub tier: Tier,
    pub hwid: Option<String>,       // bound after first activation
    pub requests_used: i64,
    pub requests_limit: i64,        // -1 = unlimited
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub active: bool,
    pub note: String,
}

impl License {
    pub fn new(tier: Tier, expires_days: Option<i64>, note: &str) -> Self {
        let token = format!("RVN-{}", Uuid::new_v4().to_string().to_uppercase().replace('-', "-").chars().take(19).collect::<String>());
        let limit = tier.monthly_requests();
        let expires_at = expires_days.map(|d| Utc::now() + Duration::days(d));
        Self {
            token,
            tier,
            hwid: None,
            requests_used: 0,
            requests_limit: limit,
            created_at: Utc::now(),
            expires_at,
            active: true,
            note: note.to_string(),
        }
    }

    pub fn is_expired(&self) -> bool {
        if let Some(exp) = self.expires_at {
            return Utc::now() > exp;
        }
        false
    }

    pub fn requests_remaining(&self) -> i64 {
        if self.requests_limit == -1 { return i64::MAX; }
        (self.requests_limit - self.requests_used).max(0)
    }
}

// ── App State ─────────────────────────────────────────────────────────────────
#[derive(Default)]
pub struct AppState {
    pub licenses: RwLock<HashMap<String, License>>,
    pub hwid_index: RwLock<HashMap<String, String>>, // hwid -> token
}

type SharedState = Arc<AppState>;

// ── Request/Response types ────────────────────────────────────────────────────
#[derive(Deserialize)]
pub struct ActivateRequest {
    pub token: String,
    pub hwid: String,
}

#[derive(Serialize)]
pub struct ActivateResponse {
    pub ok: bool,
    pub tier: String,
    pub requests_remaining: i64,
    pub expires: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Deserialize)]
pub struct ConsumeRequest {
    pub token: String,
    pub hwid: String,
    pub module: String,
}

#[derive(Serialize)]
pub struct ConsumeResponse {
    pub ok: bool,
    pub requests_remaining: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Deserialize)]
pub struct CreateTokenRequest {
    pub tier: String,
    pub expires_days: Option<i64>,
    pub note: Option<String>,
}

#[derive(Serialize)]
pub struct CreateTokenResponse {
    pub token: String,
    pub tier: String,
    pub expires_days: Option<i64>,
}

#[derive(Serialize)]
pub struct LicenseInfo {
    pub token: String,
    pub tier: String,
    pub hwid: Option<String>,
    pub requests_used: i64,
    pub requests_limit: i64,
    pub requests_remaining: i64,
    pub expires_at: Option<String>,
    pub active: bool,
    pub note: String,
}

impl From<&License> for LicenseInfo {
    fn from(l: &License) -> Self {
        Self {
            token: l.token.clone(),
            tier: l.tier.name().to_string(),
            hwid: l.hwid.clone(),
            requests_used: l.requests_used,
            requests_limit: l.requests_limit,
            requests_remaining: l.requests_remaining(),
            expires_at: l.expires_at.map(|d| d.to_rfc3339()),
            active: l.active,
            note: l.note.clone(),
        }
    }
}

// ── Admin auth middleware ─────────────────────────────────────────────────────
async fn admin_auth(
    headers: HeaderMap,
    req: axum::http::Request<axum::body::Body>,
    next: Next,
) -> impl IntoResponse {
    let auth = headers
        .get("x-admin-secret")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let secret = env_or("RAVENS_ADMIN_SECRET", ADMIN_SECRET);
    if !ct_eq(auth, &secret) {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error":"Unauthorized"}))).into_response();
    }
    next.run(req).await
}

// ── Handlers ──────────────────────────────────────────────────────────────────

// POST /api/license/activate
async fn activate_license(
    State(state): State<SharedState>,
    Json(req): Json<ActivateRequest>,
) -> impl IntoResponse {
    let token = req.token.trim().to_uppercase();
    let hwid = hash_hwid(&req.hwid);

    // Check if HWID already has a license
    {
        let hwid_idx = state.hwid_index.read().unwrap();
        if let Some(existing_token) = hwid_idx.get(&hwid) {
            if existing_token != &token {
                return Json(ActivateResponse {
                    ok: false,
                    tier: String::new(),
                    requests_remaining: 0,
                    expires: None,
                    error: Some("HWID уже привязан к другому токену".into()),
                });
            }
        }
    }

    let mut licenses = state.licenses.write().unwrap();
    let license = match licenses.get_mut(&token) {
        Some(l) => l,
        None => return Json(ActivateResponse {
            ok: false,
            tier: String::new(),
            requests_remaining: 0,
            expires: None,
            error: Some("Токен не найден".into()),
        }),
    };

    if !license.active {
        return Json(ActivateResponse {
            ok: false, tier: String::new(), requests_remaining: 0, expires: None,
            error: Some("Токен деактивирован".into()),
        });
    }
    if license.is_expired() {
        return Json(ActivateResponse {
            ok: false, tier: String::new(), requests_remaining: 0, expires: None,
            error: Some("Токен истёк".into()),
        });
    }

    // Bind HWID on first activation
    if license.hwid.is_none() {
        license.hwid = Some(hwid.clone());
        drop(licenses);
        state.hwid_index.write().unwrap().insert(hwid, token.clone());
        licenses = state.licenses.write().unwrap();
    } else if license.hwid.as_deref() != Some(&hwid) {
        return Json(ActivateResponse {
            ok: false, tier: String::new(), requests_remaining: 0, expires: None,
            error: Some("Токен привязан к другому устройству".into()),
        });
    }

    let license = licenses.get(&token).unwrap();
    Json(ActivateResponse {
        ok: true,
        tier: license.tier.name().to_string(),
        requests_remaining: license.requests_remaining(),
        expires: license.expires_at.map(|d| d.format("%Y-%m-%d").to_string()),
        error: None,
    })
}

// POST /api/license/consume
async fn consume_request(
    State(state): State<SharedState>,
    Json(req): Json<ConsumeRequest>,
) -> impl IntoResponse {
    let token = req.token.trim().to_uppercase();
    let hwid = hash_hwid(&req.hwid);
    let mut licenses = state.licenses.write().unwrap();
    let license = match licenses.get_mut(&token) {
        Some(l) => l,
        None => return Json(ConsumeResponse { ok: false, requests_remaining: 0, error: Some("Токен не найден".into()) }),
    };
    if !license.active || license.is_expired() {
        return Json(ConsumeResponse { ok: false, requests_remaining: 0, error: Some("Лицензия недействительна".into()) });
    }
    if license.hwid.as_deref() != Some(&hwid) {
        return Json(ConsumeResponse { ok: false, requests_remaining: 0, error: Some("HWID не совпадает".into()) });
    }
    if license.requests_limit != -1 && license.requests_used >= license.requests_limit {
        return Json(ConsumeResponse { ok: false, requests_remaining: 0, error: Some("Лимит запросов исчерпан".into()) });
    }
    if license.requests_limit != -1 {
        license.requests_used += 1;
    }
    let remaining = license.requests_remaining();
    Json(ConsumeResponse { ok: true, requests_remaining: remaining, error: None })
}

// GET /api/license/status/:token
async fn license_status(
    State(state): State<SharedState>,
    Path(token): Path<String>,
) -> impl IntoResponse {
    let token = token.trim().to_uppercase();
    let licenses = state.licenses.read().unwrap();
    match licenses.get(&token) {
        Some(l) => (StatusCode::OK, Json(serde_json::to_value(LicenseInfo::from(l)).unwrap())).into_response(),
        None => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error":"Токен не найден"}))).into_response(),
    }
}

// ── Admin routes ──────────────────────────────────────────────────────────────

// POST /admin/tokens
async fn admin_create_token(
    State(state): State<SharedState>,
    Json(req): Json<CreateTokenRequest>,
) -> impl IntoResponse {
    let tier = match req.tier.to_lowercase().as_str() {
        "pro"   => Tier::Pro,
        "elite" => Tier::Elite,
        "admin" => Tier::Admin,
        _       => Tier::Free,
    };
    let note = req.note.unwrap_or_default();
    let license = License::new(tier.clone(), req.expires_days, &note);
    let token = license.token.clone();
    state.licenses.write().unwrap().insert(token.clone(), license);
    Json(CreateTokenResponse {
        token,
        tier: tier.name().to_string(),
        expires_days: req.expires_days,
    })
}

// GET /admin/tokens
async fn admin_list_tokens(State(state): State<SharedState>) -> impl IntoResponse {
    let licenses = state.licenses.read().unwrap();
    let list: Vec<LicenseInfo> = licenses.values().map(LicenseInfo::from).collect();
    Json(list)
}

// DELETE /admin/tokens/:token
async fn admin_revoke_token(
    State(state): State<SharedState>,
    Path(token): Path<String>,
) -> impl IntoResponse {
    let token = token.trim().to_uppercase();
    let mut licenses = state.licenses.write().unwrap();
    match licenses.get_mut(&token) {
        Some(l) => { l.active = false; Json(serde_json::json!({"ok":true,"message":"Токен деактивирован"})) }
        None => Json(serde_json::json!({"ok":false,"error":"Не найден"})),
    }
}

// PUT /admin/tokens/:token/tier
async fn admin_change_tier(
    State(state): State<SharedState>,
    Path(token): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let token = token.trim().to_uppercase();
    let new_tier = match body["tier"].as_str().unwrap_or("free") {
        "pro"   => Tier::Pro,
        "elite" => Tier::Elite,
        "admin" => Tier::Admin,
        _       => Tier::Free,
    };
    let mut licenses = state.licenses.write().unwrap();
    match licenses.get_mut(&token) {
        Some(l) => {
            l.tier = new_tier.clone();
            l.requests_limit = new_tier.monthly_requests();
            Json(serde_json::json!({"ok":true,"tier":new_tier.name()}))
        }
        None => Json(serde_json::json!({"ok":false,"error":"Не найден"})),
    }
}

// POST /admin/tokens/:token/reset-requests
async fn admin_reset_requests(
    State(state): State<SharedState>,
    Path(token): Path<String>,
) -> impl IntoResponse {
    let token = token.trim().to_uppercase();
    let mut licenses = state.licenses.write().unwrap();
    match licenses.get_mut(&token) {
        Some(l) => { l.requests_used = 0; Json(serde_json::json!({"ok":true})) }
        None => Json(serde_json::json!({"ok":false,"error":"Не найден"})),
    }
}

// GET /admin/stats
async fn admin_stats(State(state): State<SharedState>) -> impl IntoResponse {
    let licenses = state.licenses.read().unwrap();
    let total = licenses.len();
    let active = licenses.values().filter(|l| l.active && !l.is_expired()).count();
    let by_tier = {
        let mut map = HashMap::new();
        for l in licenses.values() {
            *map.entry(l.tier.name()).or_insert(0usize) += 1;
        }
        map
    };
    Json(serde_json::json!({
        "total_tokens": total,
        "active_tokens": active,
        "by_tier": by_tier,
    }))
}

// ── Helpers ───────────────────────────────────────────────────────────────────
fn hash_hwid(hwid: &str) -> String {
    let mut h = Sha256::new();
    h.update(hwid.as_bytes());
    hex::encode(h.finalize())
}

fn seed_demo_tokens(state: &SharedState) {
    let mut licenses = state.licenses.write().unwrap();
    // Demo admin token
    let mut admin = License::new(Tier::Admin, None, "Demo admin");
    admin.token = "RVN-ADMIN-DEMO-0000-0000".to_string();
    licenses.insert(admin.token.clone(), admin);
    // Demo free token
    let free = License::new(Tier::Free, Some(30), "Demo free 30 days");
    licenses.insert(free.token.clone(), free);
}

// ── Main ──────────────────────────────────────────────────────────────────────
#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let state = Arc::new(AppState::default());
    seed_demo_tokens(&state);

    let admin_routes = Router::new()
        .route("/tokens", post(admin_create_token))
        .route("/tokens", get(admin_list_tokens))
        .route("/tokens/:token", delete(admin_revoke_token))
        .route("/tokens/:token/tier", put(admin_change_tier))
        .route("/tokens/:token/reset-requests", post(admin_reset_requests))
        .route("/stats", get(admin_stats))
        .layer(middleware::from_fn(admin_auth));

    let api_routes = Router::new()
        .route("/license/activate", post(activate_license))
        .route("/license/consume", post(consume_request))
        .route("/license/status/:token", get(license_status));

    let cors = CorsLayer::permissive();

    let app = Router::new()
        .route("/", get(|| async { Html(ADMIN_UI_HTML) }))
        .route("/admin-ui", get(|| async { Html(ADMIN_UI_HTML) }))
        .route("/admin-ui/", get(|| async { Html(ADMIN_UI_HTML) }))
        .route("/admin-ui/admin.html", get(|| async { Html(ADMIN_UI_HTML) }))
        .nest("/admin", admin_routes)
        .nest("/api", api_routes)
        .route("/health", get(|| async { Json(serde_json::json!({"status":"ok","service":"ravens-nexus-server"})) }))
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state);

    let port = env_or_u16("RAVENS_SERVER_PORT", SERVER_PORT);
    let addr = format!("0.0.0.0:{}", port);
    tracing::info!("Ravens Nexus Server starting on {}", addr);
    let local_url = format!("http://127.0.0.1:{}/admin-ui/admin.html", port);
    tracing::info!("Admin Panel (Local): {}", local_url);
    tracing::info!("Health: http://127.0.0.1:{}/health", port);
    tracing::info!("Admin secret: {}", ADMIN_SECRET);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
