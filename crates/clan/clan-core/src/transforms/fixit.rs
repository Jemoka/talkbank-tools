//! FIXIT -- normalize CHAT file formatting.
//!
//! Reimplements CLAN's `fixit` command, which normalizes CHAT file formatting
//! by re-serializing through the parser. This fixes common issues like
//! inconsistent spacing, missing terminators, and malformed tier prefixes.
//!
//! Since our parse → serialize pipeline already produces canonically formatted
//! output, FIXIT is effectively a roundtrip: parse the file, then re-serialize
//! the resulting AST. Any formatting inconsistencies are corrected during
//! serialization.
//!
//! # Differences from CLAN
//!
//! - Uses full AST roundtrip rather than heuristic text manipulation.
//! - Files that fail to parse will produce an error rather than
//!   attempting partial text-level fixes.
//! - Output is the canonical CHAT serialization, which may reorder
//!   some whitespace or normalize header formatting.

use talkbank_model::ChatFile;

use crate::framework::{TransformCommand, TransformError};

/// FIXIT transform: normalize CHAT formatting via parse → serialize roundtrip.
pub struct FixitCommand;

impl TransformCommand for FixitCommand {
    type Config = ();

    /// The transformation is a no-op on the AST — the normalization happens
    /// during the standard parse → serialize pipeline.
    fn transform(&self, _file: &mut ChatFile) -> Result<(), TransformError> {
        Ok(())
    }
}
