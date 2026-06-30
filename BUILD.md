# Ravens Nexus Desktop

## Stack
- Rust + Tauri v2 (backend / IPC)
- Svelte + Vite + Tailwind (frontend)
- Все OSINT-модули встроены в osint.rs (без внешнего сервера)

## Встроенные модули

| Модуль   | Источник                          |
|----------|-----------------------------------|
| social   | Reddit API, GitHub API, Telegram  |
| hibp     | HaveIBeenPwned v2                 |
| ip       | ip-api.com                        |
| whois    | RDAP.org + Cloudflare DoH         |
| dorks    | 18 Google Dorks                   |
| paste    | Doxbin + pastebin dorks           |
| darkweb  | Ahmia.fi                          |
| phone    | Veriphone API                     |
| intelx   | IntelX phonebook API              |
| ai       | NVIDIA NIM Nemotron-Ultra         |

## Отличия от ветки main

- server.py удалён полностью
- Нет SQLite/пользователей/OAuth
- OSINT логика в Rust (osint.rs)
- Ключ NVIDIA хранится AES-256-GCM привязан к HWID
- Нативное приложение Win/macOS/Linux
