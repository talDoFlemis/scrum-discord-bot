use std::{sync::Arc, time::Instant};

use axum::{
    extract::{MatchedPath, Request, State},
    middleware::Next,
    response::IntoResponse,
};

use crate::observability::metrics::{HttpMetrics, HttpRequestLabels};

#[tracing::instrument(name = "Metrics middleware", skip(state, req, next))]
pub async fn metrics_middleware(
    State(state): State<Arc<HttpMetrics>>,
    req: Request,
    next: Next,
) -> impl IntoResponse {
    let start = Instant::now();
    let path = if let Some(matched_path) = req.extensions().get::<MatchedPath>() {
        matched_path.as_str().to_owned()
    } else {
        req.uri().path().to_owned()
    };
    let method = req.method().to_string();

    let response = next.run(req).await;

    let latency = start.elapsed().as_secs_f64();
    let status_code: u32 = response.status().as_u16().into();

    let labels = HttpRequestLabels {
        path,
        method,
        status_code,
    };

    state.total_requests.get_or_create(&labels).inc();

    if status_code > 200 && status_code < 400 {
        state
            .latency_success
            .get_or_create(&labels)
            .observe(latency)
    } else {
        state.latency_error.get_or_create(&labels).observe(latency)
    }

    response
}
