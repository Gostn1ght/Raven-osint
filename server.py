import os
import re
import json
from flask import Flask, request, Response, stream_with_context

app = Flask(__name__)

PORT = 5000
HOST = "0.0.0.0"

SYSTEM_PROMPT = """You are an expert OSINT (Open-Source Intelligence) analyst powered by NVIDIA NIM. You help investigators find, analyze, and correlate publicly available information. Your capabilities include:

- **Person investigation**: Finding social media accounts, public records, email addresses, phone numbers
- **Domain/IP analysis**: WHOIS, DNS records, reverse IP, SSL certificates, hosting providers
- **Username search**: Locating accounts across platforms (Sherlock-style)
- **Social media OSINT**: Analyzing profiles, connections, post history, metadata
- **Image OSINT**: Reverse image search strategies, metadata extraction, geolocation clues
- **Company/organization research**: Corporate records, employees, infrastructure
- **Dark web indicators**: Breach data, leaked credentials (public sources only)
- **Geolocation**: Analyzing photos/videos for location clues (GeoSpy, Picarta techniques)
- **Google Dorks**: Crafting advanced search queries for specific targets
- **Report generation**: Structuring findings into professional intelligence reports

Always provide:
1. Step-by-step investigation methodology
2. Specific tools and commands to use
3. Google Dork queries when relevant
4. Legal and ethical reminders
5. Confidence levels for findings

Stay within legal and ethical boundaries. Only use publicly available information sources."""

HTML_PAGE = """<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>AI OSINT Investigator — NVIDIA NIM</title>
  <style>
    *, *::before, *::after { box-sizing: border-box; margin: 0; padding: 0; }

    :root {
      --bg: #0a0e17;
      --surface: #0f1520;
      --surface2: #141d2e;
      --border: #1e2d45;
      --border2: #243350;
      --text: #cdd9e5;
      --text-dim: #6e8099;
      --green: #76b947;
      --blue: #4da3ff;
      --purple: #9d6bff;
      --orange: #ff8c42;
      --red: #ff5e5e;
      --nvidia: #76b947;
    }

    body {
      font-family: 'Segoe UI', system-ui, sans-serif;
      background: var(--bg);
      color: var(--text);
      height: 100vh;
      display: flex;
      flex-direction: column;
      overflow: hidden;
    }

    /* ── TOP BAR ── */
    .topbar {
      background: var(--surface);
      border-bottom: 1px solid var(--border);
      padding: 0 1.5rem;
      height: 52px;
      display: flex;
      align-items: center;
      gap: 1rem;
      flex-shrink: 0;
    }
    .topbar .logo {
      display: flex; align-items: center; gap: 0.5rem;
      font-weight: 700; font-size: 1rem; color: #e6edf3;
      letter-spacing: 0.02em;
    }
    .topbar .logo .badge {
      background: var(--nvidia);
      color: #000;
      font-size: 0.65rem;
      font-weight: 800;
      padding: 2px 7px;
      border-radius: 4px;
      letter-spacing: 0.05em;
    }
    .topbar .model-pill {
      margin-left: auto;
      display: flex; align-items: center; gap: 0.4rem;
      background: var(--surface2);
      border: 1px solid var(--border);
      border-radius: 20px;
      padding: 3px 12px 3px 8px;
      font-size: 0.75rem;
      color: var(--text-dim);
    }
    .topbar .model-pill .dot {
      width: 7px; height: 7px;
      border-radius: 50%;
      background: var(--nvidia);
      box-shadow: 0 0 6px var(--nvidia);
    }
    .topbar .clear-btn {
      background: none;
      border: 1px solid var(--border);
      color: var(--text-dim);
      border-radius: 6px;
      padding: 4px 12px;
      font-size: 0.75rem;
      cursor: pointer;
      transition: all 0.15s;
    }
    .topbar .clear-btn:hover { border-color: var(--red); color: var(--red); }

    /* ── MAIN LAYOUT ── */
    .main {
      display: flex;
      flex: 1;
      overflow: hidden;
    }

    /* ── SIDEBAR ── */
    .sidebar {
      width: 220px;
      background: var(--surface);
      border-right: 1px solid var(--border);
      display: flex;
      flex-direction: column;
      flex-shrink: 0;
      overflow-y: auto;
    }
    .sidebar-section { padding: 0.75rem 0.75rem 0.25rem; }
    .sidebar-label {
      font-size: 0.65rem;
      font-weight: 700;
      color: var(--text-dim);
      text-transform: uppercase;
      letter-spacing: 0.1em;
      margin-bottom: 0.4rem;
    }
    .quick-btn {
      display: flex;
      align-items: center;
      gap: 0.5rem;
      width: 100%;
      background: none;
      border: none;
      color: var(--text);
      font-size: 0.82rem;
      padding: 0.45rem 0.6rem;
      border-radius: 6px;
      cursor: pointer;
      text-align: left;
      transition: background 0.12s;
      margin-bottom: 2px;
    }
    .quick-btn:hover { background: var(--surface2); }
    .quick-btn .icon { width: 18px; text-align: center; font-size: 0.9rem; }
    .sidebar-divider {
      border: none;
      border-top: 1px solid var(--border);
      margin: 0.5rem 0.75rem;
    }
    .sidebar-bottom {
      margin-top: auto;
      padding: 0.75rem;
      font-size: 0.72rem;
      color: var(--text-dim);
      line-height: 1.5;
    }

    /* ── CHAT AREA ── */
    .chat-area {
      flex: 1;
      display: flex;
      flex-direction: column;
      overflow: hidden;
    }

    .messages {
      flex: 1;
      overflow-y: auto;
      padding: 1.5rem;
      display: flex;
      flex-direction: column;
      gap: 1.25rem;
      scroll-behavior: smooth;
    }
    .messages::-webkit-scrollbar { width: 5px; }
    .messages::-webkit-scrollbar-track { background: transparent; }
    .messages::-webkit-scrollbar-thumb { background: var(--border2); border-radius: 10px; }

    /* welcome */
    .welcome {
      margin: auto;
      text-align: center;
      max-width: 480px;
      padding: 2rem;
    }
    .welcome .shield { font-size: 3.5rem; margin-bottom: 1rem; }
    .welcome h2 { color: #e6edf3; font-size: 1.3rem; margin-bottom: 0.5rem; }
    .welcome p { color: var(--text-dim); font-size: 0.88rem; line-height: 1.6; margin-bottom: 1.5rem; }
    .welcome-chips { display: flex; flex-wrap: wrap; gap: 0.4rem; justify-content: center; }
    .chip {
      background: var(--surface2);
      border: 1px solid var(--border);
      color: var(--text);
      border-radius: 20px;
      padding: 0.3rem 0.85rem;
      font-size: 0.78rem;
      cursor: pointer;
      transition: border-color 0.15s, color 0.15s;
    }
    .chip:hover { border-color: var(--blue); color: var(--blue); }

    /* messages */
    .msg { display: flex; flex-direction: column; max-width: 78%; }
    .msg.user { align-self: flex-end; align-items: flex-end; }
    .msg.assistant { align-self: flex-start; align-items: flex-start; }

    .msg-meta {
      font-size: 0.68rem;
      color: var(--text-dim);
      margin-bottom: 0.3rem;
      display: flex;
      align-items: center;
      gap: 0.35rem;
    }
    .msg-meta .avatar {
      width: 18px; height: 18px;
      border-radius: 50%;
      display: flex; align-items: center; justify-content: center;
      font-size: 0.7rem;
      font-weight: 700;
    }
    .msg.user .avatar { background: #1f6feb; color: #fff; }
    .msg.assistant .avatar { background: var(--nvidia); color: #000; }

    .bubble {
      padding: 0.8rem 1.1rem;
      border-radius: 12px;
      font-size: 0.88rem;
      line-height: 1.7;
      white-space: pre-wrap;
      word-break: break-word;
    }
    .msg.user .bubble {
      background: #1a3a6e;
      border: 1px solid #1f6feb44;
      border-bottom-right-radius: 4px;
    }
    .msg.assistant .bubble {
      background: var(--surface2);
      border: 1px solid var(--border);
      border-bottom-left-radius: 4px;
    }

    /* thinking */
    .thinking-wrap { margin-bottom: 0.4rem; }
    details.think {
      background: #0d1117;
      border: 1px solid var(--border);
      border-left: 3px solid var(--purple);
      border-radius: 6px;
      padding: 0.5rem 0.75rem;
      font-size: 0.78rem;
    }
    details.think summary {
      color: var(--purple);
      font-weight: 600;
      cursor: pointer;
      user-select: none;
      list-style: none;
      display: flex; align-items: center; gap: 0.4rem;
    }
    details.think summary::before { content: '▶'; font-size: 0.65rem; transition: transform 0.15s; }
    details.think[open] summary::before { transform: rotate(90deg); }
    .think-body {
      margin-top: 0.5rem;
      color: var(--text-dim);
      white-space: pre-wrap;
      line-height: 1.5;
    }

    /* loader */
    .loader-dot {
      display: inline-block;
      width: 6px; height: 6px;
      border-radius: 50%;
      background: var(--nvidia);
      animation: blink 1.2s infinite;
      margin: 0 2px;
    }
    .loader-dot:nth-child(2) { animation-delay: 0.2s; }
    .loader-dot:nth-child(3) { animation-delay: 0.4s; }
    @keyframes blink { 0%,80%,100% { opacity:0.15 } 40% { opacity:1 } }

    /* ── INPUT BAR ── */
    .input-bar {
      border-top: 1px solid var(--border);
      background: var(--surface);
      padding: 0.85rem 1.25rem;
      display: flex;
      gap: 0.75rem;
      align-items: flex-end;
    }
    #chatInput {
      flex: 1;
      background: var(--surface2);
      border: 1px solid var(--border);
      border-radius: 10px;
      color: var(--text);
      font-size: 0.88rem;
      padding: 0.65rem 1rem;
      resize: none;
      outline: none;
      font-family: inherit;
      line-height: 1.5;
      max-height: 120px;
      overflow-y: auto;
      transition: border-color 0.15s;
    }
    #chatInput:focus { border-color: var(--blue); }
    #chatInput::placeholder { color: var(--text-dim); }
    #sendBtn {
      background: var(--nvidia);
      color: #000;
      border: none;
      border-radius: 10px;
      padding: 0.65rem 1.25rem;
      font-size: 0.88rem;
      font-weight: 700;
      cursor: pointer;
      transition: opacity 0.15s;
      height: 38px;
      white-space: nowrap;
    }
    #sendBtn:hover { opacity: 0.85; }
    #sendBtn:disabled { opacity: 0.35; cursor: not-allowed; }

    /* scrollbar globally */
    .sidebar::-webkit-scrollbar { width: 4px; }
    .sidebar::-webkit-scrollbar-thumb { background: var(--border2); border-radius: 10px; }

    @media (max-width: 640px) {
      .sidebar { display: none; }
      .msg { max-width: 95%; }
    }
  </style>
</head>
<body>

  <!-- TOP BAR -->
  <div class="topbar">
    <div class="logo">
      🔍 AI OSINT Investigator
      <span class="badge">NVIDIA NIM</span>
    </div>
    <div class="model-pill">
      <span class="dot"></span>
      nemotron-3-ultra-550b
    </div>
    <button class="clear-btn" onclick="clearChat()">Clear</button>
  </div>

  <div class="main">

    <!-- SIDEBAR -->
    <div class="sidebar">
      <div class="sidebar-section">
        <div class="sidebar-label">Investigation</div>
        <button class="quick-btn" onclick="ask('How do I investigate a person by name and location?')">
          <span class="icon">👤</span> Person Search
        </button>
        <button class="quick-btn" onclick="ask('Analyze this username across all platforms: ')">
          <span class="icon">🕵️</span> Username OSINT
        </button>
        <button class="quick-btn" onclick="ask('Perform OSINT on this domain or IP address: ')">
          <span class="icon">🌐</span> Domain / IP
        </button>
        <button class="quick-btn" onclick="ask('Help me analyze a social media profile for OSINT. Profile URL: ')">
          <span class="icon">📱</span> Social Media
        </button>
        <button class="quick-btn" onclick="ask('Help me geolocate this image using OSINT techniques. Describe what you see: ')">
          <span class="icon">📍</span> Image Geolocation
        </button>
        <button class="quick-btn" onclick="ask('Find OSINT information about this email address: ')">
          <span class="icon">📧</span> Email Lookup
        </button>
        <button class="quick-btn" onclick="ask('Research this phone number using OSINT: ')">
          <span class="icon">📞</span> Phone Number
        </button>
      </div>
      <hr class="sidebar-divider">
      <div class="sidebar-section">
        <div class="sidebar-label">Tools &amp; Techniques</div>
        <button class="quick-btn" onclick="ask('Generate advanced Google Dork queries to find information about: ')">
          <span class="icon">🔎</span> Google Dorks
        </button>
        <button class="quick-btn" onclick="ask('What are the best OSINT tools for dark web monitoring and breach data?')">
          <span class="icon">🌑</span> Dark Web
        </button>
        <button class="quick-btn" onclick="ask('How to perform corporate/company OSINT on: ')">
          <span class="icon">🏢</span> Company Research
        </button>
        <button class="quick-btn" onclick="ask('Create a professional OSINT investigation report template for a person investigation.')">
          <span class="icon">📄</span> Report Template
        </button>
      </div>
      <hr class="sidebar-divider">
      <div class="sidebar-section">
        <div class="sidebar-label">Resources</div>
        <button class="quick-btn" onclick="ask('What are the top AI-powered OSINT tools available today?')">
          <span class="icon">🛠️</span> AI OSINT Tools
        </button>
        <button class="quick-btn" onclick="ask('Explain the OSINT framework and investigation methodology step by step.')">
          <span class="icon">📚</span> OSINT Framework
        </button>
      </div>
      <div class="sidebar-bottom">
        ⚠️ For educational &amp; lawful use only. Always follow local laws and ethical guidelines.
      </div>
    </div>

    <!-- CHAT -->
    <div class="chat-area">
      <div class="messages" id="messages">
        <div class="welcome" id="welcome">
          <div class="shield">🛡️</div>
          <h2>AI OSINT Investigator</h2>
          <p>Powered by NVIDIA NIM · Nemotron Ultra 550B with extended reasoning. Ask anything about open-source intelligence, or pick a quick action from the sidebar.</p>
          <div class="welcome-chips">
            <span class="chip" onclick="ask('How do I find someone\\'s social media accounts from just a name?')">Find social accounts</span>
            <span class="chip" onclick="ask('Generate Google Dorks for finding leaked documents about a company')">Google Dorks</span>
            <span class="chip" onclick="ask('How do I geolocate a photo using OSINT techniques?')">Geolocate a photo</span>
            <span class="chip" onclick="ask('What OSINT tools can I use to investigate an IP address?')">IP investigation</span>
            <span class="chip" onclick="ask('Create a full OSINT checklist for investigating a suspicious domain')">Domain checklist</span>
          </div>
        </div>
      </div>

      <div class="input-bar">
        <textarea id="chatInput" rows="1"
          placeholder="Describe your investigation target or ask an OSINT question…"
          oninput="autoResize(this)"></textarea>
        <button id="sendBtn" onclick="sendMessage()">▶ Send</button>
      </div>
    </div>

  </div>

  <script>
    const messagesEl = document.getElementById('messages');
    const inputEl = document.getElementById('chatInput');
    const sendBtn = document.getElementById('sendBtn');
    const history = [];

    function autoResize(el) {
      el.style.height = 'auto';
      el.style.height = Math.min(el.scrollHeight, 120) + 'px';
    }

    inputEl.addEventListener('keydown', e => {
      if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); sendMessage(); }
    });

    function scrollBottom() { messagesEl.scrollTop = messagesEl.scrollHeight; }

    function removeWelcome() {
      const w = document.getElementById('welcome');
      if (w) w.remove();
    }

    function ask(text) {
      inputEl.value = text;
      autoResize(inputEl);
      inputEl.focus();
      // place cursor at end
      inputEl.selectionStart = inputEl.selectionEnd = text.length;
    }

    function clearChat() {
      history.length = 0;
      messagesEl.innerHTML = `<div class="welcome" id="welcome">
        <div class="shield">🛡️</div>
        <h2>AI OSINT Investigator</h2>
        <p>Powered by NVIDIA NIM · Nemotron Ultra 550B with extended reasoning. Ask anything about open-source intelligence, or pick a quick action from the sidebar.</p>
        <div class="welcome-chips">
          <span class="chip" onclick="ask('How do I find someone\\'s social media accounts from just a name?')">Find social accounts</span>
          <span class="chip" onclick="ask('Generate Google Dorks for finding leaked documents about a company')">Google Dorks</span>
          <span class="chip" onclick="ask('How do I geolocate a photo using OSINT techniques?')">Geolocate a photo</span>
          <span class="chip" onclick="ask('What OSINT tools can I use to investigate an IP address?')">IP investigation</span>
          <span class="chip" onclick="ask('Create a full OSINT checklist for investigating a suspicious domain')">Domain checklist</span>
        </div>
      </div>`;
    }

    async function sendMessage() {
      const text = inputEl.value.trim();
      if (!text || sendBtn.disabled) return;

      inputEl.value = '';
      autoResize(inputEl);
      sendBtn.disabled = true;
      removeWelcome();

      // User bubble
      appendMsg('user', text);
      history.push({ role: 'user', content: text });

      // Assistant placeholder
      const { wrap, addThinking, addContent, setError } = createAssistantBubble();
      scrollBottom();

      try {
        const res = await fetch('/chat', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ messages: history })
        });

        if (!res.ok) {
          setError(`Server error: ${res.status} ${res.statusText}`);
          sendBtn.disabled = false;
          return;
        }

        const reader = res.body.getReader();
        const decoder = new TextDecoder();
        let thinkingText = '';
        let answerText = '';

        while (true) {
          const { done, value } = await reader.read();
          if (done) break;
          const raw = decoder.decode(value);
          for (const line of raw.split('\\n')) {
            if (!line.startsWith('data: ')) continue;
            const payload = line.slice(6);
            if (payload === '[DONE]') break;
            try {
              const obj = JSON.parse(payload);
              if (obj.type === 'thinking') {
                thinkingText += obj.content;
                addThinking(thinkingText);
              } else if (obj.type === 'content') {
                answerText += obj.content;
                addContent(answerText);
              } else if (obj.type === 'error') {
                setError(obj.content);
              }
            } catch(e) {}
          }
          scrollBottom();
        }

        if (answerText) history.push({ role: 'assistant', content: answerText });

      } catch (err) {
        setError(err.message);
      }

      sendBtn.disabled = false;
      inputEl.focus();
      scrollBottom();
    }

    function appendMsg(role, text) {
      const wrap = document.createElement('div');
      wrap.className = `msg ${role}`;

      const meta = document.createElement('div');
      meta.className = 'msg-meta';
      const avatar = document.createElement('div');
      avatar.className = 'avatar';
      avatar.textContent = role === 'user' ? 'U' : 'N';
      const name = document.createTextNode(role === 'user' ? 'You' : 'Nemotron');
      meta.appendChild(avatar);
      meta.appendChild(name);
      wrap.appendChild(meta);

      const bubble = document.createElement('div');
      bubble.className = 'bubble';
      bubble.textContent = text;
      wrap.appendChild(bubble);

      messagesEl.appendChild(wrap);
      scrollBottom();
    }

    function createAssistantBubble() {
      const wrap = document.createElement('div');
      wrap.className = 'msg assistant';

      const meta = document.createElement('div');
      meta.className = 'msg-meta';
      const avatar = document.createElement('div');
      avatar.className = 'avatar';
      avatar.textContent = 'N';
      meta.appendChild(avatar);
      meta.appendChild(document.createTextNode('Nemotron'));
      wrap.appendChild(meta);

      // loader
      const loader = document.createElement('div');
      loader.innerHTML = '<span class="loader-dot"></span><span class="loader-dot"></span><span class="loader-dot"></span>';
      loader.style.padding = '0.5rem 0';
      wrap.appendChild(loader);

      messagesEl.appendChild(wrap);

      let thinkDetails = null;
      let thinkBodyEl = null;
      let bubble = null;

      function addThinking(text) {
        loader.remove();
        if (!thinkDetails) {
          thinkDetails = document.createElement('details');
          thinkDetails.className = 'think';
          const summary = document.createElement('summary');
          summary.textContent = '🧠 Reasoning';
          thinkDetails.appendChild(summary);
          thinkBodyEl = document.createElement('div');
          thinkBodyEl.className = 'think-body';
          thinkDetails.appendChild(thinkBodyEl);
          wrap.appendChild(thinkDetails);
        }
        thinkBodyEl.textContent = text;
      }

      function addContent(text) {
        loader.remove();
        if (!bubble) {
          bubble = document.createElement('div');
          bubble.className = 'bubble';
          wrap.appendChild(bubble);
        }
        bubble.textContent = text;
      }

      function setError(msg) {
        loader.remove();
        if (!bubble) {
          bubble = document.createElement('div');
          bubble.className = 'bubble';
          bubble.style.color = '#ff5e5e';
          wrap.appendChild(bubble);
        }
        bubble.textContent = '⚠️ ' + msg;
      }

      return { wrap, addThinking, addContent, setError };
    }
  </script>
</body>
</html>"""


@app.route('/')
def index():
    return HTML_PAGE


@app.route('/chat', methods=['POST'])
def chat():
    data = request.get_json()
    messages = data.get('messages', [])

    api_key = os.environ.get('NVIDIA_NIM_API_KEY', '')
    if not api_key:
        def err():
            yield 'data: ' + json.dumps({"type": "error", "content": "NVIDIA_NIM_API_KEY not set."}) + '\n\n'
            yield 'data: [DONE]\n\n'
        return Response(stream_with_context(err()), mimetype='text/event-stream')

    from openai import OpenAI
    client = OpenAI(
        base_url="https://integrate.api.nvidia.com/v1",
        api_key=api_key
    )

    full_messages = [{"role": "system", "content": SYSTEM_PROMPT}] + messages

    def generate():
        try:
            completion = client.chat.completions.create(
                model="nvidia/nemotron-3-ultra-550b-a55b",
                messages=full_messages,
                temperature=1,
                top_p=0.95,
                max_tokens=16384,
                extra_body={
                    "chat_template_kwargs": {"enable_thinking": True},
                    "reasoning_budget": 16384
                },
                stream=True
            )
            for chunk in completion:
                if not chunk.choices:
                    continue
                reasoning = getattr(chunk.choices[0].delta, "reasoning_content", None)
                if reasoning:
                    yield 'data: ' + json.dumps({"type": "thinking", "content": reasoning}) + '\n\n'
                content = chunk.choices[0].delta.content
                if content:
                    yield 'data: ' + json.dumps({"type": "content", "content": content}) + '\n\n'
            yield 'data: [DONE]\n\n'
        except Exception as e:
            yield 'data: ' + json.dumps({"type": "error", "content": str(e)}) + '\n\n'
            yield 'data: [DONE]\n\n'

    return Response(stream_with_context(generate()), mimetype='text/event-stream')


if __name__ == '__main__':
    print(f"Serving on http://{HOST}:{PORT}")
    app.run(host=HOST, port=PORT, debug=False)
