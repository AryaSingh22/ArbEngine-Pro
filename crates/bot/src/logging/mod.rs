use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

pub fn setup() {
    // Console layer for development
    let console_layer = fmt::layer()
        .with_target(true)
        .with_thread_ids(true)
        .with_level(true)
        .with_ansi(true)
        .compact(); // Compact format for cleaner logs

    // Environment filter (RUST_LOG or default)
    let filter_layer = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,solana_arb_bot=debug,solana_arb_core=info"));

    // Initialize registry
    tracing_subscriber::registry()
        .with(filter_layer)
        .with(console_layer)
        .init();
}
