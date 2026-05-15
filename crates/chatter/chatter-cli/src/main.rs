#![warn(missing_docs)]
// Test code is exempt from this crate's `deny`-level panic lints —
// see `book/src/operations/panic-audit/chatter-cli.md`.
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unreachable,
        clippy::todo,
        clippy::unimplemented
    )
)]
//! `chatter` -- command-line interface for CHAT format validation, conversion,
//! and analysis.
//!
//! This binary (`chatter`) is the main user-facing tool in the TalkBank
//! toolchain. It validates CHAT transcripts, converts between CHAT and JSON,
//! normalizes files to canonical form, runs CLAN-style analyses, and provides
//! a continuous watch mode for iterative editing.
//!
//! # Command overview
//!
//! All commands are implemented as clap derive subcommands. Run
//! `chatter --help` for the full listing; the highlights are:
//!
//! | Command            | Purpose                                                    |
//! |--------------------|------------------------------------------------------------|
//! | `validate`         | Parse and validate one file or an entire directory tree     |
//! | `normalize`        | Re-serialize a CHAT file in canonical formatting            |
//! | `to-json`          | Convert CHAT to JSON (conforming to the CHAT JSON Schema)  |
//! | `from-json`        | Convert JSON back to CHAT                                  |
//! | `to-xml`           | Export one CHAT file to TalkBank XML                       |
//! | `validate-utseg`   | Check utseg corpus output against gated invariants         |
//! | `show-alignment`   | Visualize main-tier / dependent-tier alignment              |
//! | `watch`            | Re-validate on every file save (uses `notify` file watcher)|
//! | `lint`             | Detect and optionally auto-fix common issues               |
//! | `clean`            | Show cleaned text for each word (debugging aid)            |
//! | `new-file`         | Scaffold a minimal valid CHAT file                          |
//! | `cache`            | Manage the on-disk validation cache (stats, clear)          |
//! | `schema`           | Print the CHAT JSON Schema or its canonical URL             |
//! | `debug <cmd>`      | Developer/debugging tools and corpus inspection            |
//! | `clan <cmd>`       | CLAN analysis/transform commands (freq, mlu, mlt, ...)     |
//!
//! # Dispatch architecture
//!
//! ```text
//! main()
//!  ├─ clap::Parser::parse()          ← cli::Cli, cli::Commands (clap derive)
//!  ├─ cli::init_tracing(verbose, ..) ← tracing-subscriber w/ env-filter
//!  └─ cli::run(cli)                  ← composition root, then family-based dispatch
//!       └─ commands::dispatch_command
//!            ├─ ValidationCommandService
//!            ├─ UtilityCommandService
//!            ├─ CacheCommandService
//!            └─ ClanCommandService   ← delegates to clan-core library
//! ```
//!
//! Argument definitions live in [`cli::args`](cli/args.rs) (the `Cli` struct
//! and `Commands` enum). The dispatch switch is in [`cli::run`](cli/run.rs).
//! Each command handler is a function in the [`commands`] module, which in turn
//! calls into the core library crates (`talkbank-transform`, `talkbank-model`,
//! `clan-core`).
//!
//! # TUI / interactive mode
//!
//! When stdout is a TTY (or `--tui-mode force` is passed), validation commands
//! render a ratatui-based terminal UI with live progress, color-coded
//! diagnostics, and a theme system (`--theme`). The TUI can be disabled with
//! `--tui-mode disable` for piping output to files or other tools. Tracing
//! output is automatically suppressed in TUI mode to avoid interleaving.
//!
//! # Parser selection
//!
//! The `--parser` flag on `validate` selects between the canonical tree-sitter
//! parser (`tree-sitter`, default) and the experimental direct parser
//! (removed). The tree-sitter parser is the sole parser.
//! `talkbank_model::ChatFile` AST.
//!
//! # Broken pipe handling
//!
//! `main()` installs a custom panic hook that silences broken-pipe panics
//! (common when output is piped to `head` or similar), and catches unwind
//! payloads so the process exits cleanly with code 0 rather than printing a
//! panic backtrace.
//!
//! # Module map
//!
//! ```text
//! src/
//! ├── main.rs          ← entry point (this file)
//! ├── cli/
//! │   ├── args.rs      ← Cli struct, Commands enum, CLAN subcommands (clap derive)
//! │   ├── run.rs       ← composition root (TUI detection, theme loading)
//! │   └── logging.rs   ← tracing-subscriber initialization
//! ├── commands/
//! │   ├── dispatch.rs  ← feature-oriented top-level command-family routing
//! │   ├── validate/    ← single-file and directory validation
//! │   ├── validate_parallel.rs ← parallel directory validation with progress
//! │   ├── json.rs      ← to-json / from-json conversion
//! │   ├── xml.rs       ← to-xml export
//! │   ├── normalize.rs ← canonical re-serialization
//! │   ├── watch.rs     ← file-watcher continuous validation
//! │   ├── lint.rs      ← auto-fixable issue detection
//! │   ├── clean.rs     ← cleaned-text debugging output
//! │   ├── debug.rs     ← debug-family commands
//! │   ├── find/        ← `chatter debug find`
//! │   ├── new_file.rs  ← CHAT file scaffolding
//! │   ├── schema.rs    ← JSON Schema output
//! │   ├── cache/       ← cache stats and clear subcommands
//! │   ├── alignment/   ← alignment visualization (show-alignment)
//! │   ├── validate_utseg.rs ← utseg invariant gate
//! │   └── clan.rs      ← CLAN command dispatch (delegates to clan-core)
//! ├── output.rs        ← formatting and rendering helpers
//! ├── progress.rs      ← progress bar utilities
//! └── ui/              ← TUI rendering (ratatui), themes, validation display
//! ```
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

mod cli;
mod commands;
pub mod output;
pub mod progress;
pub mod ui;

use clap::{CommandFactory, FromArgMatches};
use std::panic::{AssertUnwindSafe, PanicHookInfo, catch_unwind, resume_unwind};

/// Entry point for this binary target.
fn main() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if panic_info_is_broken_pipe(info) {
            return;
        }
        default_hook(info);
    }));

    let raw_args: Vec<String> = std::env::args().collect();
    let rewritten = rewrite_chatter_clan_args(&raw_args);

    // Build the clap Command, apply CLAN subcommand help grouping, then parse.
    // Clap 4 does not support grouping subcommands under different headings
    // via derive attributes, so we post-process the command tree.
    let cmd = cli::apply_clan_help_grouping(cli::Cli::command());
    let matches = cmd.get_matches_from(rewritten);
    // clap-validation invariant: `cmd.get_matches_from(...)` above
    // exits the process on any validation failure, so reaching
    // `from_arg_matches` guarantees the matches are well-formed.
    #[allow(clippy::expect_used)]
    let cli =
        cli::Cli::from_arg_matches(&matches).expect("clap should have validated all arguments");

    let result = catch_unwind(AssertUnwindSafe(|| {
        cli::run(cli);
    }));

    if let Err(payload) = result {
        if panic_is_broken_pipe(&payload) {
            std::process::exit(0);
        }
        resume_unwind(payload);
    }
}

/// Return `true` when a panic hook payload reports a broken pipe.
fn panic_info_is_broken_pipe(info: &PanicHookInfo<'_>) -> bool {
    if let Some(msg) = info.payload().downcast_ref::<String>() {
        return msg.contains("Broken pipe");
    }
    if let Some(msg) = info.payload().downcast_ref::<&str>() {
        return msg.contains("Broken pipe");
    }
    false
}

/// Return `true` when an unwind payload reports a broken pipe.
fn panic_is_broken_pipe(payload: &Box<dyn std::any::Any + Send>) -> bool {
    if let Some(msg) = payload.downcast_ref::<String>() {
        return msg.contains("Broken pipe");
    }
    if let Some(msg) = payload.downcast_ref::<&str>() {
        return msg.contains("Broken pipe");
    }
    false
}

/// Apply CLAN argument rewriting only when `clan` is the active subcommand.
///
/// Scans for `clan` in the argument list (skipping the binary name and any
/// global flags like `--verbose`). If found, rewrites CLAN-style flags in the
/// arguments after `clan` using [`clan_core::clan_args::rewrite_clan_args`].
fn rewrite_chatter_clan_args(args: &[String]) -> Vec<String> {
    let Some(clan_pos) = active_clan_subcommand_index(args) else {
        return args.to_vec();
    };

    let mut result = args[..=clan_pos].to_vec();
    let rewritten = clan_core::clan_args::rewrite_clan_args(&args[clan_pos + 1..]);
    result.extend(rewritten);
    result
}

fn active_clan_subcommand_index(args: &[String]) -> Option<usize> {
    let mut index = 1;

    while index < args.len() {
        let arg = args[index].as_str();
        match arg {
            "--log-format" | "--tui-mode" | "--theme" => {
                index += 2;
            }
            "--verbose" => {
                index += 1;
            }
            arg if arg.starts_with("--log-format=")
                || arg.starts_with("--tui-mode=")
                || arg.starts_with("--theme=") =>
            {
                index += 1;
            }
            arg if is_verbose_short_flag(arg) => {
                index += 1;
            }
            "--help" | "-h" | "--version" | "-V" => return None,
            "clan" => return Some(index),
            arg if arg.starts_with('-') => return None,
            _ => return None,
        }
    }

    None
}

fn is_verbose_short_flag(arg: &str) -> bool {
    arg.starts_with('-') && arg.len() > 1 && arg[1..].chars().all(|c| c == 'v')
}

#[cfg(test)]
mod tests {
    use super::rewrite_chatter_clan_args;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn rewrite_chatter_clan_args_rewrites_only_active_clan_suffix() {
        let input = args(&[
            "chatter",
            "--tui-mode=disable",
            "clan",
            "freq",
            "+t*CHI",
            "+scookie",
            "sample.cha",
        ]);

        let rewritten = rewrite_chatter_clan_args(&input);

        assert_eq!(
            rewritten,
            args(&[
                "chatter",
                "--tui-mode=disable",
                "clan",
                "freq",
                "--speaker",
                "CHI",
                "--include-word",
                "cookie",
                "sample.cha",
            ])
        );
    }

    #[test]
    fn rewrite_chatter_clan_args_does_not_rewrite_non_clan_arguments() {
        let input = args(&["chatter", "validate", "clan", "+t*CHI"]);

        let rewritten = rewrite_chatter_clan_args(&input);

        assert_eq!(rewritten, input);
    }
}
