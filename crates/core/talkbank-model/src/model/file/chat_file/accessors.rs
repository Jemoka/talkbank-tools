//! Read-oriented accessors over `ChatFile` structure and participants.
//!
//! These helpers give downstream readers deterministic iteration order without
//! exposing the internal `Line` enum. `get_participant` and `all_participants`
//! reuse the canonical participant map to avoid re-parsing `@Participants`.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Participants_Header>

use crate::model::header::IDHeader;
use crate::model::{Participant, Utterance};
use crate::validation::ValidationState;
use crate::{Header, WriteChat};
use tracing::{debug, info};

use super::ChatFile;

impl<S: ValidationState> ChatFile<S> {
    /// Iterates header lines in original file order.
    ///
    /// Order preservation matters because CHAT allows headers to appear between
    /// utterances (for example `@Comment` lines mid-file).
    pub fn headers(&self) -> impl Iterator<Item = &Header> {
        self.lines.iter().filter_map(|line| line.as_header())
    }

    /// Iterates the file's `@ID` headers in original file order.
    ///
    /// Convenience over [`headers`](Self::headers) for the common case of
    /// "give me the typed `@ID` rows" — every CHAT reader that filters
    /// header lines by variant would otherwise hand-roll the same
    /// `matches!(_, Header::ID(_))` extraction.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use talkbank_model::model::ChatFile;
    /// # let chat_file = ChatFile::new(vec![]);
    /// for id in chat_file.id_headers() {
    ///     println!("speaker {} role {}", id.speaker, id.role);
    /// }
    /// ```
    pub fn id_headers(&self) -> impl Iterator<Item = &IDHeader> {
        self.headers().filter_map(|h| match h {
            Header::ID(id) => Some(id),
            _ => None,
        })
    }

    /// Iterates header lines with source spans in file order.
    ///
    /// Useful for diagnostics or transforms that need both typed header values
    /// and their byte locations in the source transcript.
    pub fn headers_with_spans(&self) -> impl Iterator<Item = (&Header, crate::Span)> {
        self.lines.iter().filter_map(|line| match line {
            crate::model::Line::Header { header, span } => Some((header.as_ref(), *span)),
            _ => None,
        })
    }

    /// Iterates utterance lines in original file order.
    ///
    /// Returned items exclude header lines but keep relative utterance ordering unchanged.
    pub fn utterances(&self) -> impl Iterator<Item = &Utterance> {
        self.lines.iter().filter_map(|line| line.as_utterance())
    }

    /// Returns the number of header lines in `self.lines`.
    ///
    /// This is computed on demand from line variants instead of cached metadata.
    pub fn header_count(&self) -> usize {
        self.lines.iter().filter(|line| line.is_header()).count()
    }

    /// Returns the number of utterance lines in `self.lines`.
    ///
    /// This is computed on demand from line variants instead of cached metadata.
    pub fn utterance_count(&self) -> usize {
        self.lines.iter().filter(|line| line.is_utterance()).count()
    }

    /// Returns participant metadata for a speaker code, if present.
    ///
    /// Lookups are exact and case-sensitive, matching canonical CHAT speaker codes.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use talkbank_model::model::ChatFile;
    /// # let chat_file = ChatFile::new(vec![]);
    /// if let Some(chi) = chat_file.get_participant("CHI") {
    ///     println!("CHI's age: {:?}", chi.age());
    /// }
    /// ```
    pub fn get_participant(&self, code: &str) -> Option<&Participant> {
        self.participants.get(code)
    }

    /// Returns all participants from the internal participant map.
    ///
    /// Order follows map iteration and should not be assumed stable for UI ordering.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use talkbank_model::model::ChatFile;
    /// # let chat_file = ChatFile::new(vec![]);
    /// for participant in chat_file.all_participants() {
    ///     println!("{}: {}", participant.code, participant.role);
    /// }
    /// ```
    pub fn all_participants(&self) -> Vec<&Participant> {
        self.participants.values().collect()
    }

    /// Returns number of participant entries currently materialized.
    ///
    /// This reflects parsed/validated participant state, not a separate header reparse.
    pub fn participant_count(&self) -> usize {
        self.participants.len()
    }

    /// Serializes the full file to an owned CHAT string.
    ///
    /// Instrumentation fields capture line/header/utterance counts so tracing
    /// backends can correlate serialization cost with transcript size.
    ///
    /// Header/utterance ordering is preserved so serialization roundtrips can be
    /// verified against `ChatFile::lines`.
    #[tracing::instrument(skip(self), fields(lines = self.lines.len()))]
    pub fn to_chat(&self) -> String {
        let header_count = self.header_count();
        let utterance_count = self.utterance_count();
        debug!(
            "Serializing CHAT file ({} lines: {} headers, {} utterances)",
            self.lines.len(),
            header_count,
            utterance_count
        );
        let mut s = String::new();
        let _ = self.write_chat(&mut s);
        info!("Serialized to {} bytes", s.len());
        s
    }
}
