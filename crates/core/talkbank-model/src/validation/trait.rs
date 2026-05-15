//! Validate trait for model types
//!
//! This trait enables each data model type to validate itself, streaming errors
//! via ErrorSink for real-time error reporting.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use crate::ErrorSink;
use crate::validation::context::ValidationContext;

/// Validation trait - all types implement this to validate themselves
///
/// Errors are streamed via ErrorSink, enabling real-time error reporting.
/// This trait enforces consistent validation across all model types.
///
/// # Example
///
/// ```rust
/// use talkbank_model::validation::{Validate, ValidationContext};
/// use talkbank_model::ErrorCollector;
/// use talkbank_model::model::SpeakerCode;
///
/// let speaker = SpeakerCode::new("CHI".to_string());
/// let ctx = ValidationContext::default();
/// let errors = ErrorCollector::new();
///
/// speaker.validate(&ctx, &errors);
///
/// assert!(errors.into_vec().is_empty());
/// ```
pub trait Validate {
    /// Validate this value, streaming errors via ErrorSink
    ///
    /// # Arguments
    /// * `context` - Validation context (languages, participants, etc.)
    /// * `errors` - Error sink for streaming validation errors
    fn validate(&self, context: &ValidationContext, errors: &impl ErrorSink);
}
