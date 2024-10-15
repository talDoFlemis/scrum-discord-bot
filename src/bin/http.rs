use anyhow::Result;
use opentelemetry::trace::TracerProvider as _;
use scrum_discord_bot::{
    configuration::get_configuration,
    observability::{
        logging::init_log,
        tracing::{get_subscriber, init_subscriber, init_trace},
    },
};

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() -> Result<()> {
    let settings = get_configuration().expect("expected to parse configuration with success");

    let trace_provider = init_trace(&settings).expect("expected to get trace_provider");
    let tracer = trace_provider.tracer(settings.application.name.clone());
    let logger_provider = init_log(&settings).expect("expected to create logger provider");

    let subscriber = get_subscriber(
        settings.application.name,
        "info".into(),
        std::io::stdout,
        tracer,
        logger_provider.clone(),
    );
    init_subscriber(subscriber);

    tracing::info!("hello men");

    opentelemetry::global::shutdown_tracer_provider();
    let _ = logger_provider.shutdown();


    Ok(())
}
