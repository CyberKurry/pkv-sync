INSERT OR IGNORE INTO runtime_config (key, value, updated_at) VALUES
  ('enable_git_smart_http', 'false', strftime('%s','now'));
