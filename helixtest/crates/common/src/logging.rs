use std::sync::Once;
use tracing_subscriber::{fmt, EnvFilter};

static INIT: Once = Once::new();

/// Initialize HelixTest logging. Log lines include structured key=value fields
/// (e.g. `info!(mode = ?m, "message")`). Use `RUST_LOG` to control level; `--verbose` sets debug.
pub fn init_logging() {
    INIT.call_once(|| {
        let filter = EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new("info,helixtest=debug"));
        fmt()
            .with_env_filter(filter)
            .with_target(true)
            .with_timer(fmt::time::uptime())
            .init();
    });
}

