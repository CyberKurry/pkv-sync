INSERT OR IGNORE INTO runtime_config (key, value, updated_at, updated_by)
VALUES ('extra_exclude_globs', '[]', strftime('%s', 'now'), NULL);
