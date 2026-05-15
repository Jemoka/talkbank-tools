//! Test module for helpers in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

use talkbank_model::model::ChatFile;
use talkbank_model::{ErrorCode, ErrorCollector, ParseError};

/// Returns whether error.
pub fn has_error(errors: &[ParseError], code: ErrorCode) -> bool {
    errors.iter().any(|e| e.code == code)
}

/// Returns error codes.
pub fn get_error_codes(errors: &[ParseError]) -> Vec<ErrorCode> {
    errors.iter().map(|e| e.code).collect()
}

/// Validates chat file.
pub fn validate_chat_file(chat_file: &ChatFile) -> Vec<ParseError> {
    let errors = ErrorCollector::new();
    chat_file.validate(&errors, None);
    errors.into_vec()
}

/// Validates chat file with alignment.
pub fn validate_chat_file_with_alignment(chat_file: &mut ChatFile) -> Vec<ParseError> {
    let errors = ErrorCollector::new();
    // TEMPORARY: skip %wor alignment - semantics still being worked out
    chat_file.validate_with_alignment(&errors, None);
    errors.into_vec()
}
