//! Core [`Word`] model and related helper types.
//!
//! This module defines the canonical typed representation of one parsed CHAT
//! word token, including structured content and optional marker fields.
//!
//! # CHAT Format References
//!
//! - [Words](https://talkbank.org/0info/manuals/CHAT.html#Words)
//! - [Special Form Markers](https://talkbank.org/0info/manuals/CHAT.html#SpecialForm_Marker)
//! - [Compounds](https://talkbank.org/0info/manuals/CHAT.html#Compounds)
//! - [Clitics](https://talkbank.org/0info/manuals/CHAT.html#Clitics)
//! - [Language Switching](https://talkbank.org/0info/manuals/CHAT.html#Language_Switching)
//! - [Media Linking](https://talkbank.org/0info/manuals/CHAT.html#Media_Linking)

use crate::model::WriteChat;
use serde::Deserialize;
use talkbank_derive::{SemanticEq, SpanShift};

use super::category::WordCategory;
use super::content::{WordContent, WordText};
use super::form::FormType;
use super::language::WordLanguageMarker;
use super::untranscribed::UntranscribedStatus;
use super::word_contents::WordContents;
use crate::model::{Bullet, LanguageCode};

/// A cached string value that is transparent to equality comparisons.
///
/// Used for memoization fields in derived-`PartialEq` structs. Two `CachedStr`
/// values always compare equal regardless of their contents, so the cache
/// never affects structural equality.
#[derive(Clone, Debug, Default)]
pub(super) struct CachedStr(pub(super) std::sync::OnceLock<smol_str::SmolStr>);

impl PartialEq for CachedStr {
    /// Always returns `true` so memoization never influences semantic equality.
    fn eq(&self, _other: &Self) -> bool {
        true // Derived from other fields; skip in equality
    }
}

/// A word in a CHAT transcript with optional markers and internal structure.
///
/// Words are the fundamental units of CHAT transcripts. They can be simple dictionary
/// forms, compounds, cliticizations, or include special markers indicating dialectal
/// variations, child-invented forms, and other linguistic phenomena.
///
/// # CHAT Format Examples
///
/// ```text
/// dog                 Simple word
/// ice+cream          Compound (legacy format)
/// gonna              Cliticization (going to)
/// sit(ting)          Shortening (incomplete word)
/// gumma@c            Child-invented form (@c marker)
/// younz@d            Dialect form (@d marker)
/// istenem@s:hu       Second language (@s:code marker)
/// bana:nas           Lengthened syllable (: marker)
/// ```
///
/// # Special Markers
///
/// - **Form markers** (`@b`, `@c`, `@d`, etc.) - See [`FormType`]
/// - **Language markers** (`@s`, `@s:code`) - See [`WordLanguageMarker`]
/// - **Category prefixes** (`&`, `&-`, etc.) - See [`WordCategory`]
/// - **Untranscribed status** - See [`UntranscribedStatus`]
///
/// # References
///
/// - [Words](https://talkbank.org/0info/manuals/CHAT.html#Words)
/// - [Special Form Markers](https://talkbank.org/0info/manuals/CHAT.html#SpecialForm_Marker)
/// - [Compounds](https://talkbank.org/0info/manuals/CHAT.html#Compounds)
/// - [Shortenings](https://talkbank.org/0info/manuals/CHAT.html#Shortenings)
/// - [Language Switching](https://talkbank.org/0info/manuals/CHAT.html#Language_Switching)
/// - [Media Linking](https://talkbank.org/0info/manuals/CHAT.html#Media_Linking)
///
/// # Alignment
///
/// Words participate in tier alignment with %mor, %pho, and other dependent tiers.
/// See [`crate::alignment`] for alignment algorithms.
#[derive(Clone, Debug, PartialEq, Deserialize, SemanticEq, SpanShift)]
pub struct Word {
    /// Source location (byte offsets in original input).
    #[serde(skip, default)]
    pub span: crate::Span,

    /// Unique identifier for tier alignment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub word_id: Option<smol_str::SmolStr>,

    /// Raw text exactly as it appeared in the input, including all markers.
    ///
    /// This preserves the original transcription with all CHAT-specific notation:
    /// - Lengthening markers (`:`)
    /// - Shortenings `(text)`
    /// - Stress markers (`ˈ`, `ˌ`)
    /// - CA elements and delimiters
    /// - Overlap points
    ///
    /// Use this for exact reproduction of the original transcript.
    pub(crate) raw_text: smol_str::SmolStr,

    /// Structured content breakdown.
    ///
    /// Uses a SmallVec-backed newtype - most words are simple (1 item)
    /// or compounds (2-3 items).
    pub content: WordContents,

    /// Word category prefix.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<WordCategory>,

    /// Form type marker (@a, @b, @c, @z:custom, etc.).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub form_type: Option<FormType>,

    /// Language-specific marker (@s or @s:code).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lang: Option<WordLanguageMarker>,

    /// Part-of-speech tag ($adj, $n, $v, etc.).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub part_of_speech: Option<smol_str::SmolStr>,

    /// Inline timing bullet parsed directly from %wor tier.
    ///
    /// When a %wor word has an inline bullet (e.g., `word •start_end•`), this field
    /// preserves the parsed bullet as a first-class entity with its span and timing data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inline_bullet: Option<Bullet>,

    /// Cached cleaned text (derived from `content`). Computed on first access.
    #[serde(skip, default)]
    #[semantic_eq(skip)]
    #[span_shift(skip)]
    pub(super) cached_cleaned_text: CachedStr,
}

impl Word {
    /// Builds a simple word where `raw_text == cleaned_text`.
    ///
    /// Use this for testing or when constructing words without complex markers.
    /// This ensures invariants are maintained for simple cases.
    pub fn simple(text: impl Into<smol_str::SmolStr>) -> Self {
        let text = text.into();
        Self::new_unchecked(text.clone(), text)
    }

    /// Builds a word without punctuation-guard checks.
    ///
    /// The `cleaned_text` parameter is used to populate the initial `content`
    /// as a single `Text` element. The word's cleaned text is always derived
    /// from `content` (via [`compute_cleaned_text`](Self::compute_cleaned_text)),
    /// not stored separately.
    pub fn new_unchecked(
        raw_text: impl Into<smol_str::SmolStr>,
        cleaned_text: impl Into<smol_str::SmolStr>,
    ) -> Self {
        let raw = raw_text.into();
        let cleaned = cleaned_text.into();

        Self {
            span: crate::Span::DUMMY,
            word_id: None,
            raw_text: raw,
            content: WordContents::new(smallvec::smallvec![WordContent::Text(
                WordText::new_unchecked(cleaned),
            )]),
            category: None,
            form_type: None,
            lang: None,
            part_of_speech: None,
            inline_bullet: None,
            cached_cleaned_text: CachedStr::default(),
        }
    }

    /// Returns source-faithful raw token text.
    ///
    /// This includes CHAT markers and punctuation exactly as parsed, making it
    /// suitable for roundtrip serialization and precise diagnostics.
    pub fn raw_text(&self) -> &str {
        &self.raw_text
    }

    /// Returns cleaned lexical text suitable for downstream NLP.
    ///
    /// Computed from `content` by concatenating `Text` and `Shortening` elements.
    /// Cached on first access via `OnceLock` — subsequent calls return a pointer.
    pub fn cleaned_text(&self) -> &str {
        self.cached_cleaned_text
            .0
            .get_or_init(|| smol_str::SmolStr::new(self.compute_cleaned_text()))
    }

    /// Returns untranscribed marker classification, if any.
    ///
    /// Computed from content: returns `Some(...)` when cleaned text is "xxx", "yyy", or "www".
    pub fn untranscribed(&self) -> Option<UntranscribedStatus> {
        self.compute_untranscribed()
    }

    /// Sets source span metadata for diagnostics.
    ///
    /// Span metadata does not affect semantic equality or serialized content.
    pub fn with_span(mut self, span: crate::Span) -> Self {
        self.span = span;
        self
    }

    /// Sets an optional stable identifier used by alignment/metadata pipelines.
    ///
    /// This ID is treated as auxiliary metadata and does not participate in text rendering.
    pub fn with_word_id(mut self, id: impl Into<smol_str::SmolStr>) -> Self {
        self.word_id = Some(id.into());
        self
    }

    /// Replaces structured internal word content.
    ///
    /// This does not rewrite `raw_text`; callers that need both fields updated
    /// should use [`replace_simple_text`](Self::replace_simple_text) or set both.
    pub fn with_content(mut self, content: impl Into<WordContents>) -> Self {
        self.content = content.into();
        self
    }

    /// Sets the optional category prefix (for example fillers/fragments).
    ///
    /// Category prefixes are rendered before lexical content in CHAT surface order.
    pub fn with_category(mut self, category: WordCategory) -> Self {
        self.category = Some(category);
        self
    }

    /// Sets the optional special-form suffix marker (`@...`).
    ///
    /// This captures CHAT form annotations such as dialectal or invented-word markers.
    pub fn with_form_type(mut self, form_type: FormType) -> Self {
        self.form_type = Some(form_type);
        self
    }

    /// Sets an explicit language override marker (`@s:code`).
    ///
    /// Explicit markers override tier/file defaults during language-aware validation.
    pub fn with_lang(mut self, lang: impl Into<LanguageCode>) -> Self {
        self.lang = Some(WordLanguageMarker::explicit(lang));
        self
    }

    /// Sets the bare language-shortcut marker (`@s`).
    ///
    /// Shortcut resolution depends on declared-language ordering in the active validation context.
    pub fn with_language_shortcut(mut self) -> Self {
        self.lang = Some(WordLanguageMarker::Shortcut);
        self
    }

    /// Sets an optional part-of-speech tag (`$...`).
    ///
    /// This field preserves transcript-provided tags and is not schema-normalized at construction.
    pub fn with_part_of_speech(mut self, pos: impl Into<smol_str::SmolStr>) -> Self {
        self.part_of_speech = Some(pos.into());
        self
    }

    /// Sets an inline timing bullet parsed from `%wor`.
    ///
    /// Inline bullets are timing metadata only; lexical text continues to come from `content`.
    pub fn with_inline_bullet(mut self, bullet: Bullet) -> Self {
        self.inline_bullet = Some(bullet);
        self
    }

    /// Compute cleaned text by concatenating Text and Shortening content elements.
    ///
    /// This is the authoritative way to derive the cleaned (NLP-ready) text from
    /// the word's structured `content`. It includes only lexical segments:
    /// - `WordContent::Text` — base graphemes
    /// - `WordContent::Shortening` — elided material restored (e.g., `som(e)` → `some`)
    ///
    /// All prosodic/analytical markers (lengthening, stress, CA elements, overlap
    /// points, compound markers, underline markers) are excluded.
    pub fn compute_cleaned_text(&self) -> String {
        let mut result = String::new();
        for item in &self.content {
            match item {
                WordContent::Text(t) => result.push_str(t.as_ref()),
                WordContent::Shortening(s) => result.push_str(s.as_ref()),
                _ => {}
            }
        }
        result
    }

    /// Compute untranscribed status from the word's content.
    ///
    /// Derives the untranscribed status by checking the cleaned text against
    /// the three canonical untranscribed markers: "xxx", "yyy", "www".
    /// The match is case-insensitive because legacy corpora use uppercase
    /// variants (e.g., "XXX") which are illegal (E241) but still represent
    /// untranscribed material. Without this, the morphotag pipeline would
    /// send uppercase variants to Stanza and produce spurious %mor entries.
    pub fn compute_untranscribed(&self) -> Option<UntranscribedStatus> {
        let cleaned = self.compute_cleaned_text();
        match cleaned.to_ascii_lowercase().as_str() {
            "xxx" => Some(UntranscribedStatus::Unintelligible),
            "yyy" => Some(UntranscribedStatus::Phonetic),
            "www" => Some(UntranscribedStatus::Untranscribed),
            _ => None,
        }
    }

    /// Replaces only `raw_text` for parser-recovery flows.
    ///
    /// This is used when parser error recovery needs to attach error fragments
    /// to a word's raw text without changing the structured content. The cleaned
    /// text is always derived from `content`, so it doesn't need updating.
    pub fn set_raw_text(&mut self, raw: impl Into<smol_str::SmolStr>) {
        self.raw_text = raw.into();
    }

    /// Replaces both raw text and content with one plain-text segment.
    ///
    /// Used by batchalign when substituting a word with a replacement string.
    /// Sets content to a single `Text` element and updates raw_text to match.
    pub fn replace_simple_text(&mut self, text: impl Into<smol_str::SmolStr>) {
        let text = text.into();
        self.raw_text = text.clone();
        self.content = WordContents::new(smallvec::smallvec![WordContent::Text(
            WordText::new_unchecked(text),
        )]);
    }

    /// Serializes this word to an owned CHAT string.
    ///
    /// This is a convenience wrapper over [`WriteChat`] for callers that need
    /// an owned value instead of writing into an existing buffer.
    pub fn to_chat(&self) -> String {
        let mut s = String::new();
        let _ = self.write_chat(&mut s);
        s
    }
}
