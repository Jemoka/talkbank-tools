#![warn(missing_docs)]
// Test code is exempt from this crate's `deny`-level panic lints —
// see `book/src/operations/panic-audit/clan-core.md`.
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
//! Reimplementation of CLAN analysis commands in Rust.
//!
//! CLAN (Computerized Language Analysis) is a toolkit by Brian MacWhinney containing
//! ~116 analysis commands implemented in ~215K lines of C/C++. This crate faithfully
//! reimplements the self-contained analysis commands in Rust, leveraging the existing
//! talkbank-tools parsing and model infrastructure.
//!
//! See the [CLAN manual](https://talkbank.org/0info/manuals/CLAN.html) for comprehensive
//! documentation of the original CLAN commands and their semantics.
//!
//! # Architecture
//!
//! The crate is split into four layers:
//!
//! - **[`framework`]** — Shared infrastructure replacing CLAN's CUTT framework:
//!   [`framework::AnalysisCommand`] trait, [`framework::FilterConfig`] for speaker/tier/word/gem filtering,
//!   [`framework::UtteranceRange`] and [`framework::DiscoveredChatFiles`] for reusable analysis input models,
//!   [`framework::AnalysisRunner`] for file loading and command dispatch, [`framework::AnalysisResult`]
//!   / [`framework::CommandOutput`] for text and structured output, and [`framework::TransformCommand`]
//!   for file-modifying commands.
//!
//! - **[`commands`]** — Individual analysis command implementations (FREQ, MLU, MLT, etc.),
//!   each implementing the [`framework::AnalysisCommand`] trait.
//!
//! - **[`transforms`]** — File-modifying commands (FLO, LOWCASE, CHSTRING, etc.),
//!   each implementing the [`framework::TransformCommand`] trait.
//!
//! - **[`converters`]** — Format conversion between CHAT and external formats
//!   (SRT, ELAN, Praat TextGrid, SALT, etc.).
//!
//! # Usage
//!
//! ```no_run
//! use std::path::Path;
//! use clan_core::framework::{AnalysisRunner, CommandOutput, OutputFormat};
//! use clan_core::commands::freq::FreqCommand;
//!
//! let runner = AnalysisRunner::new();
//! let command = FreqCommand::default();
//! let result = runner.run(&command, &[Path::new("file.cha").to_path_buf()]);
//! match result {
//!     Ok(output) => print!("{}", output.render(OutputFormat::Text)),
//!     Err(e) => eprintln!("Error: {e}"),
//! }
//! ```

pub mod clan_args;
pub mod commands;
pub mod converters;
pub mod database;
pub mod framework;
pub mod service;
pub mod service_types;
pub mod transforms;
