# Ravens Nexus — Build Guide

## Server (Replit)

1. Загрузи папку `ravens-server/` на Replit
2. Выбери шаблон **Rust**
3. Скопируй `.env.example` → `.env` и заполни секреты
4. В `.replit` укажи:
   ```toml
   run = "cargo run --release"
   ```
5. Нажми **Run** — сервер стартует на порту 3000
6. Replit даст тебе URL вида `https://ravens-nexus.your-name.repl.co`

## Admin Panel

Открой: `https://YOUR-REPLIT-URL/admin-ui/admin.html`

Введи `RAVENS_ADMIN_SECRET` из `.env` — готово.

## Client (Tauri Desktop)

```bash
cd ravens-client
npm install          # или pnpm install
npm run tauri dev    # или: npm run tauri build
```

В настройках приложения (Настройки → Сервер) введи URL сервера и токен, нажми «Активировать».

## API Endpoints

### Public
- `POST /api/license/activate` — активация токена, привязка HWID → выдаёт session-токен + salt
- `POST /api/osint/run` — **выполнить OSINT-модуль на сервере** (подписанный запрос; проверка токена, HWID, тарифа, лимита)
- `POST /api/license/consume` — списание запроса (legacy)
- `GET  /api/license/status/:token` — статус токена
- `POST /api/license/rebind` — смена HWID (требует admin_secret)
- `GET  /api/news` — публичные новости

> **Важно:** вся OSINT-логика выполняется на сервере. Клиент только активирует токен
> и отправляет подписанные запросы; ключи провайдеров (NVIDIA/IntelX/HIBP) хранятся в
> переменных окружения сервера и не попадают на клиент.

### Admin (x-admin-secret header)
- `POST   /admin/tokens` — создать токен
- `GET    /admin/tokens` — список токенов
- `DELETE /admin/tokens/:token` — отозвать
- `PUT    /admin/tokens/:token/tier` — сменить тариф
- `POST   /admin/tokens/:token/reset-requests` — сбросить счётчик
- `POST   /admin/tokens/:token/rebind-hwid` — переназначить HWID
- `GET    /admin/sessions` — активные сессии
- `POST   /admin/sessions/:id/ban` — забанить сессию / HWID
- `POST   /admin/hwid/unban` — разбанить HWID
- `GET    /admin/news` — все новости
- `POST   /admin/news` — создать новость
- `PATCH  /admin/news/:id` — обновить
- `DELETE /admin/news/:id` — удалить
- `GET    /admin/stats` — статистика

## Тарифы

| Тариф | Запросы | Модули |
|-------|---------|--------|
| FREE  | 2       | social, ip_geo, whois |
| PRO   | 1000    | + hibp, dorks, paste, darkweb, phone |
| ELITE | ∞       | + intelx, ai |
| ADMIN | ∞       | все (скрыт от клиентов) |

## HWID Binding

- При первой активации токен привязывается к железу (SHA-256 от HWID)
- Попытка активировать с другого ПК → ошибка
- Смена HWID через `/api/license/rebind` (нужен admin_secret) или через `/admin/tokens/:token/rebind-hwid`
- Бан по HWID через админ-панель → приложение не запустится даже с другим токеном

## Sessions

- Каждая активация создаёт сессию (uuid)
- Сессия передаётся при каждом запросе (`session_id`)
- Бан сессии = мягкий бан (только эта сессия)
- Бан HWID = жёсткий бан (всё устройство навсегда)
