//! Test module for headers in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

use talkbank_model::model::{Header, LanguageCode};

/// Verifies `@UTF8` headers round-trip correctly.
#[test]
fn header_round_trip_utf8() {
    let header = Header::Utf8;
    let output = header.to_chat();
    assert_eq!(output, "@UTF8");
}

/// Verifies `@Begin` headers round-trip correctly.
#[test]
fn header_round_trip_begin() {
    let header = Header::Begin;
    let output = header.to_chat();
    assert_eq!(output, "@Begin");
}

/// Verifies `@Languages` headers round-trip correctly.
#[test]
fn header_round_trip_languages() {
    let header = Header::Languages {
        codes: vec![LanguageCode::new("eng")].into(),
    };
    let output = header.to_chat();
    assert_eq!(output, "@Languages:\teng");
}

/// Verifies `@Comment` headers round-trip correctly.
#[test]
fn header_round_trip_comment() {
    let header = Header::Comment {
        content: talkbank_model::model::BulletContent::from_text("test"),
    };
    let output = header.to_chat();
    assert_eq!(output, "@Comment:\ttest");
}
