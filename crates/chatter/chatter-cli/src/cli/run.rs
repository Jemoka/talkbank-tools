//! Command dispatch from parsed CLI arguments into command handlers.
//!
//! [`run`] is the single entry point after argument parsing. It resolves cross-cutting
//! concerns — TUI auto-detection, tracing initialisation, color theme loading — then
//! hands the parsed command to the feature-oriented CLI command services in
//! [`crate::commands`].
//!
//! TUI mode is enabled automatically when stdout is a TTY unless `--tui-mode disable`
//! is passed. When active, tracing output is suppressed so it does not interleave with
//! the interactive display.

use crate::commands::{self, CommandContext};
use crate::ui::Theme;

/// Execute the CLI command
pub fn run(cli: super::Cli) {
    // Resolve TUI mode into a concrete decision
    let should_use_tui = cli.tui_mode.should_use_tui();

    // Detect if TUI mode is being used - disable logging if so to avoid cluttering the display
    super::init_tracing(cli.verbose, &cli.log_format, should_use_tui);

    // Load color theme for TUI mode
    let theme = Theme::load(cli.theme);

    commands::dispatch_command(
        cli.command,
        &CommandContext {
            should_use_tui,
            theme,
        },
    );
}
