//! Convenience APIs and trait implementations for `Header`.
//!
//! These helpers centralize header-label metadata so serializers, diagnostics,
//! and external consumers can rely on a single source of truth for canonical
//! names and validation delegation.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Line>

use super::header::Header;
use crate::model::WriteChat;

impl Header {
    /// Returns canonical header label text without the leading `@`.
    ///
    /// This mapping normalizes enum variants to their surface CHAT labels and
    /// is used by serializers, diagnostics, and UI metadata views. Keeping the
    /// mapping here avoids duplicated string tables across parser/validator code.
    pub fn name(&self) -> &str {
        match self {
            Header::Utf8 => "UTF8",
            Header::Begin => "Begin",
            Header::End => "End",
            Header::Languages { .. } => "Languages",
            Header::Participants { .. } => "Participants",
            Header::ID(_) => "ID",
            Header::Date { .. } => "Date",
            Header::Comment { .. } => "Comment",
            Header::Pid { .. } => "PID",
            Header::Media(_) => "Media",
            Header::Situation { .. } => "Situation",
            Header::Types(_) => "Types",
            Header::NewEpisode => "New Episode",
            Header::TapeLocation { .. } => "Tape Location",
            Header::BeginGem { .. } => "Bg",
            Header::EndGem { .. } => "Eg",
            Header::LazyGem { .. } => "G",
            Header::Blank => "Blank",
            Header::Number { .. } => "Number",
            Header::RecordingQuality { .. } => "Recording Quality",
            Header::Transcription { .. } => "Transcription",
            Header::Font { .. } => "Font",
            Header::Window { .. } => "Window",
            Header::ColorWords { .. } => "Color words",
            Header::Birth { .. } => "Birth of",
            Header::Birthplace { .. } => "Birthplace of",
            Header::TimeDuration { .. } => "Time Duration",
            Header::TimeStart { .. } => "Time Start",
            Header::Location { .. } => "Location",
            Header::RoomLayout { .. } => "Room Layout",
            Header::Transcriber { .. } => "Transcriber",
            Header::Warning { .. } => "Warning",
            Header::Unknown { .. } => "Unknown",
            Header::Activities { .. } => "Activities",
            Header::Bck { .. } => "Bck",
            Header::L1Of { .. } => "L1 of",
            Header::Options { .. } => "Options",
            Header::Page { .. } => "Page",
            Header::Videos { .. } => "Videos",
            Header::T { .. } => "T",
        }
    }

    /// Serializes this header to an owned CHAT line string.
    ///
    /// Prefer [`WriteChat`] directly in hot paths to avoid temporary allocation.
    /// This helper exists so callers that only need display text can skip
    /// passing a writer buffer altogether.
    pub fn to_chat(&self) -> String {
        let mut s = String::new();
        let _ = self.write_chat(&mut s);
        s
    }
}

impl std::fmt::Display for Header {
    /// Formats this header in canonical CHAT line form.
    ///
    /// This delegates to [`WriteChat`] so display output stays identical to file serialization.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.write_chat(f)
    }
}

impl crate::validation::Validate for Header {
    /// Validates this header using shared header validation logic.
    ///
    /// `Header` values do not carry their own span, so callers that need
    /// source-accurate diagnostics should validate at line/chat-file level.
    fn validate(
        &self,
        context: &crate::validation::ValidationContext,
        errors: &impl crate::ErrorSink,
    ) {
        crate::validation::header::check_header(self, crate::Span::DUMMY, context, errors);
    }
}
