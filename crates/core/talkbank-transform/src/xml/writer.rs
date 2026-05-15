//! Top-level entry point for CHAT → TalkBank XML emission.
//!
//! Owns the public [`write_chat_xml`] function, the shared
//! namespace/version constants pushed onto every `<CHAT>` root, and
//! the [`XmlEmitter`] struct that carries the `quick-xml` writer plus
//! per-document cursor state across the emission passes.
//!
//! Emission logic is split across sibling modules so no single file
//! owns the whole writer:
//!
//! - [`super::root`] — document / participants / body / utterance
//!   orchestration, plus metadata helpers (corpus lookup, date/age/sex
//!   formatting, per-speaker extras, options flags).
//! - [`super::word`] — word / terminator / separator / pause / retrace
//!   / annotated-word / replaced-word emission, plus the attribute
//!   helpers for `<w type=…>`, `<w untranscribed=…>`, retrace `<k>`,
//!   and `<tagMarker>`.
//! - [`super::mor`] — `<mor>`/`<mw>`/`<gra>` subtrees, the
//!   `UtteranceTiers` collector, and `%mor` feature serialization.
//!
//! Each submodule extends [`XmlEmitter`] with its own `impl` block,
//! so every element-level helper still shares the one canonical write
//! path held inside this struct.

use quick_xml::Writer;
use quick_xml::escape::partial_escape;
use quick_xml::events::BytesText;

use talkbank_model::model::ChatFile;
use talkbank_model::validation::ValidationState;

use super::error::XmlWriteError;

/// TalkBank-XML namespace URI. The schema declares this as the default
/// namespace on every emitted document.
pub(super) const TALKBANK_NS: &str = "http://www.talkbank.org/ns/talkbank";

/// XMLSchema-instance namespace URI, bound to the `xsi:` prefix so the
/// `schemaLocation` attribute validates against the standard schema.
pub(super) const XSI_NS: &str = "http://www.w3.org/2001/XMLSchema-instance";

/// Canonical `xsi:schemaLocation` value. Points consumers at the
/// published schema document hosted on talkbank.org.
pub(super) const SCHEMA_LOCATION: &str =
    "http://www.talkbank.org/ns/talkbank https://talkbank.org/software/talkbank.xsd";

/// Schema version currently emitted. Matches the Java Chatter golden
/// corpus at `resources/corpus/reference-xml/`, which was produced against
/// schema 3.2.3. Bumping this attribute is a coordinated decision, not
/// a silent change — see
/// `docs/talkbank-xml-consumers-2026-04.md`.
pub(super) const SCHEMA_VERSION: &str = "3.2.3";

/// Serialize a [`ChatFile`] to a TalkBank-XML string.
///
/// See the `xml` module docs for scope and the staged-implementation
/// plan, and the [`XmlWriteError`] variants for the surface of staged
/// features that currently escalate rather than emit.
///
/// # Errors
///
/// Returns [`XmlWriteError`] for unimplemented CHAT features,
/// structurally incomplete input, or failures in the underlying
/// `quick-xml` writer. No panics — per crate policy.
pub fn write_chat_xml<S: ValidationState>(file: &ChatFile<S>) -> Result<String, XmlWriteError> {
    let mut emitter = XmlEmitter::for_file(file);
    emitter.emit_document(file)?;
    emitter.finish()
}

/// Owns the `quick-xml` writer and the per-document emission state
/// (currently only the running utterance ID counter).
///
/// Fields are `pub(super)` so sibling modules (`root`, `word`, `mor`)
/// can contribute their own `impl` blocks without routing every write
/// through accessor methods. Visibility stays inside the `xml` module
/// boundary — nothing leaks to the rest of the crate.
pub(super) struct XmlEmitter {
    pub(super) writer: Writer<Vec<u8>>,
    /// 0-based counter for `uID="u0"`, `uID="u1"`, … across the
    /// document. Java Chatter assigns these in encounter order.
    pub(super) next_utterance_id: u32,
    /// The "secondary" language (the second entry in `@Languages`),
    /// used to resolve bare `@s` word shortcuts to a concrete
    /// `<langs><single>code</single></langs>`. `None` when the file
    /// declares fewer than two languages; a bare `@s` then emits
    /// nothing (there's no toggle target).
    pub(super) secondary_language: Option<talkbank_model::model::LanguageCode>,
}

impl XmlEmitter {
    /// Construct an emitter primed with per-document state derived
    /// from `file` (currently just the secondary-language cache).
    /// `indent(b' ', 2)` gives two-space pretty-printing — the
    /// structural comparator ignores whitespace, but human-readable
    /// output matches TalkBank convention and keeps manual diffs
    /// easy during development.
    pub(super) fn for_file<S: ValidationState>(file: &ChatFile<S>) -> Self {
        Self {
            writer: Writer::new_with_indent(Vec::new(), b' ', 2),
            next_utterance_id: 0,
            secondary_language: file.languages.0.get(1).cloned(),
        }
    }
}

/// Build a `BytesText` for element text content using *partial*
/// escaping: only `<`, `>`, `&` are replaced. Java Chatter writes
/// apostrophes (`'`) and double-quotes (`"`) literally in element
/// content, which is valid XML 1.0 (the apostrophe predefined
/// entity is optional in element content per
/// https://www.w3.org/TR/xml11/#syntax). Using
/// [`quick_xml::events::BytesText::new`] directly would emit
/// `let&apos;s` for `let's` and cause downstream consumers — plus
/// the structural golden comparator — to split the text at the
/// entity reference. `partial_escape` avoids that.
pub(super) fn escape_text(s: &str) -> BytesText<'_> {
    BytesText::from_escaped(partial_escape(s))
}

impl XmlEmitter {
    pub(super) fn finish(self) -> Result<String, XmlWriteError> {
        let bytes = self.writer.into_inner();
        Ok(String::from_utf8(bytes)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_model::ParseValidateOptions;

    /// Happy-path emission for the minimal file. Not the
    /// golden-structural test — that lives in `talkbank-parser-tests` —
    /// but a fast unit test that catches breakage without requiring
    /// filesystem fixtures.
    #[test]
    fn emits_minimal_document() {
        let src = "@UTF8\n@Begin\n@Languages:\teng\n\
            @Participants:\tCHI Child\n\
            @ID:\teng|corpus|CHI|||||Child|||\n\
            *CHI:\thello .\n@End\n";
        let file =
            crate::parse_and_validate(src, ParseValidateOptions::default().with_validation())
                .expect("reference minimal file must parse");

        let xml = write_chat_xml(&file).expect("skeleton must emit");

        assert!(xml.contains("<CHAT"));
        assert!(xml.contains(r#"Version="3.2.3""#));
        assert!(xml.contains(r#"Lang="eng""#));
        assert!(xml.contains(r#"Corpus="corpus""#));
        assert!(xml.contains(r#"id="CHI""#));
        assert!(xml.contains(r#"role="Child""#));
        assert!(xml.contains("<w>hello</w>"));
        assert!(xml.contains(r#"<t type="p""#));
    }
}
