INSERT OR IGNORE INTO runtime_config (key, value, updated_at) VALUES
  ('enable_metrics', 'false', strftime('%s','now'));
