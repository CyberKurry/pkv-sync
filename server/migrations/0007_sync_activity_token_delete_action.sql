CREATE TABLE sync_activity_new (
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

INSERT INTO sync_activity_new (
    id,
    user_id,
    vault_id,
    token_id,
    action,
    commit_hash,
    client_ip,
    user_agent,
    timestamp,
    details
)
SELECT
    id,
    user_id,
    vault_id,
    token_id,
    action,
    commit_hash,
    client_ip,
    user_agent,
    timestamp,
    details
FROM sync_activity;

DROP TABLE sync_activity;
ALTER TABLE sync_activity_new RENAME TO sync_activity;

CREATE INDEX idx_sync_activity_vault ON sync_activity(vault_id, timestamp);
CREATE INDEX idx_sync_activity_ip ON sync_activity(client_ip, timestamp);
