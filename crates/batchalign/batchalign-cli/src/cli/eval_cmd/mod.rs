//! `batchalign3 eval ...` — evaluation subcommands.
//!
//! Currently exposes a single variant, `eval l2-morphotag`, which ports
//! the Python analyzer at `scripts/l2-eval/analyze.py` to a typed Rust
//! subcommand. Future evaluation tools (alignment quality, WER by
//! corpus, morphotag for non-L2) will land under this same namespace.

pub mod l2_morphotag;

use crate::cli::args::{EvalAction, EvalArgs};
use crate::cli::error::CliError;

/// Dispatch an `eval ...` invocation to the right sub-handler.
pub fn run(args: &EvalArgs) -> Result<(), CliError> {
    match &args.action {
        EvalAction::L2Morphotag(a) => l2_morphotag::run(a),
    }
}
