//! Dispatch header validation to type-specific checkers.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Languages_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Participants_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#ID_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Date_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Options_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Media_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Number_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Recording_Quality_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Transcription_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Time_Duration_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Time_Start_Header>

use crate::model::Header;
use crate::validation::{Validate, ValidationContext};
use crate::{ErrorSink, Span};

use super::checkers::{
    check_birth_date_header, check_comment_warning, check_date_header, check_id_header,
};
use super::metadata::{check_time_duration_format, check_time_start_format};
use super::unknown::check_unknown_header;
use crate::model::ValidationTagged;

/// Validate a parsed `Header` and emit diagnostics through `errors`.
///
/// This is a lightweight dispatcher that routes each header variant to the
/// corresponding checker/trait implementation while preserving source span.
pub(crate) fn check_header(
    header: &Header,
    span: Span,
    _context: &ValidationContext,
    errors: &impl ErrorSink,
) {
    match header {
        Header::ID(id_header) => {
            check_id_header(id_header, span, errors);
            // E542: Unsupported sex value
            if let Some(ref sex) = id_header.sex
                && sex.has_validation_issue()
            {
                check_unsupported_sex(sex, span, errors);
            }
            // E546: Unsupported SES value
            if let Some(ref ses) = id_header.ses
                && ses.has_validation_issue()
            {
                check_unsupported_ses(ses, span, errors);
            }
        }
        Header::Participants { entries } => {
            entries.validate(_context, errors);
        }
        Header::Languages { codes } => {
            codes.validate(_context, errors);
        }
        Header::Date { date } => {
            check_date_header(date.as_str(), span, errors);
        }
        Header::Birth { date, .. } => {
            check_birth_date_header(date.as_str(), span, errors);
        }
        Header::Comment { content } => {
            check_comment_warning(content, span, errors);
        }
        Header::Unknown {
            text,
            parse_reason,
            suggested_fix,
        } => {
            check_unknown_header(
                text.as_str(),
                parse_reason.as_deref(),
                suggested_fix.as_deref(),
                span,
                errors,
            );
        }
        Header::Options { options } => {
            options.validate(_context, errors);
            check_unsupported_options(options, span, errors);
        }
        Header::Media(media_header) => {
            check_unsupported_media(media_header, span, errors);
        }
        Header::Number { number } => {
            check_unsupported_number(number, span, errors);
        }
        Header::RecordingQuality { quality } => {
            check_unsupported_recording_quality(quality, span, errors);
        }
        Header::Transcription { transcription } => {
            check_unsupported_transcription(transcription, span, errors);
        }
        Header::TimeDuration { duration }
            if duration.has_validation_issue() || duration.violates_depfile_pattern() =>
        {
            check_time_duration_format(duration.as_str(), span, errors);
        }
        Header::TimeStart { start }
            if start.has_validation_issue() || start.violates_depfile_pattern() =>
        {
            check_time_start_format(start.as_str(), span, errors);
        }
        Header::Types(_) => {
            // No validation — @Types fields have no fixed vocabulary.
        }
        _ => {}
    }
}

// ── Unsupported-value validators (E534–E539, E542) ────────────────────

/// E534: Flag each unsupported option flag in `@Options`.
fn check_unsupported_options(
    options: &crate::model::header::ChatOptionFlags,
    span: Span,
    errors: &impl ErrorSink,
) {
    for flag in options.iter() {
        if let crate::model::ChatOptionFlag::Unsupported(value) = flag {
            let mut err = crate::ParseError::new(
                crate::ErrorCode::UnsupportedOption,
                crate::Severity::Error,
                crate::SourceLocation::at_offset(span.start as usize),
                crate::ErrorContext::new(value, 0..value.len(), "option_name"),
                format!("Unsupported @Options value: '{}'", value),
            )
            .with_suggestion("Supported options (per CLAN depfile.cut): CA, NoAlign");
            err.location.span = span;
            errors.report(err);
        }
    }
}

/// E535/E536: Flag unsupported media type or status in `@Media`.
fn check_unsupported_media(
    media_header: &crate::model::MediaHeader,
    span: Span,
    errors: &impl ErrorSink,
) {
    if let crate::model::MediaType::Unsupported(value) = &media_header.media_type {
        let mut err = crate::ParseError::new(
            crate::ErrorCode::UnsupportedMediaType,
            crate::Severity::Error,
            crate::SourceLocation::at_offset(span.start as usize),
            crate::ErrorContext::new(value, 0..value.len(), "media_type"),
            format!("Unsupported @Media type: '{}'", value),
        )
        .with_suggestion("Supported media types: audio, video, missing");
        err.location.span = span;
        errors.report(err);
    }

    if let Some(crate::model::MediaStatus::Unsupported(value)) = &media_header.status {
        let mut err = crate::ParseError::new(
            crate::ErrorCode::UnsupportedMediaStatus,
            crate::Severity::Error,
            crate::SourceLocation::at_offset(span.start as usize),
            crate::ErrorContext::new(value, 0..value.len(), "media_status"),
            format!("Unsupported @Media status: '{}'", value),
        )
        .with_suggestion("Supported media status values: unlinked, missing, notrans");
        err.location.span = span;
        errors.report(err);
    }
}

/// E537: Flag unsupported `@Number` value.
fn check_unsupported_number(number: &crate::model::Number, span: Span, errors: &impl ErrorSink) {
    if let crate::model::Number::Unsupported(value) = number {
        let mut err = crate::ParseError::new(
            crate::ErrorCode::UnsupportedNumber,
            crate::Severity::Error,
            crate::SourceLocation::at_offset(span.start as usize),
            crate::ErrorContext::new(value, 0..value.len(), "number_option"),
            format!("Unsupported @Number value: '{}'", value),
        )
        .with_suggestion("Supported values: 1, 2, 3, 4, 5, more, audience");
        err.location.span = span;
        errors.report(err);
    }
}

/// E538: Flag unsupported `@Recording Quality` value.
fn check_unsupported_recording_quality(
    quality: &crate::model::RecordingQuality,
    span: Span,
    errors: &impl ErrorSink,
) {
    if let crate::model::RecordingQuality::Unsupported(value) = quality {
        let mut err = crate::ParseError::new(
            crate::ErrorCode::UnsupportedRecordingQuality,
            crate::Severity::Error,
            crate::SourceLocation::at_offset(span.start as usize),
            crate::ErrorContext::new(value, 0..value.len(), "recording_quality_option"),
            format!("Unsupported @Recording Quality value: '{}'", value),
        )
        .with_suggestion("Supported values: 1, 2, 3, 4, 5");
        err.location.span = span;
        errors.report(err);
    }
}

/// E539: Flag unsupported `@Transcription` value.
fn check_unsupported_transcription(
    transcription: &crate::model::Transcription,
    span: Span,
    errors: &impl ErrorSink,
) {
    if let crate::model::Transcription::Unsupported(value) = transcription {
        let mut err = crate::ParseError::new(
            crate::ErrorCode::UnsupportedTranscription,
            crate::Severity::Error,
            crate::SourceLocation::at_offset(span.start as usize),
            crate::ErrorContext::new(value, 0..value.len(), "transcription_option"),
            format!("Unsupported @Transcription value: '{}'", value),
        )
        .with_suggestion(
            "Supported values: eye_dialect, partial, full, detailed, coarse, checked, anonymized",
        );
        err.location.span = span;
        errors.report(err);
    }
}

/// E546: Flag unsupported SES value in `@ID`.
fn check_unsupported_ses(ses: &crate::model::SesValue, span: Span, errors: &impl ErrorSink) {
    if let crate::model::SesValue::Unsupported(value) = ses {
        let mut err = crate::ParseError::new(
            crate::ErrorCode::UnsupportedSesValue,
            crate::Severity::Error,
            crate::SourceLocation::at_offset(span.start as usize),
            crate::ErrorContext::new(value, 0..value.len(), "id_ses"),
            format!("Unsupported @ID SES value: '{}'", value),
        )
        .with_suggestion(
            "Supported values: UC, MC, WC, LI, White, Black, Asian, Latino, Native, Multiple, Unknown (or combined e.g. 'White UC')",
        );
        err.location.span = span;
        errors.report(err);
    }
}

/// E542: Flag unsupported sex value in `@ID`.
fn check_unsupported_sex(sex: &crate::model::Sex, span: Span, errors: &impl ErrorSink) {
    if let crate::model::Sex::Unsupported(value) = sex {
        let mut err = crate::ParseError::new(
            crate::ErrorCode::UnsupportedSex,
            crate::Severity::Error,
            crate::SourceLocation::at_offset(span.start as usize),
            crate::ErrorContext::new(value, 0..value.len(), "id_sex"),
            format!("Unsupported @ID sex value: '{}'", value),
        )
        .with_suggestion("Supported values: male, female");
        err.location.span = span;
        errors.report(err);
    }
}
