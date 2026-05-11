INSERT OR IGNORE INTO runtime_config (key, value, updated_at, updated_by)
VALUES
  ('enable_history_ui', 'true', strftime('%s', 'now'), NULL),
  ('enable_diff_endpoint', 'true', strftime('%s', 'now'), NULL);
