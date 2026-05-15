//! Validation-tag tests for language metadata enums.
//!
//! This suite locks down how unresolved language states surface through the
//! `ValidationTagged` derive so downstream validators can treat metadata health
//! consistently.

use super::{LanguageSource, WordLanguages};
use crate::model::{LanguageCode, ValidationTag, ValidationTagged};

/// Short helper for constructing one `LanguageCode`.
fn lc(code: &str) -> LanguageCode {
    LanguageCode::new(code)
}

/// Confirms `WordLanguages` validation tags match semantic health.
#[test]
fn word_languages_validation_tags() {
    assert_eq!(
        WordLanguages::Single(lc("eng")).validation_tag(),
        ValidationTag::Clean
    );
    assert_eq!(
        WordLanguages::Multiple(vec![lc("eng"), lc("spa")]).validation_tag(),
        ValidationTag::Clean
    );
    assert_eq!(
        WordLanguages::Ambiguous(vec![lc("eng"), lc("spa")]).validation_tag(),
        ValidationTag::Clean
    );
    assert_eq!(
        WordLanguages::Unresolved.validation_tag(),
        ValidationTag::Error
    );
    assert!(WordLanguages::Unresolved.is_validation_error());
}

/// Confirms `LanguageSource` validation tags match semantic health.
#[test]
fn language_source_validation_tags() {
    assert_eq!(
        LanguageSource::Default.validation_tag(),
        ValidationTag::Clean
    );
    assert_eq!(
        LanguageSource::TierScoped.validation_tag(),
        ValidationTag::Clean
    );
    assert_eq!(
        LanguageSource::WordExplicit.validation_tag(),
        ValidationTag::Clean
    );
    assert_eq!(
        LanguageSource::WordShortcut.validation_tag(),
        ValidationTag::Clean
    );
    assert_eq!(
        LanguageSource::Unresolved.validation_tag(),
        ValidationTag::Error
    );
}
