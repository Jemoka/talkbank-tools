//! Named-constant Unicode codepoints used throughout CHAT structural
//! syntax and CHAT validation.
//!
//! Several non-ASCII codepoints carry stable, load-bearing meaning in
//! CHAT — quotation-group bookends, tag and vocative separators,
//! curly-quote characters CLAN's `check` flags as illegal — and they
//! were previously inlined as bare `'\u{201C}'`-style char literals at
//! every use site across `talkbank-model`, `talkbank-parser`,
//! `clan-core`, the re2c parser, and the downstream
//! `batchalign-chat-ops` ASR pipeline. That scatter made the
//! codepoints' CHAT semantics invisible at the call site (a reader
//! had to know `\u{201C}` is "the structural quotation-begin
//! character") and made grep-by-name impossible.
//!
//! This module is the single home for those codepoints. Each constant
//! is named after its Unicode designation; the doc comment explains
//! the CHAT role.
//!
//! # Why not an enum
//!
//! These are not a closed set with switching behavior — they're
//! lookup names. Several of them are written by serializers that need
//! a `char` directly (`w.write_char(LEFT_DOUBLE_QUOTE)`); a `match`
//! against a discriminant adds friction without clarifying anything.
//! Named `pub const char` matches the way the codepoints are actually
//! used.

/// `"` — Unicode `LEFT DOUBLE QUOTATION MARK` (U+201C).
///
/// CHAT role: opening bookend of a [QuotationGroup][qg] in the main
/// tier — paired with [`RIGHT_DOUBLE_QUOTE`]. Inside ASR-emitted word
/// tokens (where Whisper transcribes quoted speech verbatim) it is
/// noise that the ASR boundary-trim pipeline strips before validation.
///
/// [qg]: crate::model::content::group::QuotationGroup
pub const LEFT_DOUBLE_QUOTE: char = '\u{201C}';

/// `"` — Unicode `RIGHT DOUBLE QUOTATION MARK` (U+201D).
///
/// CHAT role: closing bookend of a [QuotationGroup][qg] in the main
/// tier — paired with [`LEFT_DOUBLE_QUOTE`]. Same boundary-noise
/// disposition for ASR-emitted word tokens.
///
/// [qg]: crate::model::content::group::QuotationGroup
pub const RIGHT_DOUBLE_QUOTE: char = '\u{201D}';

/// `„` — Unicode `DOUBLE LOW-9 QUOTATION MARK` (U+201E).
///
/// CHAT role: the [tag marker][tag] separator
/// ([`crate::model::content::separator::Separator::Tag`]) used in
/// Conversation Analysis transcription. NOT a quotation character in
/// CHAT despite its Unicode name.
///
/// [tag]: https://talkbank.org/0info/manuals/CHAT.html#TAG_Marker
pub const TAG_MARKER: char = '\u{201E}';

/// `‡` — Unicode `DOUBLE DAGGER` (U+2021).
///
/// CHAT role: vocative-marker separator
/// ([`crate::model::content::separator::Separator::Vocative`]).
pub const VOCATIVE_MARKER: char = '\u{2021}';

/// `'` — Unicode `LEFT SINGLE QUOTATION MARK` (U+2018).
///
/// CHAT role: **illegal**. CLAN's `check` flags this as error 139
/// ("Special quote U2018 must be replaced by single quote (')"). ASR
/// engines occasionally emit this curly form when transcribing
/// contractions; the boundary-trim pipeline strips it.
pub const LEFT_SINGLE_QUOTE: char = '\u{2018}';

/// `'` — Unicode `RIGHT SINGLE QUOTATION MARK` (U+2019).
///
/// CHAT role: **illegal**. CLAN's `check` flags this as error 138
/// ("Special quote U2019 must be replaced by single quote (')").
/// Same disposition as [`LEFT_SINGLE_QUOTE`].
pub const RIGHT_SINGLE_QUOTE: char = '\u{2019}';

/// `«` — Unicode `LEFT-POINTING DOUBLE ANGLE QUOTATION MARK` (U+00AB).
///
/// CHAT role: not a structural CHAT character. Some non-English ASR
/// providers emit this guillemet around quoted speech; the boundary-
/// trim pipeline strips it.
pub const LEFT_GUILLEMET: char = '\u{00AB}';

/// `»` — Unicode `RIGHT-POINTING DOUBLE ANGLE QUOTATION MARK` (U+00BB).
///
/// CHAT role: paired with [`LEFT_GUILLEMET`]. Same disposition.
pub const RIGHT_GUILLEMET: char = '\u{00BB}';
