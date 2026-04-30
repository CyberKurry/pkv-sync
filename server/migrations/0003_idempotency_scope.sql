CREATE TABLE idempotency_cache_new (
    user_id         TEXT NOT NULL,
    key             TEXT NOT NULL,
    vault_id        TEXT NOT NULL DEFAULT '',
    route           TEXT NOT NULL DEFAULT '',
    request_hash    TEXT NOT NULL DEFAULT '',
    response_json   TEXT NOT NULL,
    created_at      INTEGER NOT NULL,
    PRIMARY KEY (user_id, key)
);

INSERT OR IGNORE INTO idempotency_cache_new
    (user_id, key, vault_id, route, request_hash, response_json, created_at)
SELECT
    user_id,
    key,
    '',
    '',
    '',
    response_json,
    created_at
FROM idempotency_cache;

DROP TABLE idempotency_cache;
ALTER TABLE idempotency_cache_new RENAME TO idempotency_cache;

CREATE INDEX idx_idempotency_created ON idempotency_cache(created_at);
