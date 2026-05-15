//! Provenance newtypes for text flowing through the ASR post-processing pipeline.
//!
//! These types encode WHERE text is in the ASR→CHAT pipeline and — for
//! [`ChatWordText`] specifically — carry a runtime-checked proof that the
//! value is CHAT-legal:
//!
//! - [`AsrRawText`] — raw tokens from an ASR provider (before any normalization)
//! - [`AsrNormalizedText`] — tokens after the full 8-stage pipeline
//! - [`ChatWordText`] — text the CHAT parser accepts as a legal main-tier token
//!
//! The progression is: `AsrRawText` (on [`AsrElement`]) → `AsrNormalizedText`
//! (on [`AsrWord`]) → `ChatWordText` (on the downstream CHAT word-description
//! layer).
//!
//! # Validation at construction (`ChatWordText`)
//!
//! Unlike the upstream two types, `ChatWordText` is **constructible only
//! via fallible conversion**: [`ChatWordText::try_from`],
//! [`ChatWordText::try_from_with_parser`], or their language-aware
//! counterparts [`ChatWordText::try_from_lang`] /
//! [`ChatWordText::try_from_lang_with_parser`]. The `new(&str)` constructor
//! that existed before 2026-04-22 has been removed — its infallible shape
//! let main-tier-illegal text (notably `%`, digit-leading hyphen
//! compounds, and anything else the word fragment parser rejects) reach
//! CHAT assembly silently. That path produced the user `c465e6e8-97c`
//! job failure (2026-04-22) where Rev.AI tokens `"80%"`, `"20%"`, and
//! `"17-year-old"` crashed the utseg pre-validation gate downstream.
//!
//! Today, every `ChatWordText` value is a proof that:
//!
//! 1. The contained text is either a known CHAT terminator
//!    (`Terminator::is_chat_terminator`) or
//! 2. Parses cleanly via `TreeSitterParser::parse_word_fragment`, and
//! 3. (For the `try_from_lang*` variants) satisfies every language-aware
//!    word-level rule `talkbank_model::Validate for Word` applies under
//!    the declared language — including E220 (numeric digits not
//!    allowed) for languages outside the digit-permitting set.
//!
//!
//! [`AsrRawText`] and [`AsrNormalizedText`] continue to follow the
//! simpler `new()` pattern — their documented postconditions (raw
//! provider output, post-pipeline normalization) are not amenable to a
//! single-step parser check, and the risk they carry is bounded by
//! `ChatWordText`'s gate at the CHAT-assembly boundary.
//!
//! [`AsrElement`]: super::AsrElement
//! [`AsrWord`]: super::AsrWord
use serde::{Deserialize, Serialize};
use std::fmt;

/// Raw text from an ASR provider, before any normalization.
///
/// **Source**: Provider-specific bridge code (Rev.AI, Whisper, HK engines).
///
/// **Contains**: Digits, spaces, provider markers (`<pause>`), untouched
/// provider output. May include punctuation tokens, multi-word strings,
/// or language-specific characters that haven't been normalized yet.
///
/// The ASR post-processing pipeline reads this and produces
/// [`AsrNormalizedText`] after all 8 stages.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
#[repr(transparent)]
pub struct AsrRawText(String);

impl AsrRawText {
    /// Wraps a string as raw ASR text.
    ///
    /// No validation is performed — the caller supplies text exactly as
    /// received from the ASR provider.
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// Borrows the raw ASR text for read-only inspection.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AsrRawText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for AsrRawText {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl PartialEq<&str> for AsrRawText {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

/// ASR text after the full normalization pipeline (8 stages).
///
/// **Source**: `process_raw_asr()` in `asr_postprocess/mod.rs`.
///
/// **Contains**: Compound-merged, number-expanded, disfluency-marked text.
/// Filled pauses are in `&-um` form, orthographic replacements applied
/// (`'cause` → `(be)cause`), Cantonese normalization done (for `yue`).
///
/// **NOT yet CHAT syntax** — still needs `TreeSitterParser` to become AST nodes.
/// The next step is conversion to [`ChatWordText`] at the boundary between
/// ASR post-processing and CHAT assembly.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
#[repr(transparent)]
pub struct AsrNormalizedText(String);

impl AsrNormalizedText {
    /// Wraps a string as normalized ASR text.
    ///
    /// Call this after the normalization pipeline has processed the text
    /// through compound merging, number expansion, disfluency replacement,
    /// and retrace detection.
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// Borrows the normalized text for read-only inspection.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Applies a transformation to the inner text, returning a new wrapper.
    ///
    /// Useful for pipeline stages that mutate text in place:
    /// ```ignore
    /// w.text = w.text.map(|t| expand_number(t, lang));
    /// ```
    pub fn map(self, f: impl FnOnce(&str) -> String) -> Self {
        Self(f(&self.0))
    }

    /// Appends a string to the inner text.
    ///
    /// Used by hyphen-joining in `split_multiword_tokens`.
    pub fn push_str(&mut self, s: &str) {
        self.0.push_str(s);
    }

    /// Returns a lowercase copy of the inner text.
    pub fn to_lowercase(&self) -> String {
        self.0.to_lowercase()
    }

    /// Returns `true` if the text starts with the given pattern.
    pub fn starts_with(&self, pat: char) -> bool {
        self.0.starts_with(pat)
    }
}

impl fmt::Display for AsrNormalizedText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for AsrNormalizedText {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl PartialEq<&str> for AsrNormalizedText {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

/// Text ready for CHAT assembly via `TreeSitterParser`.
///
/// Constructible only via fallible conversion — see the module docstring
/// for the full invariant and the four variants (`try_from`,
/// `try_from_with_parser`, `try_from_lang`, `try_from_lang_with_parser`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
#[repr(transparent)]
pub struct ChatWordText(String);

/// Closed-set CHAT terminator shortcut + structural word-fragment parse.
///
/// Returns `Ok(Some(Word))` when `s` parses cleanly as a main-tier word
/// (caller may run further language-level checks on the Word). Returns
/// `Ok(None)` when `s` is a CHAT terminator (`.`, `?`, `!`, `+...`,
/// etc.) — valid main-tier content but not a word, so no Word AST to
/// hand back. Returns `Err(errors)` when the fragment parser rejects
/// the input.
///
/// `ChatWordText`'s vocabulary is "a token on the main tier", broader
/// than `parse_word_fragment`'s "a word" — the ASR pipeline emits each
/// utterance's terminator as an `AsrWord` entry, so the shortcut is
/// load-bearing, not an optimisation.
fn structural_check(
    s: &str,
    parser: &talkbank_parser::TreeSitterParser,
) -> Result<Option<talkbank_model::model::Word>, Vec<talkbank_model::ParseError>> {
    if talkbank_model::model::Terminator::is_chat_terminator(s) {
        return Ok(None);
    }
    if super::MOR_PUNCT.contains(&s) {
        return Ok(None);
    }
    let errors = talkbank_model::ErrorCollector::new();
    let outcome = parser.parse_word_fragment(s, 0, &errors);
    let collected = errors.into_vec();
    match outcome {
        talkbank_model::ParseOutcome::Parsed(w) if collected.is_empty() => Ok(Some(w)),
        _ => Err(collected),
    }
}

/// Run `f` with a thread-local `TreeSitterParser`.
///
/// `TreeSitterParser` is `!Send + !Sync` (uses `RefCell` internally)
/// and carries a grammar-init cost per construction, so keeping one
/// per thread is the standard pattern. The thread_local initializer
/// cannot recover from grammar-load failure; in that environmental
/// impossibility, first access on a thread will panic — documented in
/// the parent plan's risk list.
fn with_thread_local_parser<F, R>(f: F) -> R
where
    F: FnOnce(&talkbank_parser::TreeSitterParser) -> R,
{
    thread_local! {
        #[allow(clippy::expect_used)]
        static PARSER: talkbank_parser::TreeSitterParser =
            talkbank_parser::TreeSitterParser::new()
                .expect("tree-sitter-talkbank grammar must load");
    }
    PARSER.with(f)
}

impl ChatWordText {
    /// Borrows the word text for read-only use.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Structural-only fallible constructor using a caller-supplied parser.
    ///
    /// Accepts closed-set CHAT terminators and anything
    /// `parse_word_fragment` parses cleanly. For language-level rules
    /// (notably E220 digit policy) see
    /// [`try_from_lang_with_parser`][lang]. Hot paths should reuse one
    /// parser handle — `TreeSitterParser` is `!Send + !Sync` and has a
    /// non-trivial grammar-init cost per construction.
    ///
    /// [lang]: ChatWordText::try_from_lang_with_parser
    pub fn try_from_with_parser(
        s: &str,
        parser: &talkbank_parser::TreeSitterParser,
    ) -> Result<Self, Vec<talkbank_model::ParseError>> {
        structural_check(s, parser).map(|_| Self(s.to_owned()))
    }

    /// Language-aware fallible constructor using a caller-supplied parser.
    ///
    /// Runs [`try_from_with_parser`]'s structural check first, then
    /// applies `Word::validate` under a single-language
    /// `ValidationContext` — catching E220 (digits disallowed) for
    /// languages outside the digit-permitting set, plus any other
    /// word-level rule the model validator applies. Code-switching
    /// semantics live at the full-file layer; this boundary is
    /// deliberately single-language.
    ///
    /// [`try_from_with_parser`]: ChatWordText::try_from_with_parser
    pub fn try_from_lang_with_parser(
        s: &str,
        parser: &talkbank_parser::TreeSitterParser,
        lang: &talkbank_model::model::LanguageCode,
    ) -> Result<Self, Vec<talkbank_model::ParseError>> {
        use talkbank_model::validation::Validate;

        let Some(word) = structural_check(s, parser)? else {
            // Terminator: no further language-level checks apply.
            return Ok(Self(s.to_owned()));
        };

        // Single-language context triggers E220's single-candidate
        // path. Code-switching semantics live at the full-file layer.
        let ctx = talkbank_model::validation::ValidationContext::new()
            .with_default_language(lang.clone())
            .with_declared_languages(vec![lang.clone()])
            .with_tier_language(Some(lang.clone()));
        let errors = talkbank_model::ErrorCollector::new();
        word.validate(&ctx, &errors);
        let errs = errors.into_vec();
        if errs.is_empty() {
            Ok(Self(s.to_owned()))
        } else {
            Err(errs)
        }
    }

    /// Language-aware fallible constructor using a thread-local parser.
    ///
    /// Convenience wrapper over [`try_from_lang_with_parser`] for callers
    /// that don't already own a `TreeSitterParser`.
    pub fn try_from_lang(
        s: &str,
        lang: &talkbank_model::model::LanguageCode,
    ) -> Result<Self, Vec<talkbank_model::ParseError>> {
        with_thread_local_parser(|p| Self::try_from_lang_with_parser(s, p, lang))
    }
}

/// Fallible default constructor using a thread-local parser.
///
/// Constructs a `ChatWordText` from raw text, running the word fragment
/// parser to validate structural legality. The parser is kept in a
/// thread-local slot because `talkbank_parser::TreeSitterParser` is
/// `!Send + !Sync` (uses `RefCell` internally; see its own docstring)
/// and recreating it per call would be expensive.
///
/// For hot paths processing many words, prefer
/// [`ChatWordText::try_from_with_parser`] so the caller owns and reuses
/// one parser handle explicitly.
impl TryFrom<&str> for ChatWordText {
    type Error = Vec<talkbank_model::ParseError>;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        thread_local! {
            /// Per-thread parser handle. `TreeSitterParser::new()` can
            /// fail only on ABI mismatch between the grammar and the
            /// tree-sitter runtime — an environmental impossibility at
            /// runtime. If it ever does fail, construction here would
            /// panic on first use; migrating to a `try_init`-based
            /// handle would be a follow-up and is documented in the
            /// plan's risk list.
            #[allow(clippy::expect_used)]
            static PARSER: talkbank_parser::TreeSitterParser =
                talkbank_parser::TreeSitterParser::new()
                    .expect("tree-sitter-talkbank grammar must load");
        }
        PARSER.with(|p| Self::try_from_with_parser(s, p))
    }
}

impl fmt::Display for ChatWordText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for ChatWordText {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl PartialEq<&str> for ChatWordText {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

// ---------------------------------------------------------------------------
// Timing and speaker newtypes
// ---------------------------------------------------------------------------

/// Timestamp in seconds from an ASR provider (raw timing).
///
/// ASR providers report element boundaries in fractional seconds.
/// This newtype distinguishes provider timestamps from the millisecond
/// timings used internally by `AsrWord` (plain `i64`).
#[derive(Debug, Clone, Copy, Default, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
#[repr(transparent)]
pub struct AsrTimestampSecs(pub f64);

impl AsrTimestampSecs {
    /// Returns the inner `f64` value.
    pub fn as_f64(self) -> f64 {
        self.0
    }
}

impl PartialEq<f64> for AsrTimestampSecs {
    fn eq(&self, other: &f64) -> bool {
        self.0 == *other
    }
}

impl fmt::Display for AsrTimestampSecs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.3}s", self.0)
    }
}

/// Zero-based speaker index within a recording.
///
/// Maps to participant codes (`PAR`, `INV`, `SP0`, etc.) during CHAT
/// assembly in `transcript_from_asr_utterances()`.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
#[repr(transparent)]
pub struct SpeakerIndex(pub usize);

impl SpeakerIndex {
    /// Returns the inner `usize` value.
    pub fn as_usize(self) -> usize {
        self.0
    }
}

impl PartialEq<usize> for SpeakerIndex {
    fn eq(&self, other: &usize) -> bool {
        self.0 == *other
    }
}

impl fmt::Display for SpeakerIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serde_transparency() {
        let raw = AsrRawText::new("hello world");
        let json = serde_json::to_string(&raw).unwrap();
        assert_eq!(json, "\"hello world\"");
        let decoded: AsrRawText = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, raw);
    }

    #[test]
    fn normalized_map() {
        let text = AsrNormalizedText::new("5");
        let mapped = text.map(|t| t.replace('5', "five"));
        assert_eq!(mapped.as_str(), "five");
    }

    #[test]
    fn normalized_push_str() {
        let mut text = AsrNormalizedText::new("hello");
        text.push_str("-world");
        assert_eq!(text.as_str(), "hello-world");
    }

    #[test]
    fn chat_word_text_display() {
        let text = ChatWordText::try_from("(be)cause").expect("legal word");
        assert_eq!(format!("{text}"), "(be)cause");
    }

    #[test]
    fn timestamp_serde_roundtrip() {
        let ts = AsrTimestampSecs(1.234);
        let json = serde_json::to_string(&ts).unwrap();
        assert_eq!(json, "1.234");
        let decoded: AsrTimestampSecs = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, ts);
    }

    #[test]
    fn speaker_index_serde_roundtrip() {
        let idx = SpeakerIndex(3);
        let json = serde_json::to_string(&idx).unwrap();
        assert_eq!(json, "3");
        let decoded: SpeakerIndex = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, idx);
    }

    // -------------------------------------------------------------------
    // RED — Fundamental A (provenance chain soundness)
    //
    // `ChatWordText` is documented as "text ready for CHAT assembly via
    // `TreeSitterParser`" (module doc, `asr_types.rs:153-158`). That is a
    // postcondition. The current constructor (`pub fn new(s: impl
    // Into<String>) -> Self`, `asr_types.rs:167`) is infallible and
    // performs no validation — so the type documents a promise that the
    // code does not keep. CLAUDE.md rule 6a: `From<T>` / infallible
    // `new` constructors are forbidden on types whose construction can
    // fail; use `TryFrom` instead.
    //
    // These RED tests codify the invariant as a compile-runnable
    // specification. The stub helpers `try_chat_word` /
    // `try_chat_word_for_lang` are forward declarations of the API that
    // does not yet exist on `ChatWordText` itself. When the GREEN step
    // lands, their bodies migrate into `ChatWordText::try_from` /
    // `ChatWordText::try_from_lang` on the real type, and every caller
    // currently using `ChatWordText::new(raw_string)` becomes a type
    // error until threaded through. That cascade is the point.
    //
    // Empirical origin: c465e6e8-97c (2026-04-22), Rev.AI emitted
    // `"80%"`, `"20%"`, `"17-year-old"` into main-tier word content.
    // -------------------------------------------------------------------

    /// Thin adapter for the structural invariant tests.
    ///
    /// Delegates to `ChatWordText::try_from` (the real API introduced in
    /// Step 2 of the Fundamental A plan). The `Result`'s error type is
    /// narrowed to `String` so the test assertions stay readable; the
    /// real API returns `Vec<talkbank_model::ParseError>` and callers
    /// in production get full provenance.
    fn try_chat_word(s: &str) -> Result<ChatWordText, String> {
        ChatWordText::try_from(s).map_err(|errs| {
            errs.iter()
                .map(|e| e.message.clone())
                .collect::<Vec<_>>()
                .join("; ")
        })
    }

    /// Thin adapter for the language-aware invariant tests.
    ///
    /// For Step 2 this runs the structural half only (delegating to
    /// `ChatWordText::try_from`) and accepts everything language-wise.
    /// That is correct for structural invariants that hold in every
    /// language (e.g. `%` never permitted on the main tier), and the
    /// cross-language structural test exercises exactly that property.
    ///
    /// The E220 digit-policy half lands in Step 3; until then,
    /// digit-permitting languages (yue, zho, cmn, nan, hak, min, cym,
    /// vie, tha) and digit-rejecting ones (eng and most others) both
    /// round-trip through the same structural check, which means
    /// `rejects_digits_for_eng` stays RED — by design.
    fn try_chat_word_for_lang(s: &str, lang: &str) -> Result<ChatWordText, String> {
        let code = talkbank_model::model::LanguageCode::from(lang);
        ChatWordText::try_from_lang(s, &code).map_err(|errs| {
            errs.iter()
                .map(|e| e.message.clone())
                .collect::<Vec<_>>()
                .join("; ")
        })
    }

    #[test]
    fn red_fund_a_chat_word_text_rejects_percent_sign() {
        // `%` is the CHAT dependent-tier sigil. It cannot appear on the
        // main tier in any language — E316 "Unparsable content on main
        // tier" is what tree-sitter reports when it does.
        assert!(
            try_chat_word("80%").is_err(),
            "ChatWordText must not accept `%` — it is the dep-tier \
             sigil and produces E316 at parse. Fundamental A: the \
             provenance newtype's claimed postcondition must be \
             enforced at construction."
        );
        assert!(
            try_chat_word("%").is_err(),
            "bare `%` token must also be rejected"
        );
    }

    #[test]
    fn red_fund_a_chat_word_text_accepts_legal_word() {
        // Baseline (green once Fund A is implemented): legal main-tier
        // words pass.
        assert!(
            try_chat_word("hello").is_ok(),
            "`hello` is a legal main-tier word and must construct"
        );
        assert!(
            try_chat_word("(be)cause").is_ok(),
            "`(be)cause` is a legal main-tier word with an optional \
             section and must construct"
        );
    }

    #[test]
    fn red_fund_a_chat_word_text_rejects_digits_for_eng() {
        // E220: digit-bearing word content is illegal in eng. This
        // exercises the language-aware variant of the invariant.
        assert!(
            try_chat_word_for_lang("17-year-old", "eng").is_err(),
            "digit-bearing word `17-year-old` is illegal in eng \
             (E220); ChatWordText construction must fail for this \
             (text, lang) pair"
        );
        assert!(
            try_chat_word_for_lang("80", "eng").is_err(),
            "bare numeric `80` is illegal in eng (E220)"
        );
    }

    #[test]
    fn red_fund_a_chat_word_text_accepts_digits_for_yue() {
        // Digit-permitting languages (yue/zho/cmn/nan/hak/min/cym/vie/
        // tha) pass the digit rule. The structural invariant (no `%`
        // on main tier) still holds cross-language — see the next test.
        assert!(
            try_chat_word_for_lang("17-year-old", "yue").is_ok(),
            "digit-bearing word is legal in yue; ChatWordText \
             construction must succeed"
        );
        assert!(
            try_chat_word_for_lang("80", "yue").is_ok(),
            "bare numeric is legal in yue"
        );
    }

    #[test]
    fn red_fund_a_structural_invariants_are_language_independent() {
        // `%` cannot appear on the main tier in any language because
        // it is the CHAT dep-tier sigil — a structural rule of the
        // grammar, not a language policy. Construction must fail
        // regardless of the declared language.
        assert!(
            try_chat_word_for_lang("80%", "yue").is_err(),
            "`80%` is structurally invalid as a main-tier word in \
             any language, including yue"
        );
        assert!(
            try_chat_word_for_lang("%", "zho").is_err(),
            "bare `%` is structurally invalid in any language"
        );
    }
}
