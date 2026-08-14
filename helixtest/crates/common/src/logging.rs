use std::sync::Once;
use tracing_subscriber::{fmt, EnvFilter};

static INIT: Once = Once::new();

const DEFAULT_FILTER: &str = "info,framework=debug,common=debug,helixtest_cli=debug";

/// Initialize HelixTest logging. `--verbose` enables debug if `RUST_LOG` is unset.
pub fn init_logging() {
    init_logging_verbose(false);
}

pub fn init_logging_verbose(verbose: bool) {
    INIT.call_once(|| {
        let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            if verbose {
                EnvFilter::new("debug,framework=debug,common=debug,helixtest_cli=debug")
            } else {
                EnvFilter::new(DEFAULT_FILTER)
            }
        });
        fmt()
            .with_env_filter(filter)
            .with_target(true)
            .with_timer(fmt::time::uptime())
            .init();
    });
}
