//! Error type for CHAT → TalkBank XML emission.
//!
//! Matches the workspace convention of `thiserror`-based domain errors
//! with no panics. Callers propagate these via the `?` operator; the CLI
//! maps them to exit codes / diagnostics at the boundary.

use thiserror::Error;

/// Errors raised by [`crate::xml::write_chat_xml`].
///
/// Intentionally granular so CLI and harness callers can distinguish
/// "we haven't implemented this yet" from "the input is structurally
/// malformed" from "the XML writer itself failed."
#[derive(Debug, Error)]
pub enum XmlWriteError {
    /// A phonetic / syllabification tier (`%pho`, `%mod`, `%phosyl`,
    /// `%modsyl`, `%phoaln`) was encountered. **These tiers are
    /// permanently out of scope for the Rust XML emitter.**
    ///
    /// The rich XML shape (`<pg>/<pw>/<ph>/<cmph>/<ss>`) was
    /// historically consumed by the Phon project, which has since
    /// pivoted away from TalkBank XML and now interoperates via pure
    /// CHAT. No other public consumer reads these elements, so
    /// there is no downstream reason to port the phon-ipa IPA
    /// parser into Rust solely to round-trip them.
    ///
    /// Files carrying `%pho` / `%mod` still parse, validate, and
    /// round-trip through CHAT unchanged — only the XML projection
    /// is declined.
    #[error(
        "phonetic tier XML emission is permanently unsupported \
         (Phon has moved to CHAT-only interchange); \
         encountered phonetic tier on utterance index {utterance_index}"
    )]
    PhoneticTierUnsupported {
        /// 0-based index of the utterance that carries the tier,
        /// to help callers correlate with source CHAT.
        utterance_index: usize,
    },

    /// A CHAT feature that has a known XML shape has not yet been wired
    /// into the Rust emitter. Distinguished from `PhoNotImplemented`
    /// because this covers the incremental-TDD state of the port itself,
    /// not a cross-crate dependency.
    #[error("XML emission for CHAT feature '{feature}' is not yet implemented")]
    FeatureNotImplemented {
        /// Short human-readable feature name (e.g. `"%mor tier"`,
        /// `"overlap marker"`).
        feature: String,
    },

    /// An utterance carries more than one structured dependent tier
    /// of the same kind (e.g. two `%mor` lines). Rust's
    /// `UtteranceTiers` holds a single slot per structured tier;
    /// the XML emitter surfaces the collision as a hard error
    /// because silent "keep first, drop rest" would lose data.
    /// Valid CHAT keeps these tiers singular per utterance.
    #[error("utterance {utterance_index} has multiple {tier} tiers; XML emission requires one")]
    MultipleStructuredTiers {
        /// 0-based index of the offending utterance.
        utterance_index: usize,
        /// Tier label (`"%mor"`, `"%gra"`, `"%wor"`).
        tier: &'static str,
    },

    /// Required metadata was absent from the model. Indicates a malformed
    /// `ChatFile` — e.g. a participant with no `@ID` — reaching the
    /// emitter. Upstream validation should normally have caught it.
    #[error("missing required metadata while emitting XML: {what}")]
    MissingMetadata {
        /// Short description of the missing field (e.g.
        /// `"Corpus attribute (no @ID header)"`).
        what: String,
    },

    /// The underlying XML writer (`quick-xml`) returned an error.
    #[error("XML writer failed: {0}")]
    XmlBackend(#[from] quick_xml::Error),

    /// Write-side I/O error from the `quick-xml` writer. In practice
    /// the writer is backed by an in-memory `Vec<u8>` so this variant
    /// should be unreachable for valid input, but `?` conversion
    /// requires it to be declared.
    #[error("XML writer I/O failure: {0}")]
    Io(#[from] std::io::Error),

    /// UTF-8 conversion of the emitted XML buffer failed.
    ///
    /// Not expected in practice; `quick-xml` produces UTF-8 output, but
    /// we guard the conversion rather than `expect()` per crate policy.
    #[error("emitted XML was not valid UTF-8: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),
}
