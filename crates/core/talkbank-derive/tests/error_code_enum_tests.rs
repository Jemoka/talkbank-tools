// Integration tests for the error_code_enum attribute macro.
//
// The macro generates serde/schemars derives, Display, as_str(), new(), and
// documentation_url(). We test the generated API surface here.

use talkbank_derive::error_code_enum;

#[error_code_enum]
#[derive(PartialOrd, Ord)]
enum TestErrorCode {
    #[code("E001")]
    InternalError,
    #[code("E101")]
    InvalidFormat,
    #[code("E201")]
    MissingHeader,
    #[code("E999")]
    UnknownError,
}

// ---------------------------------------------------------------------------
// Task 5: error_code_enum tests (4 tests)
// ---------------------------------------------------------------------------

#[test]
fn as_str_returns_code_string() {
    assert_eq!(TestErrorCode::InternalError.as_str(), "E001");
    assert_eq!(TestErrorCode::InvalidFormat.as_str(), "E101");
    assert_eq!(TestErrorCode::MissingHeader.as_str(), "E201");
    assert_eq!(TestErrorCode::UnknownError.as_str(), "E999");
}

#[test]
fn new_parses_known_codes() {
    assert_eq!(TestErrorCode::new("E001"), TestErrorCode::InternalError);
    assert_eq!(TestErrorCode::new("E101"), TestErrorCode::InvalidFormat);
    assert_eq!(TestErrorCode::new("E201"), TestErrorCode::MissingHeader);
}

#[test]
fn new_returns_unknown_for_unrecognized_code() {
    assert_eq!(TestErrorCode::new("E000"), TestErrorCode::UnknownError);
    assert_eq!(TestErrorCode::new("ZZZZ"), TestErrorCode::UnknownError);
    assert_eq!(TestErrorCode::new(""), TestErrorCode::UnknownError);
}

#[test]
fn display_shows_code() {
    assert_eq!(format!("{}", TestErrorCode::InternalError), "E001");
    assert_eq!(format!("{}", TestErrorCode::MissingHeader), "E201");
    assert_eq!(format!("{}", TestErrorCode::UnknownError), "E999");
}
