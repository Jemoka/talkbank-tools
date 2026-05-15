//! POSTLIST — POST Database Listing.
//!
//! Lists contents of a POST binary database file (tags, matrix entries,
//! rules, word frequencies).
//!
//! # Implementation Status
//!
//! **Deliberately not implemented.** POSTLIST operates on POST's proprietary
//! binary database format, which is not being ported.
//!
//! # Differences from CLAN
//!
//! This command is not a reimplementation — it is deliberately absent.
//! POST's binary database format is not supported.

use crate::framework::TransformError;

/// Returns an error indicating POSTLIST is deliberately not implemented.
pub fn run_postlist() -> Result<(), TransformError> {
    Err(TransformError::Transform(
        "POSTLIST is deliberately not implemented. POST's binary database \
         format is not being ported."
            .to_owned(),
    ))
}
