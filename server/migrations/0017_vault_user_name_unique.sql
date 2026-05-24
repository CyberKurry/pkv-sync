-- Enforce per-user vault name uniqueness at the database layer. Fresh databases
-- already have this constraint from 0001_initial.sql; this named index makes the
-- invariant explicit for migrated databases too.
--
-- Operational caveat: if an existing database somehow contains duplicate
-- (user_id, name) rows, SQLite will fail this migration while creating the
-- unique index. That fail-fast behavior is intentional and must be resolved by
-- deduplicating the data before re-running migrations.
CREATE UNIQUE INDEX IF NOT EXISTS idx_vaults_user_name_unique
ON vaults(user_id, name);
