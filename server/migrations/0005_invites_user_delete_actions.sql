CREATE TABLE invites_new (
    code            TEXT PRIMARY KEY,
    created_by      TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at      INTEGER NOT NULL,
    expires_at      INTEGER,
    used_at         INTEGER,
    used_by         TEXT REFERENCES users(id) ON DELETE SET NULL
);

INSERT INTO invites_new (code, created_by, created_at, expires_at, used_at, used_by)
SELECT code, created_by, created_at, expires_at, used_at, used_by
FROM invites;

DROP TABLE invites;
ALTER TABLE invites_new RENAME TO invites;
