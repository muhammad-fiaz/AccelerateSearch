//! Prometheus metrics for AccelerateSearch.
//!
//! Exposes the standard set of operational metrics used by the platform.
//! Mounted at `GET /metrics` in the REST API.

use once_cell::sync::Lazy;
use prometheus::{
    Encoder, Gauge, Histogram, HistogramOpts, IntCounterVec, IntGauge, Opts, Registry, TextEncoder,
};

/// Global Prometheus registry for AccelerateSearch metrics.
pub static REGISTRY: Lazy<Registry> = Lazy::new(Registry::new);

/// Total HTTP requests, labelled by route and status.
pub static HTTP_REQUESTS_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    let v = IntCounterVec::new(
        Opts::new("accelerate_http_requests_total", "Total HTTP requests"),
        &["route", "status"],
    )
    .expect("metric");
    REGISTRY.register(Box::new(v.clone())).ok();
    v
});

/// HTTP request duration histogram.
pub static HTTP_REQUEST_DURATION_SECONDS: Lazy<Histogram> = Lazy::new(|| {
    let h = Histogram::with_opts(
        HistogramOpts::new(
            "accelerate_http_request_duration_seconds",
            "HTTP request duration in seconds",
        )
        .buckets(vec![
            0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
        ]),
    )
    .expect("histogram");
    REGISTRY.register(Box::new(h.clone())).ok();
    h
});

/// Total search requests, labelled by collection.
pub static SEARCH_REQUESTS_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    let v = IntCounterVec::new(
        Opts::new("accelerate_search_requests_total", "Total search requests"),
        &["collection"],
    )
    .expect("metric");
    REGISTRY.register(Box::new(v.clone())).ok();
    v
});

/// Search request duration histogram.
pub static SEARCH_DURATION_SECONDS: Lazy<Histogram> = Lazy::new(|| {
    let h = Histogram::with_opts(
        HistogramOpts::new(
            "accelerate_search_duration_seconds",
            "Search request duration in seconds",
        )
        .buckets(vec![
            0.0005, 0.001, 0.0025, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0,
        ]),
    )
    .expect("histogram");
    REGISTRY.register(Box::new(h.clone())).ok();
    h
});

/// Total documents indexed, labelled by collection.
pub static DOCUMENTS_INDEXED_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    let v = IntCounterVec::new(
        Opts::new(
            "accelerate_documents_indexed_total",
            "Total documents indexed",
        ),
        &["collection"],
    )
    .expect("metric");
    REGISTRY.register(Box::new(v.clone())).ok();
    v
});

/// Index size in bytes, labelled by collection.
pub static INDEX_SIZE_BYTES: Lazy<IntGauge> = Lazy::new(|| {
    let g = IntGauge::with_opts(Opts::new(
        "accelerate_index_size_bytes",
        "Index size in bytes",
    ))
    .expect("gauge");
    REGISTRY.register(Box::new(g.clone())).ok();
    g
});

/// Total tasks processed, labelled by type and status.
pub static TASKS_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    let v = IntCounterVec::new(
        Opts::new("accelerate_tasks_total", "Total tasks"),
        &["type", "status"],
    )
    .expect("metric");
    REGISTRY.register(Box::new(v.clone())).ok();
    v
});

/// Task processing duration histogram.
pub static TASK_PROCESSING_DURATION_SECONDS: Lazy<Histogram> = Lazy::new(|| {
    let h = Histogram::with_opts(
        HistogramOpts::new(
            "accelerate_task_processing_duration_seconds",
            "Task processing duration in seconds",
        )
        .buckets(vec![0.001, 0.01, 0.1, 1.0, 10.0, 60.0, 600.0]),
    )
    .expect("histogram");
    REGISTRY.register(Box::new(h.clone())).ok();
    h
});

/// Total number of collections.
pub static COLLECTIONS_TOTAL: Lazy<IntGauge> = Lazy::new(|| {
    let g = IntGauge::with_opts(Opts::new(
        "accelerate_collections_total",
        "Total number of collections",
    ))
    .expect("gauge");
    REGISTRY.register(Box::new(g.clone())).ok();
    g
});

/// Process memory usage in bytes (RSS).
pub static MEMORY_USAGE_BYTES: Lazy<IntGauge> = Lazy::new(|| {
    let g = IntGauge::with_opts(Opts::new(
        "accelerate_memory_usage_bytes",
        "Memory usage in bytes",
    ))
    .expect("gauge");
    REGISTRY.register(Box::new(g.clone())).ok();
    g
});

/// Server uptime in seconds.
pub static UPTIME_SECONDS: Lazy<Gauge> = Lazy::new(|| {
    let g = Gauge::with_opts(Opts::new(
        "accelerate_uptime_seconds",
        "Server uptime in seconds",
    ))
    .expect("gauge");
    REGISTRY.register(Box::new(g.clone())).ok();
    g
});

/// Forces the lazy initialisation of all metrics.
pub fn init() {
    Lazy::force(&HTTP_REQUESTS_TOTAL);
    Lazy::force(&HTTP_REQUEST_DURATION_SECONDS);
    Lazy::force(&SEARCH_REQUESTS_TOTAL);
    Lazy::force(&SEARCH_DURATION_SECONDS);
    Lazy::force(&DOCUMENTS_INDEXED_TOTAL);
    Lazy::force(&INDEX_SIZE_BYTES);
    Lazy::force(&TASKS_TOTAL);
    Lazy::force(&TASK_PROCESSING_DURATION_SECONDS);
    Lazy::force(&COLLECTIONS_TOTAL);
    Lazy::force(&MEMORY_USAGE_BYTES);
    Lazy::force(&UPTIME_SECONDS);
}

/// Encodes the current state of all registered metrics to a text-format
/// Prometheus payload.
#[must_use]
pub fn gather() -> Vec<u8> {
    let metric_families = REGISTRY.gather();
    let mut buffer = Vec::new();
    let encoder = TextEncoder::new();
    let _ = encoder.encode(&metric_families, &mut buffer);
    buffer
}

/// Records the current RSS of the process into the memory gauge.
pub fn record_memory_usage() {
    #[cfg(target_os = "linux")]
    {
        use std::fs;
        if let Ok(s) = fs::read_to_string("/proc/self/statm") {
            if let Some(rss_pages) = s.split_whitespace().nth(1) {
                if let Ok(pages) = rss_pages.parse::<u64>() {
                    MEMORY_USAGE_BYTES.set((pages * 4096) as i64);
                    return;
                }
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        // mach_task_basic_info would be ideal; keep the implementation
        // cross-platform friendly by recording 0 when unavailable.
        MEMORY_USAGE_BYTES.set(0);
    }
    #[cfg(target_os = "windows")]
    {
        // Avoid pulling the `windows` crate to keep dependencies lean. The
        // working-set size is only reported when the `psutil`-style crate is
        // available; otherwise we record 0 (the metric will still be visible).
        MEMORY_USAGE_BYTES.set(0);
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        MEMORY_USAGE_BYTES.set(0);
    }
}

/// Middleware-style helper that increments the HTTP counters.
pub fn observe_http_request(route: &str, status: u16, duration_secs: f64) {
    HTTP_REQUESTS_TOTAL
        .with_label_values(&[route, &status.to_string()])
        .inc();
    HTTP_REQUEST_DURATION_SECONDS.observe(duration_secs);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_does_not_panic() {
        init();
        let payload = gather();
        let s = String::from_utf8(payload).unwrap();
        assert!(s.contains("# TYPE"));
    }

    #[test]
    fn observe_http_request_works() {
        init();
        observe_http_request("/health", 200, 0.001);
        let payload = gather();
        let s = String::from_utf8(payload).unwrap();
        assert!(s.contains("accelerate_http_requests_total"));
    }
}
