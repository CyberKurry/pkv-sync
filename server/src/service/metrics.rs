use prometheus::{
    register_gauge_with_registry, register_histogram_vec_with_registry,
    register_int_counter_vec_with_registry, register_int_gauge_with_registry, Encoder, Gauge,
    HistogramVec, IntCounterVec, IntGauge, Registry, TextEncoder,
};
use std::sync::Arc;

pub struct Metrics {
    pub registry: Registry,
    pub http_requests_total: IntCounterVec,
    pub http_request_duration_seconds: HistogramVec,
    pub push_changes_total: IntCounterVec,
    pub pull_files_total: IntCounterVec,
    pub sse_subscribers: IntGauge,
    pub active_tokens: IntGauge,
    pub vaults_total: IntGauge,
    pub blobs_total: IntGauge,
    pub blob_gc_last_run_unix_seconds: IntGauge,
    pub blob_gc_freed_bytes_total: IntCounterVec,
    pub git_repo_size_bytes: Gauge,
}

impl Metrics {
    pub fn new() -> Arc<Self> {
        let registry = Registry::new();
        let http_requests_total = register_int_counter_vec_with_registry!(
            "pkv_http_requests_total",
            "Total HTTP requests by route, method, and status code",
            &["route", "method", "code"],
            registry
        )
        .expect("register pkv_http_requests_total");
        let http_request_duration_seconds = register_histogram_vec_with_registry!(
            "pkv_http_request_duration_seconds",
            "HTTP request duration in seconds",
            &["route", "method"],
            vec![0.005, 0.01, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0],
            registry
        )
        .expect("register pkv_http_request_duration_seconds");
        let push_changes_total = register_int_counter_vec_with_registry!(
            "pkv_push_changes_total",
            "Total pushed changes by kind",
            &["kind"],
            registry
        )
        .expect("register pkv_push_changes_total");
        let pull_files_total = register_int_counter_vec_with_registry!(
            "pkv_pull_files_total",
            "Total pulled files by change bucket",
            &["bucket"],
            registry
        )
        .expect("register pkv_pull_files_total");
        let sse_subscribers = register_int_gauge_with_registry!(
            "pkv_sse_subscribers",
            "Current active SSE subscribers",
            registry
        )
        .expect("register pkv_sse_subscribers");
        let active_tokens = register_int_gauge_with_registry!(
            "pkv_active_tokens",
            "Current active unrevoked device tokens",
            registry
        )
        .expect("register pkv_active_tokens");
        let vaults_total =
            register_int_gauge_with_registry!("pkv_vaults_total", "Current vault count", registry)
                .expect("register pkv_vaults_total");
        let blobs_total =
            register_int_gauge_with_registry!("pkv_blobs_total", "Current blob count", registry)
                .expect("register pkv_blobs_total");
        let blob_gc_last_run_unix_seconds = register_int_gauge_with_registry!(
            "pkv_blob_gc_last_run_unix_seconds",
            "Last blob garbage collection completion time as Unix seconds",
            registry
        )
        .expect("register pkv_blob_gc_last_run_unix_seconds");
        let blob_gc_freed_bytes_total = register_int_counter_vec_with_registry!(
            "pkv_blob_gc_freed_bytes_total",
            "Total bytes freed by blob garbage collection",
            &["reason"],
            registry
        )
        .expect("register pkv_blob_gc_freed_bytes_total");
        let git_repo_size_bytes = register_gauge_with_registry!(
            "pkv_git_repo_size_bytes",
            "Total bytes used by vault git repositories",
            registry
        )
        .expect("register pkv_git_repo_size_bytes");

        http_requests_total.with_label_values(&["unknown", "unknown", "0"]);
        http_request_duration_seconds.with_label_values(&["unknown", "unknown"]);
        push_changes_total.with_label_values(&["text"]);
        push_changes_total.with_label_values(&["blob"]);
        push_changes_total.with_label_values(&["delete"]);
        pull_files_total.with_label_values(&["added"]);
        pull_files_total.with_label_values(&["modified"]);
        pull_files_total.with_label_values(&["deleted"]);
        blob_gc_freed_bytes_total.with_label_values(&["gc"]);

        Arc::new(Self {
            registry,
            http_requests_total,
            http_request_duration_seconds,
            push_changes_total,
            pull_files_total,
            sse_subscribers,
            active_tokens,
            vaults_total,
            blobs_total,
            blob_gc_last_run_unix_seconds,
            blob_gc_freed_bytes_total,
            git_repo_size_bytes,
        })
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        TextEncoder::new()
            .encode(&self.registry.gather(), &mut buf)
            .expect("encode prometheus metrics");
        buf
    }
}
