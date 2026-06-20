import os
import re
import json
from flask import Flask, request, Response, stream_with_context
from openai import OpenAI

app = Flask(__name__)

PORT = 5000
HOST = "0.0.0.0"

def md_to_html(md):
    html = md
    html = re.sub(r'^# (.+)$', r'<h1>\1</h1>', html, flags=re.MULTILINE)
    html = re.sub(r'^## (.+)$', r'<h2>\1</h2>', html, flags=re.MULTILINE)
    html = re.sub(r'^### (.+)$', r'<h3>\1</h3>', html, flags=re.MULTILINE)
    html = re.sub(r'\[([^\]]+)\]\(([^)]+)\)', r'<a href="\2" target="_blank" rel="noopener">\1</a>', html)
    html = re.sub(r'!\[([^\]]*)\]\(([^)]+)\)', r'<img alt="\1" src="\2" style="max-width:100px;vertical-align:middle;">', html)
    html = re.sub(r'^-{3,}$', r'<hr>', html, flags=re.MULTILINE)
    lines = html.split('\n')
    result = []
    for line in lines:
        stripped = line.strip()
        if stripped.startswith('<h') or stripped.startswith('<hr'):
            result.append(stripped)
        elif stripped == '':
            result.append('')
        else:
            result.append(f'<p>{stripped}</p>')
    return '\n'.join(result)

with open('README.md', 'r', encoding='utf-8') as f:
    readme = f.read()

readme_body = md_to_html(readme)

PAGE = f"""<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Awesome AI OSINT</title>
  <style>
    * {{ box-sizing: border-box; margin: 0; padding: 0; }}
    body {{
      font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, sans-serif;
      background: #0d1117;
      color: #c9d1d9;
      line-height: 1.7;
      padding: 2rem 1rem;
    }}
    .container {{
      max-width: 900px;
      margin: 0 auto;
    }}

    /* README section */
    .readme-card {{
      background: #161b22;
      border: 1px solid #30363d;
      border-radius: 10px;
      padding: 2.5rem 3rem;
      margin-bottom: 2.5rem;
    }}
    h1 {{ font-size: 2rem; color: #58a6ff; border-bottom: 2px solid #30363d; padding-bottom: 0.5rem; margin-bottom: 1.5rem; }}
    h2 {{ font-size: 1.4rem; color: #79c0ff; margin: 2rem 0 0.75rem; border-bottom: 1px solid #21262d; padding-bottom: 0.3rem; }}
    h3 {{ font-size: 1.1rem; color: #d2a8ff; margin: 1.5rem 0 0.5rem; }}
    p {{ margin: 0.3rem 0; }}
    a {{ color: #58a6ff; text-decoration: none; }}
    a:hover {{ text-decoration: underline; color: #79c0ff; }}
    hr {{ border: none; border-top: 1px solid #30363d; margin: 1.5rem 0; }}
    img {{ max-width: 100px; vertical-align: middle; border-radius: 6px; }}

    /* Chat section */
    .chat-card {{
      background: #161b22;
      border: 1px solid #30363d;
      border-radius: 10px;
      overflow: hidden;
      margin-bottom: 2.5rem;
    }}
    .chat-header {{
      background: #1c2128;
      border-bottom: 1px solid #30363d;
      padding: 1rem 1.5rem;
      display: flex;
      align-items: center;
      gap: 0.75rem;
    }}
    .chat-header .dot {{
      width: 10px; height: 10px;
      border-radius: 50%;
      background: #3fb950;
      box-shadow: 0 0 6px #3fb950;
    }}
    .chat-header h2 {{
      font-size: 1.1rem;
      color: #e6edf3;
      border: none;
      margin: 0;
      padding: 0;
    }}
    .chat-header .model-tag {{
      margin-left: auto;
      font-size: 0.75rem;
      color: #8b949e;
      background: #21262d;
      border: 1px solid #30363d;
      border-radius: 20px;
      padding: 0.2rem 0.75rem;
    }}
    .messages {{
      height: 420px;
      overflow-y: auto;
      padding: 1.5rem;
      display: flex;
      flex-direction: column;
      gap: 1rem;
      scroll-behavior: smooth;
    }}
    .msg {{
      display: flex;
      flex-direction: column;
      max-width: 80%;
    }}
    .msg.user {{ align-self: flex-end; align-items: flex-end; }}
    .msg.assistant {{ align-self: flex-start; align-items: flex-start; }}
    .msg-label {{
      font-size: 0.7rem;
      color: #8b949e;
      margin-bottom: 0.25rem;
      text-transform: uppercase;
      letter-spacing: 0.05em;
    }}
    .bubble {{
      padding: 0.75rem 1rem;
      border-radius: 12px;
      font-size: 0.92rem;
      line-height: 1.6;
      white-space: pre-wrap;
      word-break: break-word;
    }}
    .msg.user .bubble {{
      background: #1f6feb;
      color: #fff;
      border-bottom-right-radius: 4px;
    }}
    .msg.assistant .bubble {{
      background: #21262d;
      color: #c9d1d9;
      border: 1px solid #30363d;
      border-bottom-left-radius: 4px;
    }}
    .thinking-block {{
      background: #0d1117;
      border: 1px solid #30363d;
      border-left: 3px solid #d2a8ff;
      border-radius: 6px;
      padding: 0.6rem 0.9rem;
      font-size: 0.8rem;
      color: #8b949e;
      font-style: italic;
      margin-bottom: 0.5rem;
      cursor: pointer;
    }}
    .thinking-block summary {{
      color: #d2a8ff;
      font-style: normal;
      font-weight: 600;
      list-style: none;
      display: flex;
      align-items: center;
      gap: 0.4rem;
    }}
    .thinking-block summary::before {{ content: '▶'; font-size: 0.7rem; }}
    .thinking-block[open] summary::before {{ content: '▼'; }}
    .thinking-text {{ margin-top: 0.5rem; line-height: 1.5; white-space: pre-wrap; }}
    .chat-input-row {{
      display: flex;
      gap: 0.5rem;
      padding: 1rem 1.5rem;
      border-top: 1px solid #30363d;
      background: #1c2128;
    }}
    #chatInput {{
      flex: 1;
      background: #0d1117;
      border: 1px solid #30363d;
      border-radius: 8px;
      color: #c9d1d9;
      font-size: 0.92rem;
      padding: 0.6rem 1rem;
      resize: none;
      outline: none;
      font-family: inherit;
      line-height: 1.5;
    }}
    #chatInput:focus {{ border-color: #58a6ff; }}
    #sendBtn {{
      background: #238636;
      color: #fff;
      border: none;
      border-radius: 8px;
      padding: 0 1.2rem;
      font-size: 0.92rem;
      font-weight: 600;
      cursor: pointer;
      transition: background 0.15s;
      min-width: 72px;
    }}
    #sendBtn:hover {{ background: #2ea043; }}
    #sendBtn:disabled {{ background: #21262d; color: #484f58; cursor: not-allowed; }}

    .empty-state {{
      text-align: center;
      color: #484f58;
      margin: auto;
      padding: 2rem;
    }}
    .empty-state .icon {{ font-size: 2.5rem; margin-bottom: 0.5rem; }}
    .spinner {{
      display: inline-block;
      width: 14px; height: 14px;
      border: 2px solid #30363d;
      border-top-color: #58a6ff;
      border-radius: 50%;
      animation: spin 0.7s linear infinite;
      vertical-align: middle;
      margin-right: 6px;
    }}
    @keyframes spin {{ to {{ transform: rotate(360deg); }} }}

    @media (max-width: 600px) {{
      .readme-card, .chat-card {{ border-radius: 0; }}
      .readme-card {{ padding: 1.2rem; }}
      h1 {{ font-size: 1.5rem; }}
      .msg {{ max-width: 95%; }}
    }}
  </style>
</head>
<body>
  <div class="container">

    <!-- NVIDIA NIM Chat -->
    <div class="chat-card">
      <div class="chat-header">
        <div class="dot"></div>
        <h2>NVIDIA NIM AI Chat</h2>
        <span class="model-tag">nemotron-ultra-253b</span>
      </div>
      <div class="messages" id="messages">
        <div class="empty-state" id="emptyState">
          <div class="icon">🤖</div>
          <div>Ask anything about AI & OSINT</div>
        </div>
      </div>
      <div class="chat-input-row">
        <textarea id="chatInput" rows="2" placeholder="Ask a question… (Enter to send, Shift+Enter for newline)"></textarea>
        <button id="sendBtn" onclick="sendMessage()">Send</button>
      </div>
    </div>

    <!-- README content -->
    <div class="readme-card">
{readme_body}
    </div>

  </div>

  <script>
    const messagesEl = document.getElementById('messages');
    const inputEl = document.getElementById('chatInput');
    const sendBtn = document.getElementById('sendBtn');
    const emptyState = document.getElementById('emptyState');
    const history = [];

    inputEl.addEventListener('keydown', e => {{
      if (e.key === 'Enter' && !e.shiftKey) {{
        e.preventDefault();
        sendMessage();
      }}
    }});

    function scrollBottom() {{
      messagesEl.scrollTop = messagesEl.scrollHeight;
    }}

    function addMessage(role, text, thinkingText) {{
      if (emptyState) emptyState.remove();

      const wrap = document.createElement('div');
      wrap.className = `msg ${{role}}`;

      const label = document.createElement('div');
      label.className = 'msg-label';
      label.textContent = role === 'user' ? 'You' : 'Nemotron';
      wrap.appendChild(label);

      if (thinkingText) {{
        const details = document.createElement('details');
        details.className = 'thinking-block';
        const summary = document.createElement('summary');
        summary.textContent = 'Thinking';
        const thinkDiv = document.createElement('div');
        thinkDiv.className = 'thinking-text';
        thinkDiv.textContent = thinkingText;
        details.appendChild(summary);
        details.appendChild(thinkDiv);
        wrap.appendChild(details);
      }}

      const bubble = document.createElement('div');
      bubble.className = 'bubble';
      bubble.textContent = text;
      wrap.appendChild(bubble);

      messagesEl.appendChild(wrap);
      scrollBottom();
      return bubble;
    }}

    async function sendMessage() {{
      const text = inputEl.value.trim();
      if (!text) return;

      inputEl.value = '';
      sendBtn.disabled = true;

      addMessage('user', text);
      history.push({{ role: 'user', content: text }});

      // assistant placeholder
      if (emptyState) emptyState.remove();
      const wrap = document.createElement('div');
      wrap.className = 'msg assistant';
      const label = document.createElement('div');
      label.className = 'msg-label';
      label.textContent = 'Nemotron';
      wrap.appendChild(label);

      const spinnerSpan = document.createElement('span');
      spinnerSpan.innerHTML = '<span class="spinner"></span>Thinking…';
      spinnerSpan.style.color = '#8b949e';
      spinnerSpan.style.fontSize = '0.85rem';
      wrap.appendChild(spinnerSpan);

      messagesEl.appendChild(wrap);
      scrollBottom();

      try {{
        const res = await fetch('/chat', {{
          method: 'POST',
          headers: {{ 'Content-Type': 'application/json' }},
          body: JSON.stringify({{ messages: history }})
        }});

        const reader = res.body.getReader();
        const decoder = new TextDecoder();

        let thinkingText = '';
        let answerText = '';
        let thinkingDetails = null;
        let bubble = null;

        spinnerSpan.remove();

        while (true) {{
          const {{ done, value }} = await reader.read();
          if (done) break;
          const chunk = decoder.decode(value);
          const lines = chunk.split('\\n');

          for (const line of lines) {{
            if (!line.startsWith('data: ')) continue;
            const data = line.slice(6);
            if (data === '[DONE]') break;
            try {{
              const obj = JSON.parse(data);
              if (obj.type === 'thinking') {{
                thinkingText += obj.content;
                if (!thinkingDetails) {{
                  thinkingDetails = document.createElement('details');
                  thinkingDetails.className = 'thinking-block';
                  const summary = document.createElement('summary');
                  summary.textContent = 'Thinking';
                  thinkingDetails.appendChild(summary);
                  const thinkDiv = document.createElement('div');
                  thinkDiv.className = 'thinking-text';
                  thinkingDetails.appendChild(thinkDiv);
                  wrap.appendChild(thinkingDetails);
                }}
                thinkingDetails.querySelector('.thinking-text').textContent = thinkingText;
              }} else if (obj.type === 'content') {{
                answerText += obj.content;
                if (!bubble) {{
                  bubble = document.createElement('div');
                  bubble.className = 'bubble';
                  wrap.appendChild(bubble);
                }}
                bubble.textContent = answerText;
              }}
            }} catch(e) {{}}
          }}
          scrollBottom();
        }}

        history.push({{ role: 'assistant', content: answerText }});

      }} catch (err) {{
        spinnerSpan.remove();
        const errBubble = document.createElement('div');
        errBubble.className = 'bubble';
        errBubble.style.color = '#f85149';
        errBubble.textContent = 'Error: ' + err.message;
        wrap.appendChild(errBubble);
      }}

      sendBtn.disabled = false;
      inputEl.focus();
      scrollBottom();
    }}
  </script>
</body>
</html>"""

@app.route('/')
def index():
    return PAGE

@app.route('/chat', methods=['POST'])
def chat():
    data = request.get_json()
    messages = data.get('messages', [])

    api_key = os.environ.get('NVIDIA_NIM_API_KEY', '')
    client = OpenAI(
        base_url="https://integrate.api.nvidia.com/v1",
        api_key=api_key
    )

    def generate():
        try:
            completion = client.chat.completions.create(
                model="nvidia/nemotron-ultra-253b-v1",
                messages=messages,
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
                    payload = json.dumps({"type": "thinking", "content": reasoning})
                    yield f"data: {payload}\n\n"
                content = chunk.choices[0].delta.content
                if content:
                    payload = json.dumps({"type": "content", "content": content})
                    yield f"data: {payload}\n\n"
            yield "data: [DONE]\n\n"
        except Exception as e:
            payload = json.dumps({"type": "content", "content": f"[Error: {str(e)}]"})
            yield f"data: {payload}\n\n"
            yield "data: [DONE]\n\n"

    return Response(stream_with_context(generate()), mimetype='text/event-stream')

if __name__ == '__main__':
    print(f"Serving on http://{HOST}:{PORT}")
    app.run(host=HOST, port=PORT, debug=False)
