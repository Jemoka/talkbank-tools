//! ChatFile validation entry points.
//!
//! Validation is performed via methods on `ChatFile`:
//!
//! ```ignore
//! use talkbank_model::{ChatFile, ErrorCollector};
//!
//! let errors = ErrorCollector::new();
//! chat_file.validate(&errors);
//! let error_vec = errors.into_vec();
//! ```
//!
//! See `ChatFile::validate()` and `ChatFile::validate_with_alignment()` for details.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
