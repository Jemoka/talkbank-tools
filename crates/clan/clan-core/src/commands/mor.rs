//! MOR — Morphological Analysis.
//!
//! MOR adds `%mor` dependent tiers to CHAT files by performing morphological
//! analysis of main-tier words against a language-specific lexicon and rule set.
//!
//! # Implementation Status
//!
//! **Deliberately not implemented.** MOR requires language-specific trie-based
//! lexicon databases (~11,000 lines of C) and five rule engines (A-rules,
//! C-rules, D-rules, PREPOST rules, allomorph rules). The CHAT grammar and
//! data model have moved to UD-style morphological representation, making a
//! faithful port of the legacy CLAN MOR format impractical.
//!
//! For morphological analysis, use batchalign's neural morphosyntax pipeline,
//! which supports more languages with higher accuracy.
//!
//! # Differences from CLAN
//!
//! This command is not a reimplementation — it is deliberately absent.
//! Invoking it produces a clear error message directing users to batchalign.

use crate::framework::TransformError;

/// Returns an error indicating MOR is deliberately not implemented.
pub fn run_mor() -> Result<(), TransformError> {
    Err(TransformError::Transform(
        "MOR is deliberately not implemented. The CHAT grammar uses UD-style \
         morphological representation incompatible with legacy CLAN MOR format. \
         Use batchalign for neural morphosyntactic analysis."
            .to_owned(),
    ))
}
