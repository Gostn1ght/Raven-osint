-- Ravens Nexus — Initial Schema

CREATE TABLE IF NOT EXISTS users (
    id              TEXT PRIMARY KEY,
    username        TEXT UNIQUE NOT NULL,
    email           TEXT UNIQUE NOT NULL,
    password_hash   TEXT NOT NULL DEFAULT '',
    rank            TEXT NOT NULL DEFAULT 'ANALYST',
    hwid            TEXT,
    api_key         TEXT UNIQUE NOT NULL,
    created_at      TEXT NOT NULL,
    last_login      TEXT,
    login_attempts  INTEGER NOT NULL DEFAULT 0,
    locked_until    TEXT,
    avatar_url      TEXT,
    bio             TEXT
);

CREATE TABLE IF NOT EXISTS subscriptions (
    id              TEXT PRIMARY KEY,
    user_id         TEXT UNIQUE NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    plan            TEXT NOT NULL DEFAULT 'free',
    expires_at      TEXT,
    requests_used   INTEGER NOT NULL DEFAULT 0,
    requests_limit  INTEGER NOT NULL DEFAULT 20,
    created_at      TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS request_log (
    id          TEXT PRIMARY KEY,
    user_id     TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    tool        TEXT NOT NULL,
    query       TEXT NOT NULL,
    result      TEXT,
    duration_ms INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_reqlog_user ON request_log(user_id, created_at DESC);

CREATE TABLE IF NOT EXISTS payments (
    id          TEXT PRIMARY KEY,
    user_id     TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    provider    TEXT NOT NULL,
    amount      REAL NOT NULL,
    currency    TEXT NOT NULL DEFAULT 'RUB',
    status      TEXT NOT NULL DEFAULT 'pending',
    plan        TEXT NOT NULL,
    external_id TEXT,
    created_at  TEXT NOT NULL,
    paid_at     TEXT
);

CREATE TABLE IF NOT EXISTS posts (
    id          TEXT PRIMARY KEY,
    user_id     TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    title       TEXT NOT NULL,
    body        TEXT NOT NULL,
    tags        TEXT,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS comments (
    id          TEXT PRIMARY KEY,
    post_id     TEXT NOT NULL REFERENCES posts(id) ON DELETE CASCADE,
    user_id     TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    body        TEXT NOT NULL,
    created_at  TEXT NOT NULL
);
