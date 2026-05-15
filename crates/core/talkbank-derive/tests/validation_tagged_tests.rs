// Integration tests for the ValidationTagged derive macro.
//
// The macro generates `impl crate::model::ValidationTagged` and references
// `crate::model::ValidationTag`, so we must bring `talkbank_model::model`
// into the crate root.

use talkbank_derive::ValidationTagged as DeriveValidationTagged;
use talkbank_model::model;
use talkbank_model::model::{ValidationTag, ValidationTagged};

// ---------------------------------------------------------------------------
// Test types
// ---------------------------------------------------------------------------

/// Enum with explicit annotations on every variant.
#[derive(Debug, Clone, DeriveValidationTagged)]
enum ExplicitTags {
    #[validation_tag(clean)]
    Good,
    #[validation_tag(warning)]
    Degraded,
    #[validation_tag(error)]
    Broken,
}

/// Enum using naming conventions (suffix-based resolution).
#[derive(Debug, Clone, DeriveValidationTagged)]
enum ConventionBased {
    Clean,
    ParseError,
    DeferredWarning,
    Normal,
}

/// Enum testing the Unsupported convention.
#[derive(Debug, Clone, DeriveValidationTagged)]
enum UnsupportedVariants {
    Supported,
    Unsupported,
    FormatUnsupported,
}

// ---------------------------------------------------------------------------
// Task 4: ValidationTagged tests (5 tests)
// ---------------------------------------------------------------------------

#[test]
fn explicit_annotations() {
    assert_eq!(ExplicitTags::Good.validation_tag(), ValidationTag::Clean);
    assert_eq!(
        ExplicitTags::Degraded.validation_tag(),
        ValidationTag::Warning
    );
    assert_eq!(ExplicitTags::Broken.validation_tag(), ValidationTag::Error);
}

#[test]
fn error_suffix_convention() {
    assert_eq!(
        ConventionBased::ParseError.validation_tag(),
        ValidationTag::Error
    );
}

#[test]
fn warning_suffix_convention() {
    assert_eq!(
        ConventionBased::DeferredWarning.validation_tag(),
        ValidationTag::Warning
    );
}

#[test]
fn unsupported_convention_maps_to_warning() {
    // Both the exact name "Unsupported" and the suffix "*Unsupported" map to Warning.
    assert_eq!(
        UnsupportedVariants::Unsupported.validation_tag(),
        ValidationTag::Warning
    );
    assert_eq!(
        UnsupportedVariants::FormatUnsupported.validation_tag(),
        ValidationTag::Warning
    );
    // Non-matching name defaults to Clean.
    assert_eq!(
        UnsupportedVariants::Supported.validation_tag(),
        ValidationTag::Clean
    );
}

#[test]
fn helper_methods() {
    assert!(ExplicitTags::Broken.is_validation_error());
    assert!(!ExplicitTags::Broken.is_validation_warning());
    assert!(ExplicitTags::Broken.has_validation_issue());

    assert!(ExplicitTags::Degraded.is_validation_warning());
    assert!(!ExplicitTags::Degraded.is_validation_error());
    assert!(ExplicitTags::Degraded.has_validation_issue());

    assert!(!ExplicitTags::Good.is_validation_error());
    assert!(!ExplicitTags::Good.is_validation_warning());
    assert!(!ExplicitTags::Good.has_validation_issue());
}
