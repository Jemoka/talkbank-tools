//! Core [`AnalysisCommand`] trait and supporting types.
//!
//! Every CLAN analysis command implements this trait, which defines a
//! three-phase lifecycle: per-utterance accumulation, per-file finalization,
//! and cross-file output production. The [`AnalysisRunner`](super::AnalysisRunner)
//! drives this lifecycle, calling methods in order:
//!
//! 1. [`process_utterance()`](AnalysisCommand::process_utterance) for each
//!    filtered utterance
//! 2. [`end_file()`](AnalysisCommand::end_file) after all utterances in a file
//! 3. [`finalize()`](AnalysisCommand::finalize) to produce the typed output
//!
//! This replaces CUTT's `usage()`/`getflag()`/`init()`/`call()` contract from
//! the original CLAN C framework.

use std::path::Path;

use talkbank_model::ChatFile;
use talkbank_model::LineMap;
use talkbank_model::Utterance;

use super::output::CommandOutput;

/// Metadata about the file currently being processed.
///
/// Passed to `process_utterance` and `end_file` so commands can track
/// per-file state (e.g., for per-file frequency tables).
pub struct FileContext<'a> {
    /// Absolute or relative path to the CHAT file
    pub path: &'a Path,
    /// The parsed (and optionally validated) CHAT file
    pub chat_file: &'a ChatFile,
    /// Filename stem for display purposes
    pub filename: &'a str,
    /// Line map for O(log n) offset-to-line lookups. `None` for built files.
    pub line_map: Option<&'a LineMap>,
}

/// Trait that all CLAN analysis commands implement.
///
/// Replaces CUTT's `usage()`/`getflag()`/`init()`/`call()` contract.
/// Each command defines its own `Config` (parsed from CLI args) and
/// `State` (accumulated across utterances and files).
///
/// # Lifecycle
///
/// 1. Runner creates `State::default()`
/// 2. For each file:
///    a. For each utterance (after filtering): `process_utterance()`
///    b. After all utterances: `end_file()`
/// 3. After all files: `finalize()` produces the output
pub trait AnalysisCommand {
    /// Command-specific configuration parsed from CLI args.
    type Config;

    /// Accumulated state across utterances and files.
    /// Must implement `Default` for zero-initialization.
    type State: Default;

    /// Typed output produced by [`finalize()`](AnalysisCommand::finalize).
    ///
    /// Commands migrated to typed results use their own struct (e.g., `MluResult`).
    /// Commands not yet migrated use `AnalysisResult` as a bridge.
    type Output: CommandOutput;

    /// Process a single utterance within a file.
    ///
    /// Called once per utterance that passes the filter criteria.
    /// Accumulate counts, matches, or other data into `state`.
    fn process_utterance(
        &self,
        utterance: &Utterance,
        file_context: &FileContext<'_>,
        state: &mut Self::State,
    );

    /// Called after all utterances in a file are processed.
    ///
    /// Use this to finalize per-file statistics or emit per-file output.
    /// Default implementation does nothing.
    fn end_file(&self, _file_context: &FileContext<'_>, _state: &mut Self::State) {}

    /// Produce final output after all files are processed.
    ///
    /// Consumes the accumulated state and returns a typed result
    /// that can be rendered in multiple formats via [`CommandOutput`].
    fn finalize(&self, state: Self::State) -> Self::Output;
}
