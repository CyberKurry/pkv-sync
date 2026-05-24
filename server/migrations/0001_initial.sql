-- 0001_initial.sql - PKV Sync v1.0 baseline schema
--
-- v1.0.0 intentionally resets the SQLite migration baseline. Fresh databases
-- start from this complete schema. Databases created by 0.x releases must not
-- be upgraded in-place with this migration set.

CREATE TABLE IF NOT EXISTS users (
    id              TEXT PRIMARY KEY,
    username        TEXT UNIQUE NOT NULL,
    password_hash   TEXT NOT NULL,
    is_admin        BOOLEAN NOT NULL DEFAULT 0,
    is_active       BOOLEAN NOT NULL DEFAULT 1,
    created_at      INTEGER NOT NULL,
    last_login_at   INTEGER
);

CREATE TABLE IF NOT EXISTS vaults (
    id              TEXT PRIMARY KEY,
    user_id         TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name            TEXT NOT NULL,
    created_at      INTEGER NOT NULL,
    last_sync_at    INTEGER,
    size_bytes      INTEGER NOT NULL DEFAULT 0,
    file_count      INTEGER NOT NULL DEFAULT 0
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_vaults_user_name_unique ON vaults(user_id, name);

CREATE TABLE IF NOT EXISTS tokens (
    id              TEXT PRIMARY KEY,
    user_id         TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash      TEXT NOT NULL UNIQUE,
    device_id       TEXT NOT NULL,
    device_name     TEXT NOT NULL,
    created_at      INTEGER NOT NULL,
    expires_at      INTEGER NOT NULL,
    last_used_at    INTEGER,
    revoked_at      INTEGER
);
CREATE INDEX IF NOT EXISTS idx_tokens_hash ON tokens(token_hash) WHERE revoked_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_tokens_user_device ON tokens(user_id, device_id) WHERE revoked_at IS NULL;

CREATE TABLE IF NOT EXISTS invites (
    code            TEXT PRIMARY KEY,
    created_by      TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at      INTEGER NOT NULL,
    expires_at      INTEGER,
    used_at         INTEGER,
    used_by         TEXT REFERENCES users(id) ON DELETE SET NULL
);

CREATE TABLE IF NOT EXISTS blob_refs (
    blob_hash       TEXT NOT NULL,
    vault_id        TEXT NOT NULL REFERENCES vaults(id) ON DELETE CASCADE,
    commit_hash     TEXT NOT NULL,
    PRIMARY KEY (blob_hash, vault_id, commit_hash)
);
CREATE INDEX IF NOT EXISTS idx_blob_refs_hash ON blob_refs(blob_hash);

CREATE TABLE IF NOT EXISTS sync_activity (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id         TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    vault_id        TEXT REFERENCES vaults(id) ON DELETE CASCADE,
    token_id        TEXT REFERENCES tokens(id) ON DELETE SET NULL,
    action          TEXT NOT NULL,
    commit_hash     TEXT,
    client_ip       TEXT,
    user_agent      TEXT,
    timestamp       INTEGER NOT NULL,
    details         TEXT
);
CREATE INDEX IF NOT EXISTS idx_sync_activity_vault ON sync_activity(vault_id, timestamp);
CREATE INDEX IF NOT EXISTS idx_sync_activity_ip ON sync_activity(client_ip, timestamp);

CREATE TABLE IF NOT EXISTS idempotency_cache (
    user_id         TEXT NOT NULL,
    key             TEXT NOT NULL,
    vault_id        TEXT NOT NULL,
    route           TEXT NOT NULL,
    request_hash    TEXT NOT NULL,
    response_json   TEXT NOT NULL,
    created_at      INTEGER NOT NULL,
    PRIMARY KEY (user_id, key, vault_id, route)
);
CREATE INDEX IF NOT EXISTS idx_idempotency_created ON idempotency_cache(created_at);

CREATE TABLE IF NOT EXISTS runtime_config (
    key             TEXT PRIMARY KEY,
    value           TEXT NOT NULL,
    updated_at      INTEGER NOT NULL,
    updated_by      TEXT REFERENCES users(id)
);

CREATE TABLE IF NOT EXISTS admin_sessions (
    id              TEXT PRIMARY KEY,
    user_id         TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at      INTEGER NOT NULL,
    expires_at      INTEGER NOT NULL,
    last_seen_at    INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_admin_sessions_user ON admin_sessions(user_id);
CREATE INDEX IF NOT EXISTS idx_admin_sessions_expires ON admin_sessions(expires_at);

CREATE TABLE IF NOT EXISTS vault_settings (
    vault_id        TEXT NOT NULL,
    key             TEXT NOT NULL,
    value           TEXT NOT NULL,
    updated_at      INTEGER NOT NULL,
    PRIMARY KEY (vault_id, key),
    FOREIGN KEY (vault_id) REFERENCES vaults(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS blob_uploads (
    blob_hash       TEXT NOT NULL,
    vault_id        TEXT NOT NULL REFERENCES vaults(id) ON DELETE CASCADE,
    uploaded_at     INTEGER NOT NULL,
    PRIMARY KEY (blob_hash, vault_id)
);
CREATE INDEX IF NOT EXISTS idx_blob_uploads_vault ON blob_uploads(vault_id);

INSERT OR IGNORE INTO runtime_config (key, value, updated_at, updated_by) VALUES
  ('enable_history_ui', 'true', strftime('%s', 'now'), NULL),
  ('enable_diff_endpoint', 'true', strftime('%s', 'now'), NULL),
  ('extra_exclude_globs', '[]', strftime('%s', 'now'), NULL),
  ('inline_content_max_bytes', '8192', strftime('%s', 'now'), NULL),
  ('sse_heartbeat_seconds', '30', strftime('%s', 'now'), NULL),
  ('push_debounce_ms', '250', strftime('%s', 'now'), NULL),
  ('enable_git_smart_http', 'false', strftime('%s', 'now'), NULL),
  ('enable_metrics', 'false', strftime('%s', 'now'), NULL),
  ('enable_auto_merge', 'true', strftime('%s', 'now'), NULL);
