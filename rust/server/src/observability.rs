use tracing_subscriber::{fmt, prelude::*, EnvFilter};

pub fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let json = std::env::var("RUST_LOG_FORMAT")
        .ok()
        .is_some_and(|value| value.eq_ignore_ascii_case("json"));

    let result = if json {
        tracing_subscriber::registry()
            .with(filter)
            .with(
                fmt::layer()
                    .json()
                    .with_current_span(true)
                    .with_span_list(true)
                    .with_thread_ids(true)
                    .with_target(true),
            )
            .try_init()
    } else {
        tracing_subscriber::registry()
            .with(filter)
            .with(
                fmt::layer()
                    .with_thread_ids(true)
                    .with_thread_names(true)
                    .with_target(true)
                    .with_file(true)
                    .with_line_number(true),
            )
            .try_init()
    };

    if let Err(error) = result {
        eprintln!("culebraluxe tracing subscriber not installed: {error}");
    }
}
