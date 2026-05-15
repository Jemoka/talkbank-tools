//! Hosts the FA-specific raw-response types (`FaRawToken`,
//! `FaIndexedTiming`, `FaRawResponse`) used by Whisper-style alignment.
//! Everything else re-exports `talkbank_transform::morphosyntax`; new
//! consumers should import UD/mapping items from `talkbank_transform`
//! directly.

pub mod mapping;
mod types;

pub use talkbank_transform::morphosyntax::*;
pub use types::{FaIndexedTiming, FaRawResponse, FaRawToken};
