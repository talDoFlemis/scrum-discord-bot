use anyhow::{Context, Result};
use opentelemetry::global;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{
    propagation::TraceContextPropagator,
    runtime,
    trace::{self, RandomIdGenerator, Sampler, TracerProvider},
};

use crate::configuration::Settings;

pub fn init_trace(settings: &Settings) -> Result<TracerProvider> {
    global::set_text_map_propagator(TraceContextPropagator::new());

    let trace_provider = match settings.otel.enable {
        true => opentelemetry_otlp::new_pipeline()
            .tracing()
            .with_exporter(
                opentelemetry_otlp::new_exporter()
                    .tonic()
                    .with_endpoint(&settings.otel.endpoint),
            )
            .with_trace_config(
                trace::Config::default()
                    .with_sampler(Sampler::AlwaysOn)
                    .with_id_generator(RandomIdGenerator::default())
                    .with_resource(settings.get_resource()),
            )
            .install_batch(runtime::Tokio)
            .context("expected to genereate otlp provider")?,
        false => TracerProvider::builder()
            .with_simple_exporter(opentelemetry_stdout::SpanExporter::default())
            .build(),
    };

    Ok(trace_provider)
}
