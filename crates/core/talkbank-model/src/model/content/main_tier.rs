//! Main-tier model (`*SPK:` lines) for CHAT transcripts.
//!
//! This type combines speaker identity with shared utterance payload
//! ([`TierContent`]) and span metadata used during validation/reporting.
//!
//! # CHAT Format Structure
//!
//! ```text
//! *SPEAKER:\tCONTENT TERMINATOR [+ postcode] •timestamp•
//! ```
//!
//! The main tier consists of:
//! 1. Speaker code (e.g., `*CHI:`, `*MOT:`)
//! 2. Linkers (optional discourse markers like `++`, `+<`, `+^`)
//! 3. Language code (optional, for code-switching: `[- zho]`)
//! 4. Content (words, events, pauses, groups, etc.)
//! 5. Terminator (punctuation like `.`, `?`, `!`)
//! 6. Postcodes (optional utterance-level annotations)
//! 7. Bullet (optional media timestamp)
//!
//! # CHAT Format Examples
//!
//! ```text
//! *CHI: I want cookie .
//! *MOT: do you want a cookie ?
//! *CHI: ++ yeah !
//! *CHI: [- spa] hola ! [+ code-switching]
//! *CHI: I want &-uh cookie . •12345_23456•
//! ```
//!
//! # References
//!
//! - [Main Tier](https://talkbank.org/0info/manuals/CHAT.html#Main_Line)
//! - [Speaker Codes](https://talkbank.org/0info/manuals/CHAT.html#Speaker_Codes)
//! - [Utterance Linkers](https://talkbank.org/0info/manuals/CHAT.html#Utterance_Linkers)
//! - [Postcodes](https://talkbank.org/0info/manuals/CHAT.html#Postcodes)

use super::{
    Bullet, LanguageCode, Linker, Postcode, SpeakerCode, Terminator, TierContent, UtteranceContent,
    WriteChat,
};
use crate::alignment::helpers::{WordItem, walk_words};
use crate::model::content::word::Word;
use crate::model::dependent_tier::{WorItem, WorTier};
use crate::model::{BracketedContent, BracketedItem, ReplacedWord};
use crate::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation, Span};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift};

/// Typed representation of one main-tier utterance line.
///
/// Main tiers are the alignment anchor for dependent tiers (`%mor`, `%gra`,
/// `%pho`, `%wor`, ...).
///
/// # CHAT Format Examples
///
/// ```text
/// *CHI: I want cookie .
/// *MOT: do you want a cookie ?
/// *CHI: ++ yeah !
/// *FAT: [- spa] hola ! [+ code-switching]
/// *CHI: the dog &=barks ! •12345_23456•
/// ```
///
/// # Components
///
/// - **Speaker**: Three-letter code identifying the participant (e.g., `CHI`, `MOT`, `INV`)
/// - **Linkers**: Optional discourse markers (`++`, `+<`, `+^`) appearing after the speaker
/// - **Language code**: Optional marker for code-switching (`[- code]`)
/// - **Content**: Sequence of words, events, pauses, groups, and other elements
/// - **Terminator**: Punctuation (`.`, `?`, `!`, `+...`, etc.) marking utterance end
/// - **Postcodes**: Optional annotations after terminator (`[+ text]`, `[+bch]`, etc.)
/// - **Bullet**: Optional media timestamp (`•start_end•`)
///
/// # References
///
/// - [Main Tier](https://talkbank.org/0info/manuals/CHAT.html#Main_Line)
/// - [Speaker Codes](https://talkbank.org/0info/manuals/CHAT.html#Speaker_Codes)
/// - [Utterance Linkers](https://talkbank.org/0info/manuals/CHAT.html#Utterance_Linkers)
/// - [Postcodes](https://talkbank.org/0info/manuals/CHAT.html#Postcodes)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
pub struct MainTier {
    /// Speaker code (for example `CHI`, `MOT`, `INV`).
    pub speaker: SpeakerCode,
    /// Shared tier payload (content, markers, terminator, postcodes, bullet).
    pub content: TierContent,
    /// Source span for the full main-tier line.
    #[serde(skip)]
    #[schemars(skip)]
    pub span: Span,
    /// Source span for speaker token only.
    #[serde(skip)]
    #[schemars(skip)]
    pub speaker_span: Span,
}

impl MainTier {
    /// Builds a main tier from required fields with empty optional components.
    pub fn new(
        speaker: impl Into<SpeakerCode>,
        content: Vec<UtteranceContent>,
        terminator: impl Into<Option<Terminator>>,
    ) -> Self {
        Self {
            speaker: speaker.into(),
            content: TierContent::with_all(
                Vec::new(),
                None,
                content,
                terminator.into(),
                Vec::new(),
                None,
            ),
            span: Span::DUMMY,
            speaker_span: Span::DUMMY,
        }
    }

    /// Sets full-line source span metadata.
    pub fn with_span(mut self, span: Span) -> Self {
        self.span = span;
        self
    }

    /// Sets speaker-token source span metadata.
    pub fn with_speaker_span(mut self, speaker_span: Span) -> Self {
        self.speaker_span = speaker_span;
        self
    }

    /// Replaces linker list.
    pub fn with_linkers(mut self, linkers: Vec<Linker>) -> Self {
        self.content = self.content.with_linkers(linkers);
        self
    }

    /// Appends one linker.
    pub fn with_linker(mut self, linker: Linker) -> Self {
        self.content.linkers.0.push(linker);
        self
    }

    /// Sets utterance-scoped language code (`[- code]`).
    pub fn with_language_code(mut self, language_code: impl Into<LanguageCode>) -> Self {
        self.content = self.content.with_language_code(language_code.into());
        self
    }

    /// Return the utterance-level language that would replace whole-tier
    /// per-word `@s` markers, if any.
    ///
    /// This is the detection seam behind E255 and fix-up tooling such as
    /// `chatter debug fix-s`: if every `%mor`-bearing lexical item resolves to
    /// the same non-default language override, the utterance should be written
    /// as `[- LANG] ...` instead of tagging each word individually.
    pub fn whole_utterance_language_switch_target(
        &self,
        default_language: Option<&LanguageCode>,
        declared_languages: &[LanguageCode],
    ) -> Option<LanguageCode> {
        whole_utterance_language_switch_target(self, default_language, declared_languages)
    }

    /// Replaces postcode list.
    pub fn with_postcodes(mut self, postcodes: Vec<Postcode>) -> Self {
        self.content = self.content.with_postcodes(postcodes);
        self
    }

    /// Appends one postcode.
    pub fn with_postcode(mut self, postcode: Postcode) -> Self {
        self.content.postcodes.0.push(postcode);
        self
    }

    /// Sets utterance-level terminal bullet.
    pub fn with_bullet(mut self, bullet: Bullet) -> Self {
        self.content = self.content.with_bullet(bullet);
        self
    }

    /// Sets source span for content region (portion after speaker token).
    pub fn with_content_span(mut self, content_span: Span) -> Self {
        self.content = self.content.with_content_span(content_span);
        self
    }

    /// Serialize this main tier to an owned CHAT string.
    pub fn to_chat(&self) -> String {
        let mut s = String::new();
        let _ = self.write_chat(&mut s);
        s
    }

    /// Find the first context-dependent CA omission span in this main tier.
    ///
    /// Fragment parsers use this to reject parenthesized omission shorthand
    /// when they do not have file-level `@Options: CA` context. This matches
    /// both explicit `CAOmission` words and standalone shortening-only forms.
    pub fn find_context_dependent_ca_omission_span(&self) -> Option<Span> {
        fn word_requires_ca_file_context(word: &Word) -> bool {
            let standalone_shortening = word.content.len() == 1
                && matches!(word.content[0], crate::model::WordContent::Shortening(_));

            matches!(word.category, Some(crate::model::WordCategory::CAOmission))
                || standalone_shortening
        }

        let mut span = None;
        walk_words(&self.content.content, None, &mut |leaf| match leaf {
            WordItem::Word(word) if span.is_none() && word_requires_ca_file_context(word) => {
                span = Some(word.span);
            }
            WordItem::ReplacedWord(replaced) if span.is_none() => {
                if word_requires_ca_file_context(&replaced.word) {
                    span = Some(replaced.word.span);
                } else {
                    span = replaced
                        .replacement
                        .words
                        .iter()
                        .find(|word| word_requires_ca_file_context(word))
                        .map(|word| word.span);
                }
            }
            WordItem::Word(_) | WordItem::ReplacedWord(_) | WordItem::Separator(_) => {}
        });

        span
    }

    /// Generate a flat %wor tier from embedded timing alignment stored on words.
    ///
    /// Walks the main tier tree, extracting each alignable word's cleaned_text
    /// and timing into a flat `Vec<WorItem>`. Each word's `inline_bullet`
    /// is preserved from the main tier word. Tag-marker separators (comma,
    /// tag, vocative) are emitted as `WorItem::Separator`.
    ///
    /// # Eye Candy: Word Text is Display-Only
    ///
    /// This function copies `cleaned_text` from main tier words to %wor tier
    /// words as "eye candy" (human-readable display text). **This text is never
    /// reparsed or used for processing** — it exists solely for:
    /// - Human readability when viewing CHAT files
    /// - Error message formatting
    /// - CHAT format serialization compliance
    ///
    /// **What matters**: The `inline_bullet` timing data, which is also copied
    /// and contains the actual timing information (start_ms, end_ms) used for
    /// all timing operations.
    ///
    /// We could equally well copy `raw_text`, use placeholders, or indices —
    /// the choice is purely for human readability, not processing correctness.
    ///
    /// See: `WorTier` documentation and `docs/wor-tier-text-audit.md`.
    pub fn generate_wor_tier(&self) -> WorTier {
        let mut items: Vec<crate::model::dependent_tier::WorItem> = Vec::new();
        collect_wor_items_content(&self.content.content, &mut items);

        WorTier {
            language_code: self.content.language_code.clone(),
            items,
            terminator: self.content.terminator.clone(),
            // %wor should not carry the utterance-level bullet — that belongs
            // on the main tier only.  Word-level timing lives in each
            // WorItem's inline_bullet field.
            bullet: None,
            span: Span::DUMMY,
        }
    }
}

/// Collect flat WorItems from main tier content for %wor generation.
///
/// `%wor` generation is almost leaf-local, but replaced-word handling and
/// separator emission differ from the generic walkers. We therefore recurse
/// explicitly instead of using `walk_words()`.
fn collect_wor_items_content(content: &[UtteranceContent], out: &mut Vec<WorItem>) {
    for item in content {
        collect_wor_item(item, false, out);
    }
}

fn collect_wor_item(item: &UtteranceContent, in_retrace: bool, out: &mut Vec<WorItem>) {
    use crate::alignment::helpers::{counts_for_tier_in_context, is_tag_marker_separator};

    match item {
        UtteranceContent::Word(word) => {
            if counts_for_tier_in_context(word, crate::alignment::TierDomain::Wor, in_retrace) {
                out.push(WorItem::Word(Box::new(wor_word_from_main(word))));
            }
        }
        UtteranceContent::AnnotatedWord(annotated) => {
            if counts_for_tier_in_context(
                &annotated.inner,
                crate::alignment::TierDomain::Wor,
                in_retrace,
            ) {
                out.push(WorItem::Word(Box::new(wor_word_from_main(
                    &annotated.inner,
                ))));
            }
        }
        UtteranceContent::ReplacedWord(replaced) => {
            collect_wor_replaced_word(replaced, in_retrace, out);
        }
        UtteranceContent::Group(group) => {
            collect_wor_bracketed_content(&group.content, in_retrace, out);
        }
        UtteranceContent::AnnotatedGroup(annotated) => {
            collect_wor_bracketed_content(&annotated.inner.content, in_retrace, out);
        }
        UtteranceContent::PhoGroup(pho) => {
            collect_wor_bracketed_content(&pho.content, in_retrace, out);
        }
        UtteranceContent::SinGroup(sin) => {
            collect_wor_bracketed_content(&sin.content, in_retrace, out);
        }
        UtteranceContent::Quotation(quotation) => {
            collect_wor_bracketed_content(&quotation.content, in_retrace, out);
        }
        UtteranceContent::Retrace(retrace) => {
            collect_wor_bracketed_content(&retrace.content, true, out);
        }
        UtteranceContent::Separator(sep) => {
            if is_tag_marker_separator(sep) {
                out.push(WorItem::Separator {
                    text: sep.to_chat_string(),
                    span: sep.span(),
                });
            }
        }
        UtteranceContent::Event(_)
        | UtteranceContent::AnnotatedEvent(_)
        | UtteranceContent::Pause(_)
        | UtteranceContent::AnnotatedAction(_)
        | UtteranceContent::Freecode(_)
        | UtteranceContent::OverlapPoint(_)
        | UtteranceContent::InternalBullet(_)
        | UtteranceContent::LongFeatureBegin(_)
        | UtteranceContent::LongFeatureEnd(_)
        | UtteranceContent::UnderlineBegin(_)
        | UtteranceContent::UnderlineEnd(_)
        | UtteranceContent::NonvocalBegin(_)
        | UtteranceContent::NonvocalEnd(_)
        | UtteranceContent::NonvocalSimple(_)
        | UtteranceContent::OtherSpokenEvent(_) => {}
    }
}

fn collect_wor_bracketed_content(
    content: &BracketedContent,
    in_retrace: bool,
    out: &mut Vec<WorItem>,
) {
    for item in &content.content {
        collect_wor_bracketed_item(item, in_retrace, out);
    }
}

fn collect_wor_bracketed_item(item: &BracketedItem, in_retrace: bool, out: &mut Vec<WorItem>) {
    use crate::alignment::helpers::{counts_for_tier_in_context, is_tag_marker_separator};

    match item {
        BracketedItem::Word(word) => {
            if counts_for_tier_in_context(word, crate::alignment::TierDomain::Wor, in_retrace) {
                out.push(WorItem::Word(Box::new(wor_word_from_main(word))));
            }
        }
        BracketedItem::AnnotatedWord(annotated) => {
            if counts_for_tier_in_context(
                &annotated.inner,
                crate::alignment::TierDomain::Wor,
                in_retrace,
            ) {
                out.push(WorItem::Word(Box::new(wor_word_from_main(
                    &annotated.inner,
                ))));
            }
        }
        BracketedItem::ReplacedWord(replaced) => {
            collect_wor_replaced_word(replaced, in_retrace, out);
        }
        BracketedItem::AnnotatedGroup(annotated) => {
            collect_wor_bracketed_content(&annotated.inner.content, in_retrace, out);
        }
        BracketedItem::PhoGroup(pho) => {
            collect_wor_bracketed_content(&pho.content, in_retrace, out);
        }
        BracketedItem::SinGroup(sin) => {
            collect_wor_bracketed_content(&sin.content, in_retrace, out);
        }
        BracketedItem::Quotation(quotation) => {
            collect_wor_bracketed_content(&quotation.content, in_retrace, out);
        }
        BracketedItem::Retrace(retrace) => {
            collect_wor_bracketed_content(&retrace.content, true, out);
        }
        BracketedItem::Separator(sep) => {
            if is_tag_marker_separator(sep) {
                out.push(WorItem::Separator {
                    text: sep.to_chat_string(),
                    span: sep.span(),
                });
            }
        }
        BracketedItem::Event(_)
        | BracketedItem::AnnotatedEvent(_)
        | BracketedItem::Pause(_)
        | BracketedItem::Action(_)
        | BracketedItem::AnnotatedAction(_)
        | BracketedItem::OverlapPoint(_)
        | BracketedItem::InternalBullet(_)
        | BracketedItem::Freecode(_)
        | BracketedItem::LongFeatureBegin(_)
        | BracketedItem::LongFeatureEnd(_)
        | BracketedItem::UnderlineBegin(_)
        | BracketedItem::UnderlineEnd(_)
        | BracketedItem::NonvocalBegin(_)
        | BracketedItem::NonvocalEnd(_)
        | BracketedItem::NonvocalSimple(_)
        | BracketedItem::OtherSpokenEvent(_) => {}
    }
}

fn whole_utterance_language_switch_target(
    main_tier: &MainTier,
    default_language: Option<&LanguageCode>,
    declared_languages: &[LanguageCode],
) -> Option<LanguageCode> {
    let tier_language = main_tier
        .content
        .language_code
        .as_ref()
        .or(default_language);

    // Collect ALL word-bearing items (including fillers `&~`, `&-`,
    // `&+` and other nonword tokens), not just MOR-bearing ones. The
    // `[- LANG]` precode declares whole-utterance language scope, so
    // the predicate must verify every word the speaker actually
    // uttered — fillers and nonwords included — resolves to the same
    // language. Restricting to MOR-domain (the prior bug) skipped
    // tonal Cantonese fillers like `&~dang3` and silently classified
    // utterances as monolingual, producing E220 violations after the
    // rewrite (see `~/talkbank/docs/fix-s-overrewrite-assessment-2026-05-06.md`).
    let mut words = Vec::new();
    collect_main_tier_words_for_language_check(&main_tier.content.content, &mut words);
    if words.is_empty() {
        return None;
    }

    let mut target_lang: Option<LanguageCode> = None;
    for word in words {
        if word.lang.is_none() {
            return None;
        }

        let outcome =
            crate::validation::resolve_word_language(word, tier_language, declared_languages);
        let resolved = match outcome.resolution {
            crate::validation::LanguageResolution::Single(code) => code,
            _ => return None,
        };

        if let Some(existing) = &target_lang {
            if existing != &resolved {
                return None;
            }
        } else {
            target_lang = Some(resolved);
        }
    }

    target_lang
}

/// Collect every word-bearing item from main-tier content for the
/// `[- LANG]` predicate. Includes fillers (`&~`, `&-`, `&+`),
/// nonwords, AND retrace content — every word the speaker uttered
/// counts toward the whole-utterance language scope, including
/// false-start material the speaker then corrected. The predicate's
/// per-word `lang.is_none() → return None` guard then refuses to
/// auto-promote to `[- LANG]` whenever ANY uttered word lacks an
/// explicit language attribution.
fn collect_main_tier_words_for_language_check<'a>(
    content: &'a [UtteranceContent],
    out: &mut Vec<&'a Word>,
) {
    for item in content {
        collect_main_tier_word_item(item, out);
    }
}

fn collect_main_tier_word_item<'a>(item: &'a UtteranceContent, out: &mut Vec<&'a Word>) {
    match item {
        UtteranceContent::Word(word) => out.push(word),
        UtteranceContent::AnnotatedWord(annotated) => out.push(&annotated.inner),
        UtteranceContent::ReplacedWord(replaced) => {
            out.push(&replaced.word);
            for word in &replaced.replacement.words {
                out.push(word);
            }
        }
        UtteranceContent::Group(group) => {
            collect_main_tier_bracketed_items(&group.content, out);
        }
        UtteranceContent::AnnotatedGroup(annotated) => {
            collect_main_tier_bracketed_items(&annotated.inner.content, out);
        }
        UtteranceContent::PhoGroup(pho) => {
            collect_main_tier_bracketed_items(&pho.content, out);
        }
        UtteranceContent::SinGroup(sin) => {
            collect_main_tier_bracketed_items(&sin.content, out);
        }
        UtteranceContent::Quotation(quotation) => {
            collect_main_tier_bracketed_items(&quotation.content, out);
        }
        UtteranceContent::Retrace(retrace) => {
            collect_main_tier_bracketed_items(&retrace.content, out);
        }
        // Non-word items: skip (they don't carry word-level @s markers).
        UtteranceContent::Separator(_)
        | UtteranceContent::Event(_)
        | UtteranceContent::AnnotatedEvent(_)
        | UtteranceContent::Pause(_)
        | UtteranceContent::AnnotatedAction(_)
        | UtteranceContent::Freecode(_)
        | UtteranceContent::OverlapPoint(_)
        | UtteranceContent::InternalBullet(_)
        | UtteranceContent::LongFeatureBegin(_)
        | UtteranceContent::LongFeatureEnd(_)
        | UtteranceContent::UnderlineBegin(_)
        | UtteranceContent::UnderlineEnd(_)
        | UtteranceContent::NonvocalBegin(_)
        | UtteranceContent::NonvocalEnd(_)
        | UtteranceContent::NonvocalSimple(_)
        | UtteranceContent::OtherSpokenEvent(_) => {}
    }
}

fn collect_main_tier_bracketed_items<'a>(content: &'a BracketedContent, out: &mut Vec<&'a Word>) {
    use crate::model::content::BracketedItem;
    for entry in &content.content {
        match entry {
            BracketedItem::Word(word) => out.push(word),
            BracketedItem::AnnotatedWord(annotated) => out.push(&annotated.inner),
            BracketedItem::ReplacedWord(replaced) => {
                out.push(&replaced.word);
                for word in &replaced.replacement.words {
                    out.push(word);
                }
            }
            BracketedItem::AnnotatedGroup(annotated) => {
                collect_main_tier_bracketed_items(&annotated.inner.content, out);
            }
            BracketedItem::PhoGroup(pho) => {
                collect_main_tier_bracketed_items(&pho.content, out);
            }
            BracketedItem::SinGroup(sin) => {
                collect_main_tier_bracketed_items(&sin.content, out);
            }
            BracketedItem::Quotation(quotation) => {
                collect_main_tier_bracketed_items(&quotation.content, out);
            }
            BracketedItem::Retrace(retrace) => {
                collect_main_tier_bracketed_items(&retrace.content, out);
            }
            // Non-word items: skip.
            BracketedItem::Event(_)
            | BracketedItem::AnnotatedEvent(_)
            | BracketedItem::Pause(_)
            | BracketedItem::Action(_)
            | BracketedItem::AnnotatedAction(_)
            | BracketedItem::OverlapPoint(_)
            | BracketedItem::Separator(_)
            | BracketedItem::InternalBullet(_)
            | BracketedItem::Freecode(_)
            | BracketedItem::LongFeatureBegin(_)
            | BracketedItem::LongFeatureEnd(_)
            | BracketedItem::UnderlineBegin(_)
            | BracketedItem::UnderlineEnd(_)
            | BracketedItem::NonvocalBegin(_)
            | BracketedItem::NonvocalEnd(_)
            | BracketedItem::NonvocalSimple(_)
            | BracketedItem::OtherSpokenEvent(_) => {}
        }
    }
}

fn collect_wor_replaced_word(entry: &ReplacedWord, in_retrace: bool, out: &mut Vec<WorItem>) {
    use crate::alignment::helpers::counts_for_tier_in_context;

    if counts_for_tier_in_context(&entry.word, crate::alignment::TierDomain::Wor, in_retrace) {
        out.push(WorItem::Word(Box::new(wor_word_from_main(&entry.word))));
    }
}

/// Build a `%wor` word from a main-tier word, preserving inline timing.
///
/// The `%wor` word gets `cleaned_text` as both raw and cleaned (since `%wor`
/// serializes cleaned_text), and inherits the inline_bullet directly.
///
/// # Eye Candy: Word Text is Display-Only
///
/// **IMPORTANT**: The word text we copy here is "eye candy" — it's never
/// reparsed or used for processing. We could equally well use:
/// - `cleaned_text` (current choice - human readable, matches TextGrid)
/// - `raw_text` (preserves CHAT markers like `:`, `@c`)
/// - Placeholders (`_`, `w0`, etc.)
///
/// **Current choice**: We use `cleaned_text` for human readability and
/// consistency with TextGrid export, but this is a **convention**, not a
/// requirement. The text is write-only from a processing perspective.
///
/// **What matters**: The `inline_bullet` field, which contains the actual
/// timing data (start_ms, end_ms) used for all timing operations.
///
/// See: `WorTier` documentation and `docs/wor-tier-text-audit.md` for details.
fn wor_word_from_main(word: &Word) -> Word {
    // Copy cleaned_text as "eye candy" (convention: human-readable display)
    let cleaned = word.cleaned_text();
    let mut w = Word::new_unchecked(cleaned, cleaned);

    // Copy timing data (this is the REAL data that actually matters)
    if let Some(ref bullet) = word.inline_bullet {
        w.inline_bullet = Some(bullet.clone());
    }
    w
}

impl WriteChat for MainTier {
    /// Serialize one main tier line as CHAT text.
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        w.write_char('*')?;
        self.speaker.write_chat(w)?;
        w.write_str(":\t")?;
        self.content.write_tier_content(w)?;
        Ok(())
    }
}

impl std::fmt::Display for MainTier {
    /// Formats this main tier as one canonical CHAT utterance line.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.write_chat(f)
    }
}

impl crate::validation::Validate for MainTier {
    /// Validates speaker IDs, tier content, and delegation to nested validators.
    fn validate(&self, context: &crate::validation::ValidationContext, errors: &impl ErrorSink) {
        use crate::validation::main_tier::{
            check_no_nested_quotations, check_no_pauses_in_pho_groups,
        };
        use crate::validation::retrace::check_retraces_have_content;

        // Validate speaker
        let speaker_str = self.speaker.as_str();

        // Check length
        if speaker_str.len() > 7 {
            errors.report(ParseError::new(
                ErrorCode::InvalidSpeaker,
                Severity::Error,
                SourceLocation::new(self.speaker_span),
                ErrorContext::new(speaker_str, self.speaker_span, "speaker"),
                format!(
                    "Speaker ID '{}' exceeds maximum length of 7 characters",
                    speaker_str
                ),
            ));
        }

        // Check for invalid characters
        if let Some(invalid_char) = crate::validation::has_invalid_speaker_chars(speaker_str) {
            errors.report(ParseError::new(
                ErrorCode::InvalidSpeaker,
                Severity::Error,
                SourceLocation::new(self.speaker_span),
                ErrorContext::new(speaker_str, self.speaker_span, "speaker"),
                format!(
                    "Speaker ID '{}' contains invalid character '{}'. Speaker IDs cannot contain colon (:) or whitespace",
                    speaker_str, invalid_char
                ),
            ));
        }

        // E308: Check if speaker is in participant list (unless speaker is "0" - unidentified)
        if !context.shared.participant_ids.is_empty()
            && !context.shared.participant_ids.contains(&self.speaker)
            && speaker_str != "0"
        {
            errors.report(
                ParseError::new(
                    ErrorCode::UndeclaredSpeaker,
                    Severity::Error,
                    SourceLocation::new(self.speaker_span),
                    ErrorContext::new(speaker_str, self.speaker_span, "speaker"),
                    format!("Speaker '{}' is not in the participant list", speaker_str),
                )
                .with_suggestion("Add this speaker to the @Participants header"),
            );

            let mut expected_speakers: Vec<String> = context
                .shared
                .participant_ids
                .iter()
                .map(|speaker| speaker.as_str().to_string())
                .collect();
            expected_speakers.sort();

            let warning_context =
                ErrorContext::new(speaker_str, self.speaker_span, "speaker_not_found")
                    .with_expected(expected_speakers);
            errors.report(
                ParseError::new(
                    ErrorCode::SpeakerNotFoundInParticipants,
                    Severity::Warning,
                    SourceLocation::new(self.speaker_span),
                    warning_context,
                    format!(
                        "Speaker '{}' not found in @Participants header",
                        speaker_str
                    ),
                )
                .with_suggestion(
                    "Add this speaker to @Participants, or correct the speaker code on the tier",
                ),
            );
        }

        let tier_content = &self.content;
        let content_items = &tier_content.content;

        // E305: missing terminator — main tier utterances must end with a
        // terminator unless CA mode is active.
        if tier_content.terminator.is_none() && !context.shared.ca_mode {
            errors.report(
                ParseError::new(
                    ErrorCode::MissingTerminator,
                    Severity::Error,
                    SourceLocation::new(self.span),
                    ErrorContext::new(self.speaker.as_str(), self.span, self.speaker.as_str()),
                    "Expected terminator not found",
                )
                .with_suggestion("Add a terminator at the end: Standard (. ? !), Interruption (+... +/. +//. +/? +//? +/??  +..? +\"/. +\". +.), or CA intonation (⇗ ↗ → ↘ ⇘ ≋ +≋ ≈ +≈)"),
            );
        }

        // Validate content using the content_span for precise error highlighting
        // The content_span points to the exact portion after the colon where content errors occur
        let content_context = if let Some(content_span) = tier_content.content_span {
            // Use content's own span for precise error highlighting
            context
                .clone()
                .with_field_span(content_span)
                .with_field_text("") // Empty text - span is used for location, not text extraction
        } else {
            // Fallback for backward compatibility (shouldn't happen after parser update)
            context
                .clone()
                .with_field_span(self.span)
                .with_field_text("")
        };
        tier_content.content.validate(&content_context, errors);

        // Determine tier-level language for word validation
        // Tier can override with language_code (e.g., *CHI@s:spa)
        let tier_language = tier_content
            .language_code
            .clone()
            .or_else(|| context.shared.default_language.clone());
        let word_context = context.clone().with_tier_language(tier_language);

        // Validate all content items with the appropriate language
        for content_item in content_items.iter() {
            match content_item {
                UtteranceContent::Word(word) => {
                    word.validate(&word_context, errors);
                }
                UtteranceContent::AnnotatedWord(annotated_word) => {
                    annotated_word.validate(&word_context, errors);
                }
                UtteranceContent::ReplacedWord(replaced_word) => {
                    replaced_word.validate(&word_context, errors);
                }
                // Future: Validate other content types (events, pauses, groups, etc.)
                _ => {}
            }
        }

        // Validate bullet if present
        if let Some(bullet) = &tier_content.bullet {
            crate::validation::check_bullet(bullet, errors);
        }

        // E370: Validate retraces are followed by content
        check_retraces_have_content(self, errors);

        // E371: Validate no pauses inside phonological groups
        check_no_pauses_in_pho_groups(self, errors);

        // E372: Validate no nested quotations
        check_no_nested_quotations(self, errors);

        if let Some(target_lang) = self.whole_utterance_language_switch_target(
            word_context.shared.default_language.as_ref(),
            &word_context.shared.declared_languages,
        ) {
            let span = tier_content.content_span.unwrap_or(self.span);
            errors.report(
                ParseError::new(
                    ErrorCode::WholeUtteranceLanguageSwitchShouldUsePrecode,
                    Severity::Error,
                    SourceLocation::new(span),
                    None,
                    format!(
                        "Whole-utterance language switch to '{}' should use utterance precode [- {}] instead of tagging every lexical word with @s",
                        target_lang.as_str(),
                        target_lang.as_str()
                    ),
                )
                .with_suggestion(format!(
                    "Rewrite the utterance as '[- {}] ...' and remove the per-word @s markers",
                    target_lang.as_str()
                )),
            );
        }

        // Style-level whitespace checks are intentionally disabled in core
        // validation to avoid false positives on valid reference CHAT corpora.
    }
}

#[cfg(test)]
mod tests {
    use crate::ErrorCollector;
    use crate::Span;
    use crate::model::{
        BracketedContent, BracketedItem, Bullet, Group, MainTier, ReplacedWord, Replacement,
        Terminator, UtteranceContent, Word, WordCategory, WordContent, WordLanguageMarker,
        WordShortening,
    };
    use crate::validation::ValidationContext;
    use crate::{ErrorCode, Validate};

    /// Generates wor tier produces flat words with timing.
    #[test]
    fn generate_wor_tier_produces_flat_words_with_timing() -> Result<(), String> {
        let mut timed = Word::simple("hello");
        timed.inline_bullet = Some(Bullet::new(100, 200));
        let plain = Word::simple("world");

        let main = MainTier::new(
            "CHI",
            vec![
                UtteranceContent::Word(Box::new(timed.clone())),
                UtteranceContent::Word(Box::new(plain.clone())),
            ],
            Terminator::Period { span: Span::DUMMY },
        );

        let wor = main.generate_wor_tier();
        let words: Vec<&Word> = wor.words().collect();
        assert_eq!(words.len(), 2);

        assert_eq!(words[0].cleaned_text(), "hello");
        match &words[0].inline_bullet {
            Some(b) => {
                assert_eq!(b.timing.start_ms, 100);
                assert_eq!(b.timing.end_ms, 200);
            }
            None => return Err("expected inline_bullet on first word".into()),
        }

        assert_eq!(words[1].cleaned_text(), "world");
        assert!(words[1].inline_bullet.is_none());
        Ok(())
    }

    /// Generates wor tier extracts words from groups.
    #[test]
    fn generate_wor_tier_extracts_words_from_groups() -> Result<(), String> {
        let mut timed = Word::simple("hello");
        timed.inline_bullet = Some(Bullet::new(50, 150));

        let group = Group::new(BracketedContent::new(vec![BracketedItem::Word(Box::new(
            timed.clone(),
        ))]));

        let main = MainTier::new(
            "CHI",
            vec![UtteranceContent::Group(group)],
            Terminator::Period { span: Span::DUMMY },
        );

        let wor = main.generate_wor_tier();
        let words: Vec<&Word> = wor.words().collect();
        assert_eq!(words.len(), 1);

        assert_eq!(words[0].cleaned_text(), "hello");
        match &words[0].inline_bullet {
            Some(b) => {
                assert_eq!(b.timing.start_ms, 50);
                assert_eq!(b.timing.end_ms, 150);
            }
            None => return Err("expected inline_bullet on grouped word".into()),
        }
        Ok(())
    }

    #[test]
    fn find_context_dependent_ca_omission_span_detects_grouped_ca_omission() {
        let omission_span = Span::from_usize(12, 18);
        let omission = Word::new_unchecked("(word)", "word")
            .with_category(WordCategory::CAOmission)
            .with_span(omission_span);
        let main = MainTier::new(
            "CHI",
            vec![UtteranceContent::Group(Group::new(BracketedContent::new(
                vec![BracketedItem::Word(Box::new(omission))],
            )))],
            Terminator::Period { span: Span::DUMMY },
        );

        assert_eq!(
            main.find_context_dependent_ca_omission_span(),
            Some(omission_span)
        );
    }

    #[test]
    fn find_context_dependent_ca_omission_span_detects_replacement_shortening() {
        let shortening_span = Span::from_usize(24, 30);
        let replacement_shortening = Word::new_unchecked("(lo)", "lo")
            .with_content(vec![WordContent::Shortening(
                WordShortening::new_unchecked("lo"),
            )])
            .with_span(shortening_span);
        let replaced = ReplacedWord::new(
            Word::simple("hello"),
            Replacement::new(vec![replacement_shortening]),
        );
        let main = MainTier::new(
            "CHI",
            vec![UtteranceContent::ReplacedWord(Box::new(replaced))],
            Terminator::Period { span: Span::DUMMY },
        );

        assert_eq!(
            main.find_context_dependent_ca_omission_span(),
            Some(shortening_span)
        );
    }

    #[test]
    fn validate_flags_all_at_s_single_language_utterance() {
        let mut hola = Word::simple("hola");
        hola.lang = Some(WordLanguageMarker::Shortcut);
        let mut amiga = Word::simple("amiga");
        amiga.lang = Some(WordLanguageMarker::Shortcut);

        let main = MainTier::new(
            "PAR",
            vec![
                UtteranceContent::Word(Box::new(hola)),
                UtteranceContent::Word(Box::new(amiga)),
            ],
            Terminator::Period { span: Span::DUMMY },
        );

        let context = ValidationContext::new()
            .with_default_language(crate::model::LanguageCode::new("eng"))
            .with_declared_languages(vec![
                crate::model::LanguageCode::new("eng"),
                crate::model::LanguageCode::new("spa"),
            ]);
        let errors = ErrorCollector::new();
        main.validate(&context, &errors);
        let error_vec = errors.into_vec();
        assert!(
            error_vec
                .iter()
                .any(|err| err.code == ErrorCode::WholeUtteranceLanguageSwitchShouldUsePrecode),
            "all-@s utterance should be rejected as a whole-utterance language switch"
        );
    }

    #[test]
    fn validate_allows_mixed_tagged_and_untagged_utterance() {
        let mut hola = Word::simple("hola");
        hola.lang = Some(WordLanguageMarker::Shortcut);
        let friend = Word::simple("friend");

        let main = MainTier::new(
            "PAR",
            vec![
                UtteranceContent::Word(Box::new(hola)),
                UtteranceContent::Word(Box::new(friend)),
            ],
            Terminator::Period { span: Span::DUMMY },
        );

        let context = ValidationContext::new()
            .with_default_language(crate::model::LanguageCode::new("eng"))
            .with_declared_languages(vec![
                crate::model::LanguageCode::new("eng"),
                crate::model::LanguageCode::new("spa"),
            ]);
        let errors = ErrorCollector::new();
        main.validate(&context, &errors);
        let error_vec = errors.into_vec();
        assert!(
            !error_vec
                .iter()
                .any(|err| err.code == ErrorCode::WholeUtteranceLanguageSwitchShouldUsePrecode),
            "utterances with untagged lexical words should stay on the normal word-level path"
        );
    }

    // ========================================================================
    // Regression tests for `whole_utterance_language_switch_target` —
    // the predicate behind `chatter debug fix-s` and validator E255.
    //
    // Bug history (2026-05-06): the predicate originally collected words
    // via the MOR-domain walker, which silently skipped fillers (`&~`,
    // `&-`, `&+`) and other nonwords. For utterances like
    // `*CHI: ballet@s , &~dang3 &~dang1 &~dang1 .`, the predicate saw
    // only `[ballet@s]` and concluded "monolingual eng," rewriting the
    // utterance to `[- eng] ballet , &~dang3 &~dang1 &~dang1 .` and
    // producing E220 ("digits not allowed in eng word") on the Cantonese
    // tone fillers downstream. The fix is to walk ALL word-bearing
    // items including fillers; the per-word `lang.is_none() → return
    // None` guard then catches every filler that lacks an explicit
    // `@s:LANG` marker.
    //
    // See `~/talkbank/docs/fix-s-overrewrite-assessment-2026-05-06.md`
    // for the full corpus-wide damage assessment.
    // ========================================================================

    /// GREEN baseline — clean all-`@s` utterance is correctly detected
    /// as a monolingual whole-utterance language switch. The `@s`
    /// shortcut resolves to "the OTHER declared language" relative to
    /// the tier-default language; here, default=`yue` makes `@s`
    /// resolve to `eng`.
    #[test]
    fn whole_utterance_target_returns_some_for_uniform_at_s_only_words() {
        use crate::model::LanguageCode;
        let main = MainTier::new(
            "CHI",
            vec![
                UtteranceContent::Word(Box::new(Word::simple("ballet").with_language_shortcut())),
                UtteranceContent::Word(Box::new(Word::simple("hello").with_language_shortcut())),
            ],
            Terminator::Period { span: Span::DUMMY },
        );

        let default = LanguageCode::new("yue");
        let declared = vec![LanguageCode::new("yue"), LanguageCode::new("eng")];
        let target = main.whole_utterance_language_switch_target(Some(&default), &declared);
        assert_eq!(
            target.as_ref().map(|c| c.as_str()),
            Some("eng"),
            "all-@s utterance with default=yue, declared=yue,eng must resolve to eng"
        );
    }

    /// RED → GREEN regression — the AliciaCan shape: one `@s` lexical
    /// word + several Cantonese tone-bearing nonword fillers (`&~dang3`,
    /// etc.). The fillers carry no explicit `@s:LANG` marker, so the
    /// predicate must return `None` (cannot confirm whole-utterance
    /// monolingual scope).
    ///
    /// Source: `Biling/YipMatthews/Can/AliciaCan/011016.cha:2611`
    /// — the smoking-gun case for the 2026-05-06 fix-s over-rewrite
    /// damage (440 files, 679 utterances).
    #[test]
    fn whole_utterance_target_returns_none_when_nonword_filler_lacks_lang_marker() {
        use crate::model::LanguageCode;
        let mut nonword = Word::new_unchecked("&~dang3", "dang3");
        nonword = nonword.with_category(WordCategory::Nonword);
        let main = MainTier::new(
            "CHI",
            vec![
                UtteranceContent::Word(Box::new(Word::simple("ballet").with_language_shortcut())),
                UtteranceContent::Word(Box::new(nonword)),
            ],
            Terminator::Period { span: Span::DUMMY },
        );

        let declared = vec![LanguageCode::new("yue"), LanguageCode::new("eng")];
        let target = main.whole_utterance_language_switch_target(None, &declared);
        assert_eq!(
            target, None,
            "a nonword filler without an @s:LANG marker must force whole-utterance \
             predicate to return None — otherwise fix-s rewrites the utterance to \
             [- LANG] and produces E220 on Cantonese tone fillers (AliciaCan bug)"
        );
    }

    /// Same invariant for `&-um`-style filler — the BA2-equivalent
    /// English filler — when paired with an `@s` lexical word in a
    /// non-English context. Without an explicit `@s:LANG` marker, the
    /// filler has language-null status and the predicate must refuse
    /// to declare whole-utterance scope.
    #[test]
    fn whole_utterance_target_returns_none_when_filler_lacks_lang_marker() {
        use crate::model::LanguageCode;
        let mut filler = Word::new_unchecked("&-um", "um");
        filler = filler.with_category(WordCategory::Filler);
        let main = MainTier::new(
            "CHI",
            vec![
                UtteranceContent::Word(Box::new(Word::simple("dile").with_language_shortcut())),
                UtteranceContent::Word(Box::new(filler)),
                UtteranceContent::Word(Box::new(Word::simple("a").with_language_shortcut())),
            ],
            Terminator::Period { span: Span::DUMMY },
        );

        let declared = vec![LanguageCode::new("eng"), LanguageCode::new("spa")];
        let target = main.whole_utterance_language_switch_target(None, &declared);
        assert_eq!(
            target, None,
            "an unmarked filler in an otherwise @s-tagged utterance must force the \
             predicate to return None — otherwise fix-s wrongly declares whole-utterance \
             scope despite the filler's unknown language status"
        );
    }

    /// Same invariant for `&+`-style phonological fragment.
    #[test]
    fn whole_utterance_target_returns_none_when_phonological_fragment_lacks_lang_marker() {
        use crate::model::LanguageCode;
        let mut frag = Word::new_unchecked("&+fr", "fr");
        frag = frag.with_category(WordCategory::PhonologicalFragment);
        let main = MainTier::new(
            "CHI",
            vec![
                UtteranceContent::Word(Box::new(Word::simple("hola").with_language_shortcut())),
                UtteranceContent::Word(Box::new(frag)),
            ],
            Terminator::Period { span: Span::DUMMY },
        );

        let declared = vec![LanguageCode::new("eng"), LanguageCode::new("spa")];
        let target = main.whole_utterance_language_switch_target(None, &declared);
        assert_eq!(
            target, None,
            "a phonological-fragment word without an @s:LANG marker must force the \
             predicate to return None"
        );
    }

    /// RED → GREEN regression — utterance with an unmarked filler
    /// INSIDE a retrace block. Mirrors wild patterns from
    /// `~/talkbank/still-have-error-7.log` like
    /// `*MAR: eh@s la@s &~s [///] el@s viernes@s ...` and
    /// `*WYN: people@s [//] (.) some@s ... &~sə [//] strange@s .`.
    ///
    /// Per CHAT semantics, retracted content is still uttered — even
    /// though the speaker self-corrected, the false-start words were
    /// spoken. Whole-utterance language scope therefore covers the
    /// retrace too. If a retracted filler/nonword has no `@s:LANG`
    /// marker, the predicate must return None (we cannot confirm
    /// monolingual scope).
    #[test]
    fn whole_utterance_target_returns_none_when_retraced_filler_lacks_lang_marker() {
        use crate::model::LanguageCode;
        use crate::model::content::Retrace;
        use crate::model::content::retrace::RetraceKind;

        // Inside the retrace: bare nonword `&~s` with no lang marker.
        let mut nonword = Word::new_unchecked("&~s", "s");
        nonword = nonword.with_category(WordCategory::Nonword);
        let retrace_content = BracketedContent::new(vec![BracketedItem::Word(Box::new(nonword))]);
        let retrace = Retrace::new(retrace_content, RetraceKind::Multiple);

        // Post-retrace: clean @s shortcut words.
        let main = MainTier::new(
            "MAR",
            vec![
                UtteranceContent::Retrace(Box::new(retrace)),
                UtteranceContent::Word(Box::new(Word::simple("el").with_language_shortcut())),
                UtteranceContent::Word(Box::new(Word::simple("viernes").with_language_shortcut())),
            ],
            Terminator::Period { span: Span::DUMMY },
        );

        let default = LanguageCode::new("eng");
        let declared = vec![LanguageCode::new("eng"), LanguageCode::new("spa")];
        let target = main.whole_utterance_language_switch_target(Some(&default), &declared);
        assert_eq!(
            target, None,
            "an unmarked nonword filler INSIDE a retrace must still force \
             the predicate to return None — retracted-but-uttered content \
             counts toward whole-utterance language scope"
        );
    }

    /// GREEN guard — if a filler IS explicitly tagged with the same
    /// `@s:LANG` as the lexical content, the predicate accepts the
    /// rewrite. Locks in that the fix doesn't over-reject — fillers
    /// that legitimately match the target language must still allow
    /// the precode promotion.
    #[test]
    fn whole_utterance_target_accepts_rewrite_when_filler_has_matching_explicit_lang() {
        use crate::model::LanguageCode;
        let lang = LanguageCode::new("eng");
        let mut filler = Word::new_unchecked("&-um", "um");
        filler = filler
            .with_category(WordCategory::Filler)
            .with_lang(lang.clone());
        let main = MainTier::new(
            "CHI",
            vec![
                UtteranceContent::Word(Box::new(Word::simple("hello").with_lang(lang.clone()))),
                UtteranceContent::Word(Box::new(filler)),
            ],
            Terminator::Period { span: Span::DUMMY },
        );

        let declared = vec![LanguageCode::new("yue"), lang.clone()];
        let target = main.whole_utterance_language_switch_target(None, &declared);
        assert_eq!(
            target.as_ref().map(|c| c.as_str()),
            Some("eng"),
            "filler with explicit matching @s:LANG must not block the rewrite"
        );
    }
}
