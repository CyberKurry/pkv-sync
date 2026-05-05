CREATE TABLE tokens_new (
    id              TEXT PRIMARY KEY,
    user_id         TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash      TEXT NOT NULL UNIQUE,
    device_id       TEXT NOT NULL,
    device_name     TEXT NOT NULL,
    created_at      INTEGER NOT NULL,
    last_used_at    INTEGER,
    revoked_at      INTEGER
);

INSERT INTO tokens_new
    (id, user_id, token_hash, device_id, device_name, created_at, last_used_at, revoked_at)
SELECT
    id,
    user_id,
    token_hash,
    'legacy_' || id,
    device_name,
    created_at,
    last_used_at,
    revoked_at
FROM tokens;

DROP TABLE tokens;
ALTER TABLE tokens_new RENAME TO tokens;

CREATE INDEX idx_tokens_hash ON tokens(token_hash) WHERE revoked_at IS NULL;
CREATE INDEX idx_tokens_user_device ON tokens(user_id, device_id) WHERE revoked_at IS NULL;
