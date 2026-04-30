-- 0001_initial.sql - PKV Sync v1 baseline schema

CREATE TABLE users (
    id              TEXT PRIMARY KEY,
    username        TEXT UNIQUE NOT NULL,
    password_hash   TEXT NOT NULL,
    is_admin        BOOLEAN NOT NULL DEFAULT 0,
    is_active       BOOLEAN NOT NULL DEFAULT 1,
    created_at      INTEGER NOT NULL,
    last_login_at   INTEGER
);

CREATE TABLE vaults (
    id              TEXT PRIMARY KEY,
    user_id         TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name            TEXT NOT NULL,
    created_at      INTEGER NOT NULL,
    last_sync_at    INTEGER,
    size_bytes      INTEGER NOT NULL DEFAULT 0,
    file_count      INTEGER NOT NULL DEFAULT 0,
    UNIQUE (user_id, name)
);

CREATE TABLE tokens (
    id              TEXT PRIMARY KEY,
    user_id         TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash      TEXT NOT NULL UNIQUE,
    device_name     TEXT NOT NULL,
    created_at      INTEGER NOT NULL,
    last_used_at    INTEGER,
    revoked_at      INTEGER
);
CREATE INDEX idx_tokens_hash ON tokens(token_hash) WHERE revoked_at IS NULL;

CREATE TABLE invites (
    code            TEXT PRIMARY KEY,
    created_by      TEXT NOT NULL REFERENCES users(id),
    created_at      INTEGER NOT NULL,
    expires_at      INTEGER,
    used_at         INTEGER,
    used_by         TEXT REFERENCES users(id)
);

CREATE TABLE blob_refs (
    blob_hash       TEXT NOT NULL,
    vault_id        TEXT NOT NULL REFERENCES vaults(id) ON DELETE CASCADE,
    commit_hash     TEXT NOT NULL,
    PRIMARY KEY (blob_hash, vault_id, commit_hash)
);
CREATE INDEX idx_blob_refs_hash ON blob_refs(blob_hash);

CREATE TABLE sync_activity (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id         TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    vault_id        TEXT REFERENCES vaults(id) ON DELETE CASCADE,
    token_id        TEXT REFERENCES tokens(id),
    action          TEXT NOT NULL,
    commit_hash     TEXT,
    client_ip       TEXT,
    user_agent      TEXT,
    timestamp       INTEGER NOT NULL,
    details         TEXT
);
CREATE INDEX idx_sync_activity_vault ON sync_activity(vault_id, timestamp);
CREATE INDEX idx_sync_activity_ip ON sync_activity(client_ip, timestamp);

CREATE TABLE idempotency_cache (
    key             TEXT PRIMARY KEY,
    user_id         TEXT NOT NULL,
    response_json   TEXT NOT NULL,
    created_at      INTEGER NOT NULL
);
CREATE INDEX idx_idempotency_created ON idempotency_cache(created_at);

CREATE TABLE runtime_config (
    key             TEXT PRIMARY KEY,
    value           TEXT NOT NULL,
    updated_at      INTEGER NOT NULL,
    updated_by      TEXT REFERENCES users(id)
);
