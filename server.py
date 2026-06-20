import http.server
import socketserver
import re

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

body = md_to_html(readme)

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
      max-width: 860px;
      margin: 0 auto;
      background: #161b22;
      border: 1px solid #30363d;
      border-radius: 10px;
      padding: 2.5rem 3rem;
    }}
    h1 {{
      font-size: 2rem;
      color: #58a6ff;
      border-bottom: 2px solid #30363d;
      padding-bottom: 0.5rem;
      margin-bottom: 1.5rem;
    }}
    h2 {{
      font-size: 1.4rem;
      color: #79c0ff;
      margin: 2rem 0 0.75rem;
      border-bottom: 1px solid #21262d;
      padding-bottom: 0.3rem;
    }}
    h3 {{
      font-size: 1.1rem;
      color: #d2a8ff;
      margin: 1.5rem 0 0.5rem;
    }}
    p {{
      margin: 0.3rem 0;
    }}
    a {{
      color: #58a6ff;
      text-decoration: none;
    }}
    a:hover {{
      text-decoration: underline;
      color: #79c0ff;
    }}
    hr {{
      border: none;
      border-top: 1px solid #30363d;
      margin: 1.5rem 0;
    }}
    img {{
      max-width: 100px;
      vertical-align: middle;
      border-radius: 6px;
    }}
    @media (max-width: 600px) {{
      .container {{ padding: 1.2rem; }}
      h1 {{ font-size: 1.5rem; }}
    }}
  </style>
</head>
<body>
  <div class="container">
{body}
  </div>
</body>
</html>"""

class Handler(http.server.SimpleHTTPRequestHandler):
    def do_GET(self):
        self.send_response(200)
        self.send_header('Content-Type', 'text/html; charset=utf-8')
        self.end_headers()
        self.wfile.write(PAGE.encode('utf-8'))

    def log_message(self, format, *args):
        print(f"[{self.address_string()}] {format % args}")

print(f"Serving on http://{HOST}:{PORT}")
with socketserver.TCPServer((HOST, PORT), Handler) as httpd:
    httpd.serve_forever()
