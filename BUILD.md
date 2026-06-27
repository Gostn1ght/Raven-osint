# Ravens Nexus — Руководство по сборке

## Предварительные требования

| Инструмент      | Версия    | Установка |
|----------------|-----------|-----------|
| Rust + Cargo   | ≥ 1.78    | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| Node.js        | ≥ 20 LTS  | https://nodejs.org |
| pnpm           | ≥ 9       | `npm i -g pnpm` |
| Tauri CLI      | 2.x       | `cargo install tauri-cli@^2` |
| (Windows) WebView2 | актуальная | Предустановлен в Win11, иначе: https://developer.microsoft.com/edge/webview2/ |

---

## Структура проекта

```
ravens-nexus/
├── Cargo.toml                    ← Workspace-манифест (оба крейта)
├── ravens-server/                ← Серверный бинарник
│   ├── Cargo.toml
│   ├── migrations/               ← SQLx-миграции
│   └── src/
│       ├── main.rs               ← Точка входа (axum)
│       ├── config.rs             ← Настройки из .env
│       ├── errors.rs             ← Единая обработка ошибок
│       ├── auth/mod.rs           ← JWT, Argon2, HWID, middleware
│       ├── db/mod.rs             ← SQLite pool + репозитории
│       ├── api/                  ← HTTP-роуты
│       │   ├── auth.rs           ← /api/auth/*
│       │   ├── osint.rs          ← /api/osint/*
│       │   ├── user.rs           ← /api/user/*
│       │   ├── payments.rs       ← /api/pay/*
│       │   └── admin.rs          ← /api/admin/*
│       ├── osint/                ← OSINT-модули
│       │   ├── sherlock.rs       ← Поиск по никнейму (100+ сайтов)
│       │   ├── holehe.rs         ← Email-разведка
│       │   ├── whois.rs          ← WHOIS / DNS / IP-info
│       │   ├── dorks.rs          ← Google Dorks генератор
│       │   ├── geodata.rs        ← GeoIP, OpenSky, USGS
│       │   └── image_search.rs   ← Lenso + Picarta
│       └── payments/
│           ├── yoomoney.rs
│           └── cryptomus.rs
└── ravens-client/                ← Tauri-приложение
    ├── package.json
    ├── vite.config.ts
    ├── tailwind.config.js
    ├── index.html
    ├── src/                      ← Svelte-фронтенд
    │   ├── App.svelte            ← Корневой компонент
    │   ├── routes/
    │   │   ├── Login.svelte
    │   │   └── Dashboard.svelte
    │   └── components/
    │       ├── Sidebar.svelte
    │       ├── Titlebar.svelte
    │       ├── History.svelte
    │       ├── Profile.svelte
    │       └── tools/
    │           ├── ToolUsername.svelte
    │           ├── ToolEmail.svelte
    │           ├── ToolWhois.svelte
    │           ├── ToolDorks.svelte
    │           ├── ToolGeo.svelte
    │           └── ToolImage.svelte
    └── src-tauri/                ← Rust-ядро Tauri
        ├── Cargo.toml
        ├── tauri.conf.json
        └── src/
            ├── main.rs           ← Инициализация Tauri
            ├── commands.rs       ← invoke()-обработчики
            ├── hwid.rs           ← Генерация HWID
            ├── server.rs         ← Запуск/мониторинг сервера
            └── config.rs         ← Шифрованный конфиг
```

---

## 1. Сборка сервера

```bash
cd ravens-nexus

# Копируем конфиг
cp .env.example .env
# Редактируем .env — заполняем JWT_SECRET, ADMIN_TOKEN и API-ключи

# Локальная сборка (debug)
cargo build -p ravens-server

# Release
cargo build --release -p ravens-server
# → target/release/ravens-nexus-server(.exe)

# Запуск сервера
./target/release/ravens-nexus-server
# Сервер слушает http://127.0.0.1:8080
```

---

## 2. Сборка клиента (Tauri)

```bash
cd ravens-nexus/ravens-client

# Установка зависимостей фронтенда
pnpm install

# Скопировать сервер рядом с тауровым бинарником (для dev-режима)
cp ../target/release/ravens-nexus-server src-tauri/

# Dev-режим (hot reload + живой сервер)
cargo tauri dev

# Production build
cargo tauri build
# → src-tauri/target/release/bundle/
#   ├── nsis/RavensNexus_0.1.0_x64-setup.exe   (Windows installer)
#   ├── msi/RavensNexus_0.1.0_x64_en-US.msi
#   ├── appimage/ravens-nexus_0.1.0_amd64.AppImage
#   └── macos/RavensNexus.app / RavensNexus.dmg
```

---

## 3. Кросс-компиляция

### Windows → Linux (ARM64)
```bash
rustup target add aarch64-unknown-linux-gnu
sudo apt-get install gcc-aarch64-linux-gnu  # на Ubuntu/Debian
CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc \
  cargo build --release --target aarch64-unknown-linux-gnu -p ravens-server
```

### macOS universal binary (Intel + Apple Silicon)
```bash
rustup target add x86_64-apple-darwin aarch64-apple-darwin
cargo build --release --target x86_64-apple-darwin  -p ravens-server
cargo build --release --target aarch64-apple-darwin -p ravens-server
lipo -create -output ravens-nexus-server \
  target/x86_64-apple-darwin/release/ravens-nexus-server \
  target/aarch64-apple-darwin/release/ravens-nexus-server
```

---

## 4. API Контракт (сводная таблица эндпоинтов)

| Метод | Путь | Auth | Описание |
|-------|------|------|----------|
| POST | `/api/auth/register` | — | Регистрация (username, email, password, hwid?) |
| POST | `/api/auth/login` | — | Вход (email, password, hwid?) → JWT |
| POST | `/api/auth/hwid` | JWT | Привязка HWID |
| GET  | `/api/user/me` | JWT | Профиль текущего пользователя |
| PUT  | `/api/user/me` | JWT | Обновление профиля (bio, avatar_url) |
| GET  | `/api/user/subscription` | JWT | Статус подписки |
| GET  | `/api/user/api-key` | JWT | Получить API-ключ |
| POST | `/api/osint/username` | JWT | Поиск по нику (Sherlock) |
| POST | `/api/osint/email` | JWT | Email разведка (Holehe + HIBP) |
| POST | `/api/osint/whois` | JWT | WHOIS домена |
| POST | `/api/osint/dns` | JWT | DNS-записи |
| POST | `/api/osint/ip` | JWT | IP-info |
| POST | `/api/osint/dorks` | JWT | Генерация Google Dorks |
| POST | `/api/osint/image` | JWT | Геолокация изображения + reverse search |
| POST | `/api/osint/geoip` | JWT | GeoIP |
| GET  | `/api/osint/aircraft` | JWT | Live aircraft (bbox: lat_min/max, lon_min/max) |
| GET  | `/api/osint/earthquakes` | JWT | USGS землетрясения (min_magnitude?) |
| GET  | `/api/osint/history` | JWT | История запросов пользователя |
| POST | `/api/pay/create` | JWT | Создать платёж (provider, plan) |
| POST | `/api/pay/yoomoney/callback` | — | Webhook ЮMoney |
| POST | `/api/pay/cryptomus/callback` | — | Webhook Cryptomus |
| GET  | `/api/admin/users` | AdminToken | Список пользователей |
| POST | `/api/admin/grant-subscription` | AdminToken | Выдать подписку вручную |
| GET  | `/api/admin/stats` | AdminToken | Статистика платформы |
| GET  | `/health` | — | Health check |

### Тарифы

| План | Цена | Лимит/мес |
|------|------|-----------|
| free | — | 20 запросов |
| basic | 299 ₽ | 500 запросов |
| pro | 799 ₽ | 2 000 запросов |
| elite | 1 999 ₽ | 10 000 запросов |

---

## 5. Переменные окружения

Скопируйте `.env.example` в `.env` и заполните:

| Переменная | Обязательна | Описание |
|-----------|-------------|----------|
| `JWT_SECRET` | ✅ | 64-символьный случайный hex (openssl rand -hex 64) |
| `ADMIN_TOKEN` | ✅ | Токен для доступа к /api/admin/* |
| `DATABASE_URL` | — | По умолчанию `sqlite://ravens.db` |
| `HIBP_API_KEY` | — | haveibeenpwned.com API ключ |
| `LENSO_API_KEY` | — | Lenso.ai API ключ (reverse image search) |
| `PICARTA_API_KEY` | — | Picarta (image geolocation) |
| `YOOMONEY_TOKEN` | — | Номер кошелька ЮMoney |
| `CRYPTOMUS_MERCHANT` | — | Merchant ID в Cryptomus |
| `CRYPTOMUS_API_KEY` | — | API ключ Cryptomus |

---

## 6. GitHub Actions CI/CD

Файл `.github/workflows/build.yml` автоматически:
1. Собирает `ravens-nexus-server` под 5 платформ (Win/Linux/macOS Intel+ARM/Linux ARM)
2. Собирает Tauri-клиент под Windows/Linux/macOS Intel+ARM (с вложенным сервером)
3. При пуше тега `v*` — создаёт GitHub Release с артефактами

```bash
# Создать релиз v0.1.0
git tag v0.1.0
git push origin v0.1.0
```

Для подписи обновлений сгенерируйте ключ:
```bash
cargo tauri signer generate -w ~/.tauri/ravens-nexus.key
```
И добавьте секреты в GitHub:
- `TAURI_SIGNING_PRIVATE_KEY` — содержимое `.key`-файла
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` — пароль
