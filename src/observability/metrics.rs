use std::sync::Arc;

use prometheus_client::{
    encoding::EncodeLabelSet,
    metrics::{counter::Counter, family::Family, histogram::Histogram},
    registry::Registry,
};

use prometheus_client_derive_encode::EncodeLabelValue;

use crate::configuration::Settings;

pub struct Metrics {
    pub http: Arc<HttpMetrics>,
}

#[derive(Clone, Debug)]
pub struct HttpMetrics {
    pub total_requests: Family<HttpRequestLabels, Counter>,
    pub request_with_error: Family<HttpRequestLabels, Counter>,
    pub latency_error: Family<HttpRequestLabels, Histogram>,
    pub latency_success: Family<HttpRequestLabels, Histogram>,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelValue)]
pub enum Method {
    Get,
    Put,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub struct HttpRequestLabels {
    pub method: Method,
    pub path: String,
    pub status_code: u32,
}

impl Default for HttpMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpMetrics {
    pub fn new() -> Self {
        Self {
            total_requests: Family::default(),
            request_with_error: Family::default(),
            latency_error: Family::<HttpRequestLabels, Histogram>::new_with_constructor(|| {
                let custom_buckets = [
                    0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
                ];
                Histogram::new(custom_buckets.into_iter())
            }),
            latency_success: Family::<HttpRequestLabels, Histogram>::new_with_constructor(|| {
                let custom_buckets = [
                    0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
                ];
                Histogram::new(custom_buckets.into_iter())
            }),
        }
    }

    pub fn register(&self, registry: &mut Registry) {
        registry.register(
            "total_request",
            "Total amount of requests",
            self.total_requests.clone(),
        );

        registry.register(
            "requests_with_error",
            "Amount of requests with error",
            self.request_with_error.clone(),
        );

        registry.register("latency_error", "Latency error", self.latency_error.clone());

        registry.register(
            "latency_success",
            "Latency success",
            self.latency_success.clone(),
        );
    }
}

pub fn init_metrics(settings: &Settings) -> (Arc<Metrics>, Registry) {
    let mut registry = Registry::with_prefix(&settings.application.name);

    let http_metrics = HttpMetrics::default();
    http_metrics.register(&mut registry);

    let metrics = Metrics {
        http: http_metrics.into(),
    };

    (Arc::new(metrics), registry)
}
