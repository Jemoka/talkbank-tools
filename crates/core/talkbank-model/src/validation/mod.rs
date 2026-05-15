//! Validation module for CHAT data model
//!
//! Validation is performed via **methods on model types**:
//!
//! ```ignore
//! use talkbank_model::{ChatFile, ErrorCollector};
//!
//! let errors = ErrorCollector::new();
//! chat_file.validate(&errors);
//! let error_vec = errors.into_vec();
//! ```
//!
//! ## Public API
//!
//! - **`ChatFile::validate()`** - Validate entire file with streaming errors
//! - **`ChatFile::validate_with_alignment()`** - Validate with tier alignment
//! - **`Validate` trait** - Implemented by all model types for uniform validation
//! - **`ValidationContext`** - File-level context passed down validation hierarchy
//!
//! ## Location Tracking Limitation
//!
//! Currently, validation errors use placeholder source locations `(1, 1)` because
//! the domain model (Word, MainTier, etc.) does not carry source location information.
//! This is a deliberate design choice for the current phase:
//! - Domain model remains simple and focused on semantics
//! - Location tracking will be added in Phase 4 (Validation Engine) when we design
//!   a comprehensive approach that integrates with parsing and editor integration
//!
//! For now, validation errors still provide useful context through ErrorContext
//! (the actual text, column ranges, and expectations).
//!
//! ## Design Principles
//!
//! - Validation is separate from parsing (parse, do not validate)
//! - Add new errors using TDD with focused tests
//! - Stream diagnostics via `ErrorSink` without early returns
//!
//! ## Validation Diagnostics Rules
//!
//! - Do not rely on fabricated values from parser recovery when validating semantics
//! - If source context is unknown, represent that explicitly instead of fake sentinel spans/content
//! - Keep validation errors structured and miette-friendly for consistent source-located rendering
//! - Alignment-related validation must honor parse-taint and skip mismatches for tainted domains
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

// Module declarations
#[cfg(feature = "async")]
pub mod async_runtime;
mod bullet;
mod chat_file;
mod config;
mod context;
#[doc(hidden)]
pub mod cross_utterance;
pub(crate) mod header;
#[doc(hidden)]
pub mod main_tier;
pub(crate) mod retrace;
mod speaker;
mod state;
pub(crate) mod temporal;
mod r#trait;
mod unparsed_tier;
pub(crate) mod utterance;
pub(crate) mod word;

// Re-export public API
pub use config::ValidationConfig;
pub use context::{SharedValidationData, ValidationContext, language_allows_numbers};
pub use state::{NotValidated, Validated, ValidationState};
pub use r#trait::Validate;

// Re-export async helpers when feature is enabled
#[cfg(feature = "async")]
pub use crate::AsyncChannelErrorSink;
#[cfg(feature = "async")]
pub use async_runtime::{AsyncValidationError, validate_async, validate_with_config_async};
pub use word::language::LanguageResolution;
pub use word::{LanguageResolutionOutcome, resolve_word_language};

// Public bullet validation function
pub(crate) use bullet::check_bullet;
pub use bullet::check_bullet_monotonicity;
pub(crate) use speaker::has_invalid_speaker_chars;
pub(crate) use unparsed_tier::check_user_defined_tier_content;

// Re-export tests if they exist
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ErrorCollector;
    use crate::model::{Annotated, ContentAnnotation, Word};
    use crate::validation::Validate;

    /// Verifies word validation with unknown annotation.
    #[test]
    fn test_word_validation_with_unknown_annotation() {
        // Build word programmatically (not by parsing)
        let word = Annotated::new(Word::new_unchecked("hello [::: stuff]", "hello"))
            .with_scoped_annotations(vec![ContentAnnotation::Unknown(
                crate::model::ScopedUnknown {
                    marker: ":::".into(),
                    text: "stuff".into(),
                },
            )]);

        // Validate it (no language context needed for this test)
        let errors = ErrorCollector::new();
        let context = ValidationContext::new();
        word.validate(&context, &errors);
        let error_vec = errors.into_vec();

        // Should have E207 error for unknown annotation
        use crate::ErrorCode;
        assert_eq!(error_vec.len(), 1);
        assert_eq!(error_vec[0].code, ErrorCode::UnknownAnnotation);
        // Note: The actual error message may vary, just check that it contains the marker
        assert!(
            error_vec[0].message.contains(":::"),
            "Error message should mention the unknown marker ':::'. Got: {}",
            error_vec[0].message
        );
    }

    /// Verifies word validation no errors.
    #[test]
    fn test_word_validation_no_errors() {
        // Build valid word programmatically
        // Note: Don't wrap in Annotated unless there are actual annotations,
        // as E214 fires for empty scoped annotations in Annotated wrappers.
        let word = Word::new_unchecked("hello", "hello");

        let errors = ErrorCollector::new();
        let context = ValidationContext::new();
        word.validate(&context, &errors);
        let error_vec = errors.into_vec();

        // Should have no errors
        assert_eq!(error_vec.len(), 0);
    }
}
