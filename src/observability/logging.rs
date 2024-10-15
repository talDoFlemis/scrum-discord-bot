use anyhow::{Context, Result};
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{logs::LoggerProvider, runtime};

use crate::configuration::Settings;

pub fn init_log(settings: &Settings) -> Result<LoggerProvider> {
    let logger_provider = match settings.otel.enable {
        true => opentelemetry_otlp::new_pipeline()
            .logging()
            .with_exporter(
                opentelemetry_otlp::new_exporter()
                    .tonic()
                    .with_endpoint(&settings.otel.endpoint),
            )
            .with_resource(settings.get_resource())
            .install_batch(runtime::Tokio)
            .context("expected to genereate otlp log provider")?,
        false => LoggerProvider::builder()
            .with_resource(settings.get_resource())
            .build(),
    };

    Ok(logger_provider)
}
