INSERT OR IGNORE INTO runtime_config (key, value, updated_at) VALUES
  ('inline_content_max_bytes', '8192', strftime('%s','now')),
  ('sse_heartbeat_seconds', '30', strftime('%s','now')),
  ('push_debounce_ms', '250', strftime('%s','now'));
