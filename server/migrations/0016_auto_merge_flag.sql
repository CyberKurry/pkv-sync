INSERT OR IGNORE INTO runtime_config (key, value, updated_at) VALUES
  ('enable_auto_merge', 'true', strftime('%s','now'));
