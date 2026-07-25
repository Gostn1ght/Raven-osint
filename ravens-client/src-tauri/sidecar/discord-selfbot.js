// ═══════════════════════════════════════════════════════════════════════════════
// Ravens Nexus — Discord self-bot sidecar (discord.js-selfbot-v13)
// ═══════════════════════════════════════════════════════════════════════════════
// WARNING: discord.js-selfbot-v13 automates a USER account, which violates Discord's
// Terms of Service and can get the account permanently banned. This runs only when the
// operator supplies their own token and accepts the risk. No token is ever hard-coded.
//
// Protocol: newline-delimited JSON over stdio (one JSON object per line).
//   IN : {"id":1,"cmd":"login","token":"..."}
//        {"id":2,"cmd":"listChannels"}
//        {"id":3,"cmd":"send","channelId":"123","content":"text"}
//        {"id":4,"cmd":"status"}
//        {"id":5,"cmd":"logout"}
//   OUT: {"id":1,"ok":true,"result":{...}} | {"id":1,"ok":false,"error":"..."}
//        {"event":"ready","user":"name#0000"}   (unsolicited)
//        {"event":"boot","ok":true}             (on start)

let Client;
try {
  ({ Client } = require('discord.js-selfbot-v13'));
} catch (e) {
  // Dependency missing — report and keep the pipe alive so the host sees a clean error.
  process.stdout.write(JSON.stringify({ event: 'boot', ok: false, error: 'discord.js-selfbot-v13 not installed' }) + '\n');
}

let client = null;
let ready = false;

function send(obj) {
  try { process.stdout.write(JSON.stringify(obj) + '\n'); } catch (_) { /* ignore */ }
}

function destroyClient() {
  if (client) { try { client.destroy(); } catch (_) {} }
  client = null;
  ready = false;
}

async function handle(msg) {
  const id = msg.id;
  const cmd = msg.cmd;
  try {
    switch (cmd) {
      case 'status':
        return send({ id, ok: true, result: { loggedIn: !!client, ready } });

      case 'logout':
        destroyClient();
        return send({ id, ok: true, result: true });

      case 'login': {
        if (!Client) throw new Error('discord.js-selfbot-v13 not installed');
        if (!msg.token) throw new Error('token required');
        destroyClient();
        client = new Client({ checkUpdate: false });
        client.once('ready', () => {
          ready = true;
          send({ event: 'ready', user: client.user ? client.user.tag : null, id: client.user ? client.user.id : null });
        });
        await client.login(msg.token);
        return send({ id, ok: true, result: { user: client.user ? client.user.tag : null } });
      }

      case 'listChannels': {
        if (!client || !ready) throw new Error('not logged in');
        const out = [];
        for (const guild of client.guilds.cache.values()) {
          for (const ch of guild.channels.cache.values()) {
            // Text-capable guild channels only (text + announcement).
            if (ch.type === 'GUILD_TEXT' || ch.type === 'GUILD_NEWS') {
              out.push({ id: ch.id, name: ch.name, guild: guild.name });
            }
          }
        }
        out.sort((a, b) => (a.guild + a.name).localeCompare(b.guild + b.name));
        return send({ id, ok: true, result: out });
      }

      case 'send': {
        if (!client || !ready) throw new Error('not logged in');
        if (!msg.channelId) throw new Error('channelId required');
        const ch = await client.channels.fetch(String(msg.channelId));
        if (!ch || typeof ch.send !== 'function') throw new Error('channel not sendable');
        const content = (msg.content == null ? '' : String(msg.content)).slice(0, 2000);
        if (!content) throw new Error('empty content');
        await ch.send(content);
        return send({ id, ok: true, result: true });
      }

      default:
        return send({ id, ok: false, error: 'unknown cmd: ' + cmd });
    }
  } catch (e) {
    return send({ id, ok: false, error: String(e && e.message ? e.message : e) });
  }
}

let buf = '';
process.stdin.setEncoding('utf8');
process.stdin.on('data', (chunk) => {
  buf += chunk;
  let nl;
  while ((nl = buf.indexOf('\n')) >= 0) {
    const line = buf.slice(0, nl).trim();
    buf = buf.slice(nl + 1);
    if (!line) continue;
    let msg;
    try { msg = JSON.parse(line); } catch (_) { send({ ok: false, error: 'bad json' }); continue; }
    handle(msg);
  }
});
process.stdin.on('end', () => { destroyClient(); process.exit(0); });

send({ event: 'boot', ok: true });
