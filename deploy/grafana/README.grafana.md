# PKV Sync Grafana Dashboard

This directory contains a Grafana 10+ dashboard template for PKV Sync Prometheus metrics:

- `pkv-sync.json`

## Enable Metrics

PKV Sync does not expose metrics by default. Enable the runtime setting before configuring Prometheus:

```toml
enable_metrics = true
```

The `/metrics` endpoint is gated by the deployment key. Prometheus must include the same deployment key used by the server, sent as `x-pkvsync-deployment-key`.

Example Prometheus scrape job:

```yaml
scrape_configs:
  - job_name: pkv-sync
    metrics_path: /metrics
    scheme: https
    static_configs:
      - targets:
          - pkv-sync.example.com
    http_headers:
      x-pkvsync-deployment-key:
        secrets:
          - "${PKVSYNC_DEPLOYMENT_KEY}"
```

If your Prometheus version or deployment method does not support per-job `http_headers`, place PKV Sync behind a scrape proxy that injects `x-pkvsync-deployment-key` for Prometheus only.

## Import Into Grafana

1. Open Grafana.
2. Go to **Dashboards** > **New** > **Import**.
3. Upload `deploy/grafana/pkv-sync.json`, or paste its JSON content.
4. Select your Prometheus datasource for the `DS_PROMETHEUS` variable.
5. Import the dashboard.

The dashboard uses `$__rate_interval` and should work with Grafana 10+ and a Prometheus-compatible datasource.
