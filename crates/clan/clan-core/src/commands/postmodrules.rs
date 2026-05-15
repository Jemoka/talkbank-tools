//! POSTMODRULES — POST Rule Modification.
//!
//! Modifies disambiguation rules in a POST binary database file.
//!
//! # Implementation Status
//!
//! **Deliberately not implemented.** POSTMODRULES operates on POST's
//! proprietary binary database format, which is not being ported.
//!
//! # Differences from CLAN
//!
//! This command is not a reimplementation — it is deliberately absent.
//! POST's binary database format is not supported.

use crate::framework::TransformError;

/// Returns an error indicating POSTMODRULES is deliberately not implemented.
pub fn run_postmodrules() -> Result<(), TransformError> {
    Err(TransformError::Transform(
        "POSTMODRULES is deliberately not implemented. POST's binary database \
         format is not being ported."
            .to_owned(),
    ))
}
