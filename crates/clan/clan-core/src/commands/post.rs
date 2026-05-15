//! POST — Part-of-Speech Disambiguation.
//!
//! POST disambiguates ambiguous `%mor` tiers (with `^`-separated alternatives)
//! using HMM/Brill tagging with trained model files.
//!
//! # Implementation Status
//!
//! **Deliberately not implemented.** POST requires trained binary database files
//! and reads ambiguous `%mor` tiers with `^` separators — a format not supported
//! by the current CHAT grammar or data model.
//!
//! For POS disambiguation, use batchalign's neural pipeline.
//!
//! # Differences from CLAN
//!
//! This command is not a reimplementation — it is deliberately absent.
//! Invoking it produces a clear error message directing users to batchalign.

use crate::framework::TransformError;

/// Returns an error indicating POST is deliberately not implemented.
pub fn run_post() -> Result<(), TransformError> {
    Err(TransformError::Transform(
        "POST is deliberately not implemented. The CHAT grammar does not support \
         the ^-separated ambiguity format that POST reads. \
         Use batchalign for neural POS disambiguation."
            .to_owned(),
    ))
}
