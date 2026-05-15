//! POSTTRAIN — POST Model Training.
//!
//! Trains a POST database from manually tagged CHAT files.
//!
//! # Implementation Status
//!
//! **Deliberately not implemented.** POSTTRAIN produces POST's proprietary
//! binary database format, which is not being ported.
//!
//! # Differences from CLAN
//!
//! This command is not a reimplementation — it is deliberately absent.
//! POST's binary database format is not supported.

use crate::framework::TransformError;

/// Returns an error indicating POSTTRAIN is deliberately not implemented.
pub fn run_posttrain() -> Result<(), TransformError> {
    Err(TransformError::Transform(
        "POSTTRAIN is deliberately not implemented. POST's binary database \
         format is not being ported."
            .to_owned(),
    ))
}
