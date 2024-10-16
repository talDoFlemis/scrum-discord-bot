use std::{net::SocketAddr, sync::Arc, time::Duration};
use tokio::sync::Mutex;

use anyhow::{Context, Result};
use axum::{
    body::Body,
    extract::State,
    http::{header::CONTENT_TYPE, Response, StatusCode},
    middleware,
    response::IntoResponse,
    routing::get,
    Router,
};
use axum_tracing_opentelemetry::middleware::{OtelAxumLayer, OtelInResponseLayer};
use opentelemetry::trace::TracerProvider as _;
use prometheus_client::{encoding::text::encode, registry::Registry};
use scrum_discord_bot::{
    configuration::{get_configuration, Settings},
    drivers::http::middlewares::{self},
    observability::{
        get_subscriber, init_subscriber,
        log::init_log,
        metrics::{init_metrics, Metrics},
        trace::init_trace,
    },
};
use tokio::signal;
use tower::ServiceBuilder;
use tower_http::{
    catch_panic::CatchPanicLayer,
    compression::CompressionLayer,
    normalize_path::NormalizePathLayer,
    timeout::{RequestBodyTimeoutLayer, TimeoutLayer},
    trace::{DefaultOnRequest, DefaultOnResponse, TraceLayer},
    validate_request::ValidateRequestHeaderLayer,
    CompressionLevel, LatencyUnit,
};
use tracing::Level;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[tokio::main]
#[tracing::instrument]
async fn main() -> Result<()> {
    let settings = get_configuration().expect("expected to parse configuration with success");

    // Tracing, logs and metrics
    let trace_provider = init_trace(&settings).expect("expected to get trace_provider");
    let tracer = trace_provider.tracer(settings.application.name.clone());
    let logger_provider = init_log(&settings).expect("expected to create logger provider");

    let subscriber = get_subscriber(
        settings.application.name.clone(),
        "info".into(),
        std::io::stdout,
        tracer,
        logger_provider.clone(),
    );
    init_subscriber(subscriber);

    let (metrics, registry) = init_metrics(&settings);
    let registry = Arc::new(Mutex::new(registry));

    metrics_server(&settings, registry).await?;

    tracing::info!(
        "listening on address for metrics {:?}",
        settings.prometheus.port
    );

    let app = app(&settings, metrics);

    let address = format!("{}:{}", settings.http.host, settings.http.port)
        .parse::<SocketAddr>()
        .context("expected to parse address")?;

    let listener = tokio::net::TcpListener::bind(address)
        .await
        .context("expected to create listener")?;

    tracing::info!("listening on address {:?}", address);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();

    opentelemetry::global::shutdown_tracer_provider();
    let _ = logger_provider.shutdown();

    Ok(())
}

fn app(settings: &Settings, metrics: Arc<Metrics>) -> Router {
    let telemetry_middleware = ServiceBuilder::new()
        .layer(OtelInResponseLayer)
        .layer(OtelAxumLayer::default());

    let default_middleware = ServiceBuilder::new()
        .layer(
            TraceLayer::new_for_http()
                .on_request(DefaultOnRequest::new().level(Level::INFO))
                .on_response(
                    DefaultOnResponse::new()
                        .level(Level::INFO)
                        .include_headers(true)
                        .latency_unit(LatencyUnit::Micros),
                ),
        )
        .layer(NormalizePathLayer::trim_trailing_slash())
        .layer(ValidateRequestHeaderLayer::accept("application/json"))
        .layer(CompressionLayer::new().quality(CompressionLevel::Fastest))
        .layer(RequestBodyTimeoutLayer::new(Duration::from_secs(
            settings.http.timeout,
        )))
        .layer(TimeoutLayer::new(Duration::from_secs(
            settings.http.timeout,
        )))
        .layer(CatchPanicLayer::new());

    let real_router = Router::new()
        .route_layer(middleware::from_fn_with_state(
            metrics.http.clone(),
            middlewares::metrics_middleware,
        ))
        .layer(telemetry_middleware)
        // Non telemetry layers that won't contain span shit
        .route("/healthz", get(health_handler))
        .layer(default_middleware);

    Router::new().nest(&settings.http.prefix, real_router)
}

pub async fn health_handler() -> &'static str {
    StatusCode::OK.as_str()
}

async fn metrics_handler(State(state): State<Arc<Mutex<Registry>>>) -> impl IntoResponse {
    let state = state.lock().await;
    let mut buffer = String::new();
    encode(&mut buffer, &state).unwrap();

    Response::builder()
        .status(StatusCode::OK)
        .header(
            CONTENT_TYPE,
            "application/openmetrics-text; version=1.0.0; charset=utf-8",
        )
        .body(Body::from(buffer))
        .unwrap()
}

async fn metrics_server(settings: &Settings, registry: Arc<Mutex<Registry>>) -> Result<()> {
    let router = Router::new()
        .route(&settings.prometheus.path, get(metrics_handler))
        .with_state(registry);

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", settings.prometheus.port))
        .await
        .context("expected to create listener")?;

    tokio::spawn(async move {
        axum::serve(listener, router)
            .with_graceful_shutdown(shutdown_signal())
            .await
            .expect("expected to listen to prometheus handler");
    });

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
