//! Non-empty string type for correct-by-construction validation
//!
//! This type enforces at construction time that the string is non-empty,
//! eliminating the need for separate validation of "is this string empty?"
//!
//! **Note:** Deserialization allows empty strings - use the `Validate` trait
//! to check invariants on deserialized data. This allows validation errors
//! to go through our rich ErrorSink system with proper context.
//!
//! CHAT reference anchors (where this primitive is heavily used):
//! - [File Headers](https://talkbank.org/0info/manuals/CHAT.html#File_Headers)
//! - [Words](https://talkbank.org/0info/manuals/CHAT.html#Words)

use crate::validation::{Validate, ValidationContext};
use crate::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation, Span};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::Deref;
use talkbank_derive::{SemanticEq, SpanShift};

/// String wrapper with a non-empty construction invariant.
///
/// `new()` enforces non-empty input. Deserialization intentionally allows empty
/// payloads so invariant violations can flow through normal validation/error
/// reporting with source context.
///
/// Note: Whitespace-only strings are considered valid (they are non-empty).
///
/// # Construction
///
/// Use `NonEmptyString::new()` which returns `Option<NonEmptyString>`:
///
/// ```
/// use talkbank_model::model::NonEmptyString;
///
/// // Successful construction
/// if let Some(s) = NonEmptyString::new("hello") {
///     assert_eq!(s.as_str(), "hello");
/// }
///
/// // Whitespace is valid (non-empty)
/// assert!(NonEmptyString::new("   ").is_some());
///
/// // Only empty string is rejected
/// assert!(NonEmptyString::new("").is_none());
/// ```
///
/// # Validation
///
/// After deserializing data, use the `Validate` trait to check invariants:
///
/// ```ignore
/// use talkbank_model::validation::{Validate, ValidationContext};
/// use talkbank_model::ErrorCollector;
///
/// let errors = ErrorCollector::new();
/// let ctx = ValidationContext::default();
/// deserialized_data.validate(&ctx, &errors);
/// // Check errors.into_vec() for any NonEmptyString violations
/// ```
///
/// # Usage
///
/// `NonEmptyString` derefs to `&str` for convenient access:
///
/// ```
/// use talkbank_model::model::NonEmptyString;
///
/// if let Some(s) = NonEmptyString::new("hello") {
///     assert_eq!(s.as_str().get(0..2), Some("he"));  // str methods work via Deref
/// }
/// ```
///
/// References:
/// - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
/// - <https://talkbank.org/0info/manuals/CHAT.html#Word_Tier>
#[derive(
    Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift,
)]
#[serde(transparent)]
pub struct NonEmptyString(smol_str::SmolStr);

impl NonEmptyString {
    /// Create a new `NonEmptyString` if the input is non-empty.
    ///
    /// Returns `None` only for truly empty input (`""`).
    /// Whitespace-only strings are accepted because this type enforces lexical
    /// non-emptiness, while higher-level semantic rules decide whether blanks are meaningful.
    ///
    /// # Examples
    ///
    /// ```
    /// use talkbank_model::model::NonEmptyString;
    ///
    /// assert!(NonEmptyString::new("hello").is_some());
    /// assert!(NonEmptyString::new("   ").is_some());  // whitespace is valid
    /// assert!(NonEmptyString::new("").is_none());
    /// ```
    pub fn new(s: impl AsRef<str>) -> Option<Self> {
        let s = s.as_ref();
        if s.is_empty() {
            None
        } else {
            Some(Self(smol_str::SmolStr::from(s)))
        }
    }

    /// Construct without checking emptiness.
    ///
    /// # Safety
    ///
    /// Caller must guarantee non-empty input.
    ///
    /// # Panics
    ///
    /// In debug builds, panics if the string is empty.
    pub fn new_unchecked(s: impl AsRef<str>) -> Self {
        let s = s.as_ref();
        debug_assert!(
            !s.is_empty(),
            "NonEmptyString::new_unchecked called with empty string"
        );
        Self(smol_str::SmolStr::from(s))
    }

    /// Borrow the inner string.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consume and return an owned `String`.
    pub fn into_inner(self) -> String {
        self.0.to_string()
    }
}

impl Deref for NonEmptyString {
    type Target = str;

    /// Borrows the stored text as `&str`.
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<str> for NonEmptyString {
    /// Borrows the stored text as `&str`.
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for NonEmptyString {
    /// Displays the stored string value.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Validate for NonEmptyString {
    /// Reports an error when deserialized content violates the non-empty invariant.
    fn validate(&self, context: &ValidationContext, errors: &impl ErrorSink) {
        if !self.0.is_empty() {
            return;
        }

        let span = match context.field_span {
            Some(span) => span,
            None => Span::from_usize(0, 0),
        };
        let location = match context.field_span {
            Some(span) => SourceLocation::new(span),
            None => SourceLocation::at_offset(0),
        };
        // DEFAULT: Missing field text is reported as empty to match the offending value.
        let source_text = context.field_text.clone().unwrap_or_default();
        // DEFAULT: When no label is provided, describe the missing value as a generic string.
        let label = context.field_label.unwrap_or("string");
        // DEFAULT: Empty strings map to the canonical EmptyString error code.
        let code = context.field_error_code.unwrap_or(ErrorCode::EmptyString);

        errors.report(
            ParseError::new(
                code,
                Severity::Error,
                location,
                ErrorContext::new(source_text, span, label),
                format!("{} cannot be empty", label),
            )
            .with_suggestion("Provide a non-empty value"),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Non-empty inputs construct successfully.
    #[test]
    fn test_construction() {
        assert!(NonEmptyString::new("hello").is_some());
        assert!(NonEmptyString::new("a").is_some());
        assert!(NonEmptyString::new(" x ").is_some());
    }

    /// Empty input is rejected by `new()`.
    #[test]
    fn test_empty_rejected() {
        assert!(NonEmptyString::new("").is_none());
    }

    /// Whitespace-only input is still non-empty and therefore valid.
    #[test]
    fn test_whitespace_allowed() {
        // Whitespace-only strings are valid (not empty)
        assert!(NonEmptyString::new("   ").is_some());
        assert!(NonEmptyString::new("\t\n").is_some());
    }

    /// `Deref<str>` behavior is available.
    #[test]
    fn test_deref() -> Result<(), String> {
        let s = NonEmptyString::new("hello")
            .ok_or_else(|| "Expected NonEmptyString for 'hello'".to_string())?;
        assert_eq!(s.as_str().get(0..2), Some("he"));
        assert_eq!(s.len(), 5);
        Ok(())
    }

    /// `Display` prints inner contents.
    #[test]
    fn test_display() -> Result<(), String> {
        let s = NonEmptyString::new("hello")
            .ok_or_else(|| "Expected NonEmptyString for 'hello'".to_string())?;
        assert_eq!(format!("{}", s), "hello");
        Ok(())
    }

    /// Serde roundtrip preserves values.
    #[test]
    fn test_serde_roundtrip() -> Result<(), String> {
        let s = NonEmptyString::new("hello")
            .ok_or_else(|| "Expected NonEmptyString for 'hello'".to_string())?;
        let json = serde_json::to_string(&s)
            .map_err(|err| format!("Failed to serialize NonEmptyString: {err}"))?;
        assert_eq!(json, "\"hello\"");

        let deserialized: NonEmptyString = serde_json::from_str(&json)
            .map_err(|err| format!("Failed to deserialize NonEmptyString: {err}"))?;
        assert_eq!(deserialized, s);
        Ok(())
    }

    /// Empty payload may deserialize and is caught by validation later.
    #[test]
    fn test_deserialize_empty_allowed_but_invalid() -> Result<(), String> {
        let s: NonEmptyString = serde_json::from_str(r#""""#)
            .map_err(|err| format!("Failed to deserialize empty NonEmptyString: {err}"))?;
        assert!(s.as_str().is_empty());
        Ok(())
    }

    /// Whitespace payload deserializes as valid non-empty content.
    #[test]
    fn test_deserialize_whitespace_valid() -> Result<(), String> {
        let s: NonEmptyString = serde_json::from_str(r#""   ""#)
            .map_err(|err| format!("Failed to deserialize whitespace NonEmptyString: {err}"))?;
        assert!(!s.as_str().is_empty());
        assert_eq!(s.as_str(), "   ");
        Ok(())
    }

    /// Validation reports `EmptyString` for empty payloads.
    #[test]
    fn test_validate_empty_reports_error() -> Result<(), String> {
        use crate::ErrorCollector;
        use crate::validation::ValidationContext;

        let s: NonEmptyString = serde_json::from_str(r#""""#)
            .map_err(|err| format!("Failed to deserialize empty NonEmptyString: {err}"))?;
        let errors = ErrorCollector::new();
        let ctx = ValidationContext::default();
        s.validate(&ctx, &errors);

        let error_vec = errors.into_vec();
        assert!(
            error_vec
                .iter()
                .any(|e| e.code == crate::ErrorCode::EmptyString),
            "Expected E003 for empty NonEmptyString, got: {:#?}",
            error_vec
        );
        Ok(())
    }
}
