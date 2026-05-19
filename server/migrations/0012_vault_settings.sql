CREATE TABLE vault_settings (
    vault_id        TEXT NOT NULL,
    key             TEXT NOT NULL,
    value           TEXT NOT NULL,
    updated_at      INTEGER NOT NULL,
    PRIMARY KEY (vault_id, key),
    FOREIGN KEY (vault_id) REFERENCES vaults(id) ON DELETE CASCADE
);
