//! Field-level validators for specific header payloads.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#ID_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Date_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Role_Field>

use crate::model::{BulletContent, IDHeader, ValidationTagged};
use crate::validation::speaker::has_invalid_speaker_chars;
use crate::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation, Span};

use super::metadata;
use super::participant::{is_allowed_participant_role, suggest_similar_role};

/// Validates required `@ID` slots and role/age formatting rules.
pub(super) fn check_id_header(id_header: &IDHeader, span: Span, errors: &impl ErrorSink) {
    if id_header.language.is_empty() {
        let mut err = ParseError::new(
            ErrorCode::EmptyIDLanguage,
            Severity::Error,
            SourceLocation::at_offset(span.start as usize),
            ErrorContext::new("", 0..0, "id_language"),
            "ID header language field cannot be empty",
        );
        err.location.span = span;
        errors.report(err);
    }

    if id_header.speaker.is_empty() {
        let mut err = ParseError::new(
            ErrorCode::EmptyIDSpeaker,
            Severity::Error,
            SourceLocation::at_offset(span.start as usize),
            ErrorContext::new(id_header.speaker.as_str(), 0..0, "id_speaker"),
            "ID header speaker field cannot be empty",
        );
        err.location.span = span;
        errors.report(err);
    }

    check_speaker_id(id_header.speaker.as_str(), "speaker_id", span, errors);

    if id_header.role.is_empty() {
        let mut err = ParseError::new(
            ErrorCode::EmptyIDRole,
            Severity::Error,
            SourceLocation::at_offset(span.start as usize),
            ErrorContext::new(id_header.role.as_str(), 0..0, "id_role"),
            "Empty role field in @ID header: the role (8th field) must not be blank",
        )
        .with_suggestion(
            "Set a valid role such as 'Target_Child', 'Mother', 'Investigator', or 'Participant'",
        );
        err.location.span = span;
        errors.report(err);
    } else if !is_allowed_participant_role(id_header.role.as_str()) {
        // E532: Invalid participant role
        let suggested = suggest_similar_role(id_header.role.as_str());
        let mut err = ParseError::new(
            ErrorCode::InvalidParticipantRole,
            Severity::Error,
            SourceLocation::at_offset(span.start as usize),
            ErrorContext::new(id_header.role.as_str(), 0..0, "id_role"),
            format!("Invalid participant role: '{}'", id_header.role.as_str()),
        )
        .with_suggestion(format!("Use a valid CHAT role such as: {}", suggested));
        err.location.span = span;
        errors.report(err);
    }

    if let Some(ref age) = id_header.age {
        if age.has_validation_issue() {
            let raw = age.as_str();
            let mut err = ParseError::new(
                ErrorCode::InvalidAgeFormat,
                Severity::Error,
                SourceLocation::at_offset(span.start as usize),
                ErrorContext::new(raw, 0..raw.len(), "id_age"),
                format!("Age should be in format years;months.days, got: {}", raw),
            );
            err.location.span = span;
            errors.report(err);
        } else if age.violates_depfile_pattern() {
            // Mirrors CLAN CHECK error 34 ("Illegal date representation"):
            // depfile.cut admits exactly yy; / yy;mm. / yy;mm.dd, so every
            // other shape — one-digit month, missing trailing period,
            // single-digit day, etc. — is a hard error. Java Chatter
            // matches by rejecting at parse time in the ANTLR grammar.
            let raw = age.as_str();
            let mut err = ParseError::new(
                ErrorCode::InvalidAgeFormat,
                Severity::Error,
                SourceLocation::at_offset(span.start as usize),
                ErrorContext::new(raw, 0..raw.len(), "id_age"),
                format!(
                    "Illegal date representation: '{}'. Legal forms per CLAN depfile.cut: YY; or YY;MM. or YY;MM.DD (two-digit MM and DD)",
                    raw
                ),
            )
            .with_suggestion(
                "Run \"chstring +q +1\" to zero-pad, or edit manually: 3;0 → 3;00., 2;6 → 2;06.",
            );
            err.location.span = span;
            errors.report(err);
        }
    }
}

/// Validates `@Date` payload and delegates format checks.
pub(super) fn check_date_header(date: &str, span: Span, errors: &impl ErrorSink) {
    if date.is_empty() {
        let mut err = ParseError::new(
            ErrorCode::EmptyDate,
            Severity::Error,
            SourceLocation::at_offset(span.start as usize),
            ErrorContext::new(date, 0..0, "date"),
            "@Date header should not be empty",
        );
        err.location.span = span;
        errors.report(err);
    } else {
        metadata::check_date_format(date, span, errors, ErrorCode::InvalidDateFormat);
    }
}

/// Validates `@Birth of <CODE>` payload against the depfile date
/// template. Same component-level rules as `@Date`, but emits E545
/// so users can distinguish which header was wrong.
pub(super) fn check_birth_date_header(date: &str, span: Span, errors: &impl ErrorSink) {
    if date.is_empty() {
        // Empty @Birth of is allowed (date-unknown), matching the
        // `Option<ChatDate>`-style semantics of participant fields.
        return;
    }
    metadata::check_date_format(date, span, errors, ErrorCode::InvalidBirthDateFormat);
}

/// Validates speaker-code constraints shared by participant fields.
pub(crate) fn check_speaker_id(
    speaker: &str,
    field_label: &str,
    span: Span,
    errors: &impl ErrorSink,
) {
    if speaker.len() > 7 {
        let mut err = ParseError::new(
            ErrorCode::InvalidSpeaker,
            Severity::Error,
            SourceLocation::at_offset(span.start as usize),
            ErrorContext::new(speaker, 0..speaker.len(), field_label),
            format!(
                "Speaker ID '{}' exceeds maximum length of 7 characters",
                speaker
            ),
        );
        err.location.span = span;
        errors.report(err);
    }

    if let Some(invalid_char) = has_invalid_speaker_chars(speaker) {
        let mut err = ParseError::new(
            ErrorCode::InvalidSpeaker,
            Severity::Error,
            SourceLocation::at_offset(span.start as usize),
            ErrorContext::new(speaker, 0..speaker.len(), field_label),
            format!(
                "Speaker ID '{}' contains invalid character '{}'. Speaker IDs cannot contain colon (:) or whitespace",
                speaker, invalid_char
            ),
        );
        err.location.span = span;
        errors.report(err);
    }
}

/// Intentionally no-op for `@Comment` and `@Warning` payload bodies.
pub(super) fn check_comment_warning(
    _content: &BulletContent,
    _span: Span,
    _errors: &impl ErrorSink,
) {
    // @Comment and @Warning headers are opaque CHAT data — not directives
    // for our validator.  Nothing to check here.
}
