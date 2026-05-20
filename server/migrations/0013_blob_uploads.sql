CREATE TABLE IF NOT EXISTS blob_uploads (
    blob_hash       TEXT NOT NULL,
    vault_id        TEXT NOT NULL REFERENCES vaults(id) ON DELETE CASCADE,
    uploaded_at     INTEGER NOT NULL,
    PRIMARY KEY (blob_hash, vault_id)
);

CREATE INDEX IF NOT EXISTS idx_blob_uploads_vault ON blob_uploads(vault_id);
