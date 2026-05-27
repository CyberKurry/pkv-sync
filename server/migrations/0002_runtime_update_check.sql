INSERT INTO runtime_config (key, value, updated_at, updated_by)
SELECT 'update_check.enabled', 'true', strftime('%s', 'now'), NULL
WHERE NOT EXISTS (SELECT 1 FROM runtime_config WHERE key = 'update_check.enabled');

INSERT INTO runtime_config (key, value, updated_at, updated_by)
SELECT 'update_check.interval_seconds', '86400', strftime('%s', 'now'), NULL
WHERE NOT EXISTS (SELECT 1 FROM runtime_config WHERE key = 'update_check.interval_seconds');
