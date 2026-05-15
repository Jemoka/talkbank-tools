//! MEGRASP — Grammar Relation Parsing.
//!
//! MEGRASP adds `%gra` dependent tiers to CHAT files by performing MaxEnt
//! beam-search dependency parsing on `%mor` tiers.
//!
//! # Implementation Status
//!
//! **Deliberately not implemented.** MEGRASP requires trained MaxEnt model
//! weights and operates on the legacy CLAN `%mor` format.
//!
//! For dependency parsing, use batchalign's neural pipeline.
//!
//! # Differences from CLAN
//!
//! This command is not a reimplementation — it is deliberately absent.
//! Invoking it produces a clear error message directing users to batchalign.

use crate::framework::TransformError;

/// Returns an error indicating MEGRASP is deliberately not implemented.
pub fn run_megrasp() -> Result<(), TransformError> {
    Err(TransformError::Transform(
        "MEGRASP is deliberately not implemented. It requires trained MaxEnt \
         model weights for the legacy CLAN %mor format. \
         Use batchalign for neural dependency parsing."
            .to_owned(),
    ))
}
