//! Tracing subscriber setup for CLI and TUI execution modes.
//!
//! Logging is OFF by default — `chatter` produces only structured validation output
//! unless the user explicitly opts in with `-v` flags or the `RUST_LOG` environment
//! variable. This avoids polluting validation results with log noise and keeps TUI
//! rendering clean.
//!
//! Verbosity ladder: `-v` (WARN) → `-vv` (INFO) → `-vvv` (DEBUG) → `-vvvv` (TRACE).
//! When `RUST_LOG` is set it takes precedence over the flag count. If neither is
//! present, no subscriber is installed at all, so `tracing` macros are zero-cost.

use tracing::{Level, debug};
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

use super::LogFormat;

/// Initialize tracing/logging based on verbosity level and format
///
/// Logging is OFF by default. Use verbosity flags to enable:
/// - `-v`: WARN level (warnings and errors)
/// - `-vv`: INFO level
/// - `-vvv`: DEBUG level
/// - `-vvvv`: TRACE level
pub fn init_tracing(verbosity: u8, log_format: &LogFormat, _is_tui_mode: bool) {
    // Determine log level from verbosity count - OFF by default unless RUST_LOG is set
    let level = match verbosity {
        0 => None,
        1 => Some(Level::WARN),  // -v: warnings and errors
        2 => Some(Level::INFO),  // -vv: info, warnings, errors
        3 => Some(Level::DEBUG), // -vvv: debug, info, warnings, errors
        _ => Some(Level::TRACE), // -vvvv+: all logging
    };

    // Build EnvFilter that respects RUST_LOG if set, otherwise uses verbosity.
    let env_filter = EnvFilter::try_from_default_env()
        .ok()
        .or_else(|| level.map(|lvl| EnvFilter::new(lvl.as_str())));

    // If neither verbosity nor RUST_LOG is set, logging is disabled.
    let Some(env_filter) = env_filter else {
        return;
    };

    // Initialize subscriber based on format
    match log_format {
        LogFormat::Text => {
            tracing_subscriber::registry()
                .with(env_filter)
                .with(fmt::layer())
                .init();
        }
        LogFormat::Json => {
            tracing_subscriber::registry()
                .with(env_filter)
                .with(fmt::layer().json())
                .init();
        }
    }

    debug!("Logging initialized at level: {:?}", level);
}
