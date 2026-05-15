//! Core `ChatFile` type and line-container wrappers.
//!
//! CHAT reference anchors:
//! - [File headers](https://talkbank.org/0info/manuals/CHAT.html#File_Headers)
//! - [Participants header](https://talkbank.org/0info/manuals/CHAT.html#Participants_Header)
//! - [ID header](https://talkbank.org/0info/manuals/CHAT.html#ID_Header)

use crate::model::{ChatOptionFlags, Header, LanguageCodes, MediaHeader, Participant, SpeakerCode};
use crate::validation::{NotValidated, Validate, Validated, ValidationContext, ValidationState};
use crate::{ErrorSink, LineMap};
use indexmap::IndexMap;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use talkbank_derive::{SemanticEq, SpanShift};

use crate::Line;

/// Top-level CHAT transcript model.
///
/// `ChatFile` preserves transcript line ordering and carries parser-derived
/// participant metadata plus optional runtime indexing (`line_map`).
///
/// # Structure
///
/// A CHAT file is a sequence of lines where each line is either:
/// - **Header**: Metadata starting with `@` (e.g., `@Begin`, `@Comment`)
/// - **Utterance**: Transcribed speech with main tier and dependent tiers
///
/// Headers and utterances are **interleaved** to preserve file structure:
///
/// ```text
/// @UTF8
/// @Begin
/// *CHI: hello .
/// @Comment: Between utterances
/// *MOT: hi .
/// @End
/// ```
///
/// # CHAT Manual Reference
///
/// - [CHAT Format Overview](https://talkbank.org/0info/manuals/CHAT.html)
/// - [File Structure](https://talkbank.org/0info/manuals/CHAT.html#File_Headers)
///
/// # Type-State Pattern
///
/// `ChatFile` uses a type-state pattern to enforce validation at compile-time:
/// - `ChatFile<NotValidated>` - Fresh from parser, not yet validated
/// - `ChatFile<Validated>` - Has been validated, can be exported to JSON
///
/// Only `ChatFile<Validated>` has JSON serialization methods available.
///
/// # Example
///
/// ```
/// use talkbank_model::model::{ChatFile, Header, LanguageCode, Line};
/// use talkbank_model::{Span, ErrorCollector};
///
/// // Parsing returns NotValidated
/// let chat_file = ChatFile::new(vec![
///     Line::header_with_span(Header::Utf8, Span::DUMMY),
///     Line::header_with_span(Header::Begin, Span::DUMMY),
///     Line::header_with_span(Header::Languages { codes: vec![LanguageCode::new("eng")].into() }, Span::DUMMY),
///     // Utterances would be added here
///     Line::header_with_span(Header::End, Span::DUMMY),
/// ]);
///
/// // Validate to get Validated state
/// let errors = ErrorCollector::new();
/// let validated = chat_file.validate_into(&errors, None);
///
/// // Only validated files can be serialized to JSON
/// // let json = match validated.to_json_validated() {
/// //     Ok(json) => json,
/// //     Err(_) => return,
/// // };
/// ```
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
pub struct ChatFile<S: ValidationState = NotValidated> {
    /// Sequence of lines (headers + utterances) in file order.
    ///
    /// The vector preserves exact interleaving from source text.
    ///
    /// Headers and utterances can appear in any order, though typical files have:
    /// - File headers at the beginning (@UTF8, @Begin, @Participants, @ID)
    /// - Utterances in the middle
    /// - Comments and notes interleaved with utterances
    /// - @End header at the end
    pub lines: ChatFileLines,

    /// Structured participant metadata (derived from `@Participants` + `@ID` + `@Birth`).
    ///
    /// Populated during parsing by matching:
    /// - @Participants entries with their corresponding @ID headers
    /// - Optional `@Birth of <CODE>` headers
    ///
    /// Each participant listed in @Participants MUST have a corresponding @ID header.
    /// This is validated during parsing (E522 error if missing).
    ///
    /// # Example
    ///
    /// For a CHAT file with:
    /// ```chat
    /// @Participants:    CHI Ruth Target_Child, INV Chiat Investigator
    /// @ID:    eng|chiat|CHI|10;03.||||Target_Child|||
    /// @ID:    eng|chiat|INV|||||Investigator|||
    /// @Birth of CHI:    28-JUN-2001
    /// ```
    ///
    /// This map will contain:
    /// - CHI => Participant { code: "CHI", name: Some("Ruth"), id: {...}, birth_date: Some("28-JUN-2001") }
    /// - INV => Participant { code: "INV", name: Some("Chiat"), id: {...}, birth_date: None }
    ///
    /// # Note
    ///
    /// Uses `#[serde(default)]` for backwards compatibility with existing JSON.
    ///
    /// Participants are stored in an `IndexMap` so serialization preserves the declared order
    /// from @Participants/@ID headers, ensuring deterministic JSON output.
    #[serde(default)]
    pub participants: IndexMap<SpeakerCode, Participant>,

    /// Languages from `@Languages` header. Empty if absent.
    ///
    /// Auto-extracted from `lines` during construction. The first code is the
    /// transcript default language.
    #[serde(default)]
    pub languages: LanguageCodes,

    /// Option flags from `@Options` header. Empty if absent.
    ///
    /// Auto-extracted from `lines` during construction.
    #[serde(default)]
    pub options: ChatOptionFlags,

    /// Media info from `@Media` header. `None` if absent.
    ///
    /// Auto-extracted from `lines` during construction. Only the first
    /// `@Media` header is captured (CHAT files have at most one).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media: Option<Box<MediaHeader>>,

    /// Byte-offset line index for O(log n) offset-to-line lookups.
    ///
    /// `Some(...)` for files parsed from source text; `None` for programmatically
    /// constructed files (batchalign, tests) where no source text exists.
    /// Rebuilt from source on re-parse — never serialized.
    #[serde(skip, default)]
    #[schemars(skip)]
    #[semantic_eq(skip)]
    #[span_shift(skip)]
    pub line_map: Option<LineMap>,

    /// Type-state marker for validation state.
    ///
    /// Zero runtime cost; used only for compile-time API gating.
    #[serde(skip, default = "default_phantom_data")]
    #[schemars(skip)]
    #[semantic_eq(skip)]
    #[span_shift(skip)]
    _state: PhantomData<S>,
}

/// Provides the serde default for the type-state phantom parameter.
fn default_phantom_data<S>() -> PhantomData<S> {
    PhantomData
}

/// Scan `lines` once to extract `@Languages`, `@Options`, and `@Media` fields.
fn extract_header_fields(
    lines: &[Line],
) -> (LanguageCodes, ChatOptionFlags, Option<Box<MediaHeader>>) {
    let mut languages = LanguageCodes::default();
    let mut options = ChatOptionFlags::default();
    let mut media: Option<Box<MediaHeader>> = None;

    for line in lines {
        if let Line::Header { header, .. } = line {
            match header.as_ref() {
                Header::Languages { codes } => {
                    languages = codes.clone();
                }
                Header::Options { options: opt_flags } => {
                    options = opt_flags.clone();
                }
                Header::Media(m) if media.is_none() => {
                    media = Some(Box::new(m.clone()));
                }
                _ => {}
            }
        }
    }

    (languages, options, media)
}

impl<S: ValidationState> ChatFile<S> {
    /// Re-tags the same payload with a different validation-state marker.
    ///
    /// Internal helper for type-state transitions. External callers should use
    /// [`ChatFile::validate_into`] to move from `NotValidated` to `Validated`.
    fn change_state<T: ValidationState>(self) -> ChatFile<T> {
        ChatFile {
            lines: self.lines,
            participants: self.participants,
            languages: self.languages,
            options: self.options,
            media: self.media,
            line_map: self.line_map,
            _state: PhantomData,
        }
    }
}

impl ChatFile<NotValidated> {
    /// Build a `NotValidated` file from parsed lines with empty participant map.
    ///
    /// Use when participant metadata has not been assembled yet (for example,
    /// intermediate parser stages before header post-processing).
    pub fn new(lines: Vec<Line>) -> Self {
        let (languages, options, media) = extract_header_fields(&lines);
        Self {
            lines: lines.into(),
            participants: IndexMap::new(),
            languages,
            options,
            media,
            line_map: None,
            _state: PhantomData,
        }
    }

    /// Build a `NotValidated` file with parser-populated participant metadata.
    ///
    /// This constructor is used once `@Participants`/`@ID` reconciliation has
    /// produced the participant map but validation has not run yet.
    pub fn with_participants(
        lines: Vec<Line>,
        participants: IndexMap<SpeakerCode, Participant>,
    ) -> Self {
        let (languages, options, media) = extract_header_fields(&lines);
        Self {
            lines: lines.into(),
            participants,
            languages,
            options,
            media,
            line_map: None,
            _state: PhantomData,
        }
    }

    /// Build a `NotValidated` file with participants and offset line map.
    ///
    /// Use when source text is available and offset lookups should be preserved.
    pub fn with_line_map(
        lines: Vec<Line>,
        participants: IndexMap<SpeakerCode, Participant>,
        line_map: LineMap,
    ) -> Self {
        let (languages, options, media) = extract_header_fields(&lines);
        Self {
            lines: lines.into(),
            participants,
            languages,
            options,
            media,
            line_map: Some(line_map),
            _state: PhantomData,
        }
    }

    /// Validates this file and transitions type-state to `Validated`.
    ///
    /// Validation findings are streamed to `errors`; the returned value is always
    /// the same file payload tagged as validated.
    ///
    /// # Parameters
    ///
    /// * `errors` - Error sink for streaming validation errors
    /// * `filename` - Optional filename (without extension) for E531 media filename validation
    ///
    /// # Example
    ///
    /// ```ignore
    /// use talkbank_model::{ChatFile, ErrorCollector};
    ///
    /// let file: ChatFile<NotValidated> = parse_file(content);
    /// let errors = ErrorCollector::new();
    /// let validated: ChatFile<Validated> = file.validate_into(&errors, Some("myfile"));
    ///
    /// // Now can serialize to JSON
    /// let json = match validated.to_json_validated() {
    ///     Ok(json) => json,
    ///     Err(_) => return,
    /// };
    /// ```
    pub fn validate_into(
        self,
        errors: &impl ErrorSink,
        filename: Option<&str>,
    ) -> ChatFile<Validated> {
        // Run validation, streaming errors
        self.validate(errors, filename);

        // Convert to Validated state
        self.change_state()
    }
}

/// Newtype wrapper around a list of lines in a CHAT file.
///
/// Preserves the interleaved ordering of headers and utterances.
///
/// # Reference
///
/// - [File headers](https://talkbank.org/0info/manuals/CHAT.html#File_Headers)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
#[serde(transparent)]
#[schemars(transparent)]
pub struct ChatFileLines(pub Vec<Line>);

impl ChatFileLines {
    /// Wrap line values while preserving file order.
    ///
    /// The wrapper exists to keep a typed boundary around the interleaved line
    /// stream while still exposing `Vec` ergonomics through `Deref`.
    pub fn new(lines: Vec<Line>) -> Self {
        Self(lines)
    }

    /// Return `true` when the file has no lines.
    ///
    /// This check is intentionally structural and does not inspect header state.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Deref for ChatFileLines {
    type Target = Vec<Line>;

    /// Borrows the underlying line vector.
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for ChatFileLines {
    /// Mutably borrows the underlying line vector.
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<Vec<Line>> for ChatFileLines {
    /// Wraps owned lines without copying.
    fn from(lines: Vec<Line>) -> Self {
        Self(lines)
    }
}

impl<'a> IntoIterator for &'a ChatFileLines {
    type Item = &'a Line;
    type IntoIter = std::slice::Iter<'a, Line>;

    /// Iterates immutably over file lines.
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl<'a> IntoIterator for &'a mut ChatFileLines {
    type Item = &'a mut Line;
    type IntoIter = std::slice::IterMut<'a, Line>;

    /// Iterates mutably over file lines.
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter_mut()
    }
}

impl IntoIterator for ChatFileLines {
    type Item = Line;
    type IntoIter = std::vec::IntoIter<Line>;

    /// Consumes the wrapper and yields owned lines.
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl Validate for ChatFileLines {
    /// Line-level validation is driven from `ChatFile` where header/order context exists.
    fn validate(&self, _context: &ValidationContext, _errors: &impl ErrorSink) {}
}
