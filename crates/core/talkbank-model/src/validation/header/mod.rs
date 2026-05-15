//! Header-validation orchestration and rule entrypoints.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Languages_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Participants_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#ID_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Date_Header>
//!
//! The public submodules expose focused rule families (structure, participant,
//! metadata), while `check_header` provides the single dispatch point used by
//! line/file validators.

pub mod metadata;
pub mod participant;
pub mod structure;

mod checkers;
mod unknown;
mod validate;

pub(crate) use checkers::check_speaker_id;
pub(crate) use validate::check_header;

#[cfg(test)]
mod tests {
    use super::check_header;
    use crate::model::{Header, ParticipantEntry, ParticipantRole, SpeakerCode, WarningText};
    use crate::validation::ValidationContext;
    use crate::{ErrorCode, ErrorCollector, Severity, Span};

    /// Speaker IDs containing `:` are rejected as invalid.
    ///
    /// Colon is reserved by CHAT for tier-prefix syntax, so allowing it would make
    /// speaker parsing ambiguous.
    #[test]
    fn test_speaker_id_with_colon_invalid() {
        let entry = ParticipantEntry {
            speaker_code: SpeakerCode::new("CH:I"),
            name: None,
            role: ParticipantRole::new("Child"),
        };

        let header = Header::Participants {
            entries: vec![entry].into(),
        };

        let errors = ErrorCollector::new();
        let ctx = ValidationContext::default();
        check_header(&header, Span::DUMMY, &ctx, &errors);
        let error_vec = errors.into_vec();

        // Should catch the ':' character (reserved as delimiter)
        assert!(
            !error_vec.is_empty(),
            "Should have errors for speaker ID with ':'"
        );
        assert!(
            error_vec
                .iter()
                .any(|e| e.message.contains("invalid character") && e.message.contains("colon"))
        );
    }

    /// Standard uppercase speaker IDs pass character validation.
    ///
    /// The assertion specifically checks that no invalid-character diagnostics are emitted.
    #[test]
    fn test_speaker_id_valid() {
        let entry = ParticipantEntry {
            speaker_code: SpeakerCode::new("CHI"),
            name: None,
            role: ParticipantRole::new("Child"),
        };

        let header = Header::Participants {
            entries: vec![entry].into(),
        };

        let errors = ErrorCollector::new();
        let ctx = ValidationContext::default();
        check_header(&header, Span::DUMMY, &ctx, &errors);
        let error_vec = errors.into_vec();

        // Should not have speaker ID character errors (might have other errors though)
        assert!(
            !error_vec
                .iter()
                .any(|e| e.message.contains("invalid character"))
        );
    }

    /// Unknown headers map to exactly one `E525` diagnostic.
    ///
    /// This keeps malformed-header reporting predictable for downstream tooling.
    #[test]
    fn test_unknown_header_reports_error() {
        // Header::Unknown is used for unknown/malformed headers
        let header = Header::Unknown {
            text: WarningText::new("@Unknown:\tsomething".to_string()),
            parse_reason: Some("Unrecognized header type".to_string()),
            suggested_fix: None,
        };

        let errors = ErrorCollector::new();
        let ctx = ValidationContext::default();
        check_header(&header, Span::DUMMY, &ctx, &errors);
        let error_vec = errors.into_vec();

        // Should report exactly one E525 error
        assert_eq!(
            error_vec.len(),
            1,
            "Should have exactly one error for unknown header"
        );
        assert_eq!(error_vec[0].code, ErrorCode::UnknownHeader);
        assert_eq!(error_vec[0].severity, Severity::Error);
        assert!(
            error_vec[0].message.contains("Unknown or malformed header"),
            "Error message should mention unknown header"
        );
    }

    // ── E534–E539: Unsupported value tests ──────────────────────────

    #[test]
    fn test_e534_unsupported_option() {
        use crate::model::{ChatOptionFlag, ChatOptionFlags};

        let header = Header::Options {
            options: ChatOptionFlags::new(vec![
                ChatOptionFlag::Ca,
                ChatOptionFlag::Unsupported("NewThing".to_string()),
            ]),
        };

        let errors = ErrorCollector::new();
        let ctx = ValidationContext::default();
        check_header(&header, Span::DUMMY, &ctx, &errors);
        let error_vec = errors.into_vec();

        assert!(
            error_vec
                .iter()
                .any(|e| e.code == ErrorCode::UnsupportedOption),
            "Should report E534 for unsupported option"
        );
    }

    #[test]
    fn test_e534_known_options_no_unsupported_warning() {
        use crate::model::{ChatOptionFlag, ChatOptionFlags};

        let header = Header::Options {
            options: ChatOptionFlags::new(vec![ChatOptionFlag::Ca, ChatOptionFlag::NoAlign]),
        };

        let errors = ErrorCollector::new();
        let ctx = ValidationContext::default();
        check_header(&header, Span::DUMMY, &ctx, &errors);
        let error_vec = errors.into_vec();

        assert!(
            !error_vec
                .iter()
                .any(|e| e.code == ErrorCode::UnsupportedOption),
            "Known options should not trigger E534"
        );
    }

    #[test]
    fn test_e535_unsupported_media_type() {
        use crate::model::{MediaHeader, MediaType};

        let header = Header::Media(MediaHeader::new(
            "test",
            MediaType::Unsupported("hologram".to_string()),
        ));

        let errors = ErrorCollector::new();
        let ctx = ValidationContext::default();
        check_header(&header, Span::DUMMY, &ctx, &errors);
        let error_vec = errors.into_vec();

        assert_eq!(error_vec.len(), 1);
        assert_eq!(error_vec[0].code, ErrorCode::UnsupportedMediaType);
    }

    #[test]
    fn test_e536_unsupported_media_status() {
        use crate::model::{MediaHeader, MediaStatus, MediaType};

        let header = Header::Media(
            MediaHeader::new("test", MediaType::Audio)
                .with_status(MediaStatus::Unsupported("archived".to_string())),
        );

        let errors = ErrorCollector::new();
        let ctx = ValidationContext::default();
        check_header(&header, Span::DUMMY, &ctx, &errors);
        let error_vec = errors.into_vec();

        assert_eq!(error_vec.len(), 1);
        assert_eq!(error_vec[0].code, ErrorCode::UnsupportedMediaStatus);
    }

    #[test]
    fn test_e537_unsupported_number() {
        use crate::model::Number;

        let header = Header::Number {
            number: Number::Unsupported("99".to_string()),
        };

        let errors = ErrorCollector::new();
        let ctx = ValidationContext::default();
        check_header(&header, Span::DUMMY, &ctx, &errors);
        let error_vec = errors.into_vec();

        assert_eq!(error_vec.len(), 1);
        assert_eq!(error_vec[0].code, ErrorCode::UnsupportedNumber);
    }

    #[test]
    fn test_e538_unsupported_recording_quality() {
        use crate::model::RecordingQuality;

        let header = Header::RecordingQuality {
            quality: RecordingQuality::Unsupported("excellent".to_string()),
        };

        let errors = ErrorCollector::new();
        let ctx = ValidationContext::default();
        check_header(&header, Span::DUMMY, &ctx, &errors);
        let error_vec = errors.into_vec();

        assert_eq!(error_vec.len(), 1);
        assert_eq!(error_vec[0].code, ErrorCode::UnsupportedRecordingQuality);
    }

    #[test]
    fn test_e539_unsupported_transcription() {
        use crate::model::Transcription;

        let header = Header::Transcription {
            transcription: Transcription::Unsupported("rough".to_string()),
        };

        let errors = ErrorCollector::new();
        let ctx = ValidationContext::default();
        check_header(&header, Span::DUMMY, &ctx, &errors);
        let error_vec = errors.into_vec();

        assert_eq!(error_vec.len(), 1);
        assert_eq!(error_vec[0].code, ErrorCode::UnsupportedTranscription);
    }

    #[test]
    fn test_known_media_no_warnings() {
        use crate::model::{MediaHeader, MediaStatus, MediaType};

        let header = Header::Media(
            MediaHeader::new("test", MediaType::Audio).with_status(MediaStatus::Unlinked),
        );

        let errors = ErrorCollector::new();
        let ctx = ValidationContext::default();
        check_header(&header, Span::DUMMY, &ctx, &errors);
        let error_vec = errors.into_vec();

        assert!(
            error_vec.is_empty(),
            "Known media type and status should not trigger warnings"
        );
    }
}
