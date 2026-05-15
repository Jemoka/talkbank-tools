//! AST types — mirrors talkbank-model structure.
//!
//! Focused on main tier for now. Will expand to full ChatFile.

use crate::token::Token;
use serde::Serialize;

/// A parsed main tier: *SPEAKER:\t tier_body
/// grammar.js: main_tier = seq(star, speaker, colon, tab, tier_body)
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MainTier<'a> {
    pub speaker: Token<'a>,
    pub tier_body: TierBody<'a>,
}

/// grammar.js: tier_body = seq(
///   optional(linkers),
///   optional(seq(langcode, whitespaces)),
///   contents,
///   utterance_end
/// )
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TierBody<'a> {
    pub linkers: Vec<Token<'a>>,
    pub langcode: Option<Token<'a>>,
    pub contents: Vec<ContentItem<'a>>,
    pub terminator: Option<Token<'a>>,
    pub postcodes: Vec<Token<'a>>,
    pub media_bullet: Option<Token<'a>>,
}

/// grammar.js: contents = repeat1(choice(whitespaces, content_item, separator, overlap_point))
/// Whitespace is not stored — structural only.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum ContentItem<'a> {
    /// A word with optional trailing annotations.
    Word(WordWithAnnotations<'a>),
    /// grammar.js: pause_token
    Pause(Token<'a>),
    /// Atomic annotation: [/], [!], [=text], etc.
    Annotation(Token<'a>),
    /// Separator: comma, semicolon, intonation contours, etc.
    Separator(Token<'a>),
    /// grammar.js: overlap_point
    OverlapPoint(Token<'a>),
    /// Retrace: word(s) followed by [/], [//], [///], [/-], or [/?]
    /// The retraced content is wrapped here; the corrected content follows.
    Retrace(Retrace<'a>),
    /// grammar.js: group_with_annotations = seq(<, contents, >, annotations)
    Group(Group<'a>),
    /// grammar.js: quotation = seq(left_double_quote, contents, right_double_quote)
    Quotation(Quotation<'a>),
    /// Bare event: &=description (no annotations)
    Event(Vec<Token<'a>>),
    /// Event with annotations: &=description [annotation1] [annotation2] ...
    /// grammar.js: nonword_with_optional_annotations wraps events.
    /// Retrace markers are dropped (not applicable to events).
    AnnotatedEvent {
        event: Token<'a>,
        annotations: Vec<ParsedAnnotation<'a>>,
    },
    /// Media bullet
    MediaBullet(Token<'a>),
    /// CA element or delimiter
    CaMarker(Token<'a>),
    /// Underline begin marker (\u0002\u0001)
    UnderlineBegin(Token<'a>),
    /// Underline end marker (\u0002\u0002)
    UnderlineEnd(Token<'a>),
    /// Other spoken event: &*SPK:word
    OtherSpokenEvent(Token<'a>),
    /// Phonological group: ‹ contents ›
    PhoGroup(Vec<ContentItem<'a>>),
    /// Sign group: 〔 contents 〕
    SinGroup(Vec<ContentItem<'a>>),
    /// Long feature begin: &{l=LABEL
    LongFeatureBegin(Token<'a>),
    /// Long feature end: &}l=LABEL
    LongFeatureEnd(Token<'a>),
    /// Nonvocal begin: &{n=LABEL
    NonvocalBegin(Token<'a>),
    /// Nonvocal end: &}n=LABEL
    NonvocalEnd(Token<'a>),
    /// Nonvocal simple (self-closing): &{n=LABEL}
    NonvocalSimple(Token<'a>),
    /// Standalone zero (0) — action without speech.
    /// grammar.js: nonword = choice(event, zero)
    /// With annotations: annotated_action; without: bare action.
    Action {
        zero: Token<'a>,
        annotations: Vec<ParsedAnnotation<'a>>,
    },
}

/// grammar.js: standalone_word = seq(optional(prefix|zero), word_body, optional(form_marker),
///   optional(word_lang_suffix), optional(pos_tag))
/// word_with_optional_annotations = seq(standalone_word, repeat(annotation))
///
/// Mirrors the model Word structure: category prefix, body content, suffix markers.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WordWithAnnotations<'a> {
    /// Category prefix: Zero (0), PrefixFiller (&-), PrefixNonword (&~), PrefixFragment (&+).
    pub category: Option<WordCategory>,
    /// Word body content — mirrors model WordContent.
    pub body: Vec<WordBodyItem<'a>>,
    /// Form marker suffix: tag-extracted content (e.g., "f", "z:grm"). None if absent.
    pub form_marker: Option<&'a str>,
    /// Language suffix. None if absent.
    pub lang: Option<ParsedLangSuffix<'a>>,
    /// POS tag: tag-extracted content (e.g., "n", "adj"). None if absent.
    pub pos_tag: Option<&'a str>,
    /// Trailing scoped annotations: [*], [= text], [/], [!], etc.
    pub annotations: Vec<ParsedAnnotation<'a>>,
    /// Raw text of the entire word, sliced directly from source.
    /// Eliminates the need for `source` in conversion — the AST is self-contained.
    pub raw_text: &'a str,
}

/// A parsed scoped annotation. Tag-extracted content — no delimiters.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum ParsedAnnotation<'a> {
    /// `[/]`, `[//]`, `[///]`, `[/-]` — retrace markers
    Retrace(RetraceKindParsed),
    /// `[!]` — stressing
    Stressing,
    /// `[!!]` — contrastive stressing
    ContrastiveStressing,
    /// `[?]` — uncertain
    Uncertain,
    /// `[e]` — exclude
    Exclude,
    /// `[* code]` — error marker. Content is the code (may be empty).
    Error(&'a str),
    /// `[<]` or `[<1]` — overlap precedes. Content is the optional index digit.
    OverlapPrecedes(&'a str),
    /// `[>]` or `[>1]` — overlap follows
    OverlapFollows(&'a str),
    /// `[= text]` — explanation
    Explanation(&'a str),
    /// `[=! text]` — paralinguistic
    Paralinguistic(&'a str),
    /// `[=? text]` — alternative
    Alternative(&'a str),
    /// `[% text]` — percent comment
    PercentComment(&'a str),
    /// `[: replacement words]` — replacement
    Replacement(&'a str),
    /// `[- lang]` — language code (on utterance, not word, but can appear in annotation position)
    Langcode(&'a str),
    /// `[+ code]` — postcode (rare in word annotation position)
    Postcode(&'a str),
}

impl ParsedAnnotation<'_> {
    /// Whether this annotation is a retrace marker.
    pub fn is_retrace(&self) -> bool {
        matches!(self, ParsedAnnotation::Retrace(_))
    }

    /// Extract retrace kind if this is a retrace annotation.
    pub fn retrace_kind(&self) -> Option<RetraceKindParsed> {
        match self {
            ParsedAnnotation::Retrace(k) => Some(*k),
            _ => None,
        }
    }
}

/// Category of a word, determined by its prefix token.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum WordCategory {
    /// `0word` — omitted word
    Omission,
    /// `&~word` — babbling/nonword
    Nonword,
    /// `&-word` — filler
    Filler,
    /// `&+word` — phonological fragment
    Fragment,
}

/// Parsed language suffix from `@s` tokens.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum ParsedLangSuffix<'a> {
    /// Bare `@s` — toggle shortcut
    Shortcut,
    /// `@s:eng` or `@s:eng+zho` or `@s:eng&spa` — carries the code(s)
    Explicit(&'a str),
}

/// A single item inside a word body. Mirrors model `WordContent`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum WordBodyItem<'a> {
    /// Plain text segment (e.g., "hello", "want")
    Text(&'a str),
    /// Shortened syllable — tag-extracted content (e.g., "be" from "(be)")
    Shortening(&'a str),
    /// Syllable lengthening (:, ::, :::) — count of colons
    Lengthening(u8),
    /// Compound marker (+)
    CompoundMarker,
    /// Stress marker (primary ˈ or secondary ˌ)
    Stress(StressKind),
    /// Overlap point (⌈, ⌉, ⌊, ⌋ with optional index)
    OverlapPoint(OverlapKind, &'a str),
    /// Syllable pause (^)
    SyllablePause,
    /// Clitic boundary (~)
    CliticBoundary,
    /// CA element (single symbol like ↑, ↓, ≠, etc.)
    CaElement(CaElementKind),
    /// CA delimiter (paired like °softer°, ∆faster∆, etc.)
    CaDelimiter(CaDelimiterKind),
}

/// Primary vs secondary stress.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum StressKind {
    Primary,
    Secondary,
}

/// Overlap point direction and position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum OverlapKind {
    TopBegin,
    TopEnd,
    BottomBegin,
    BottomEnd,
}

/// CA element types — one per symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum CaElementKind {
    BlockedSegments, // ≠
    Constriction,    // ∾
    Hardening,       // ☇
    HurriedStart,    // ⇗
    Inhalation,      // ∙
    LaughInWord,     // ꓸ
    PitchDown,       // ↓
    PitchReset,      // ↕
    PitchUp,         // ↑
    SuddenStop,      // ≋
}

/// CA delimiter types — paired markers that scope content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum CaDelimiterKind {
    Unsure,            // ⁇
    Precise,           // §
    Creaky,            // ⁎
    Softer,            // °
    SegmentRepetition, // ↫
    Faster,            // ∆
    Slower,            // ∇
    Whisper,           // ∬
    Singing,           // ∮
    LowPitch,          // ▁
    HighPitch,         // ▔
    Louder,            // ◉
    SmileVoice,        // ☺
    BreathyVoice,      // ♋
    Yawn,              // Ϋ
}

/// Retraced content: words the speaker said then corrected.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Retrace<'a> {
    /// The retraced content (words that were corrected).
    pub content: Vec<ContentItem<'a>>,
    /// The retrace kind.
    pub kind: RetraceKindParsed,
    /// Whether this retrace originated from a `<group> [/]` (angle brackets).
    pub is_group: bool,
    /// Non-retrace annotations that followed the retrace marker (e.g., `[?]` after `[/]`).
    /// In grammar.js, annotations attach to `word_with_optional_annotations`, so
    /// they belong to the retrace, not the word inside it.
    pub annotations: Vec<ParsedAnnotation<'a>>,
}

/// Retrace kind — matches grammar.js retrace variants exactly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum RetraceKindParsed {
    /// `[/]`
    Partial,
    /// `[//]`
    Complete,
    /// `[///]`
    Multiple,
    /// `[/-]`
    Reformulation,
}

/// grammar.js: group_with_annotations
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Group<'a> {
    pub contents: Vec<ContentItem<'a>>,
    pub annotations: Vec<ParsedAnnotation<'a>>,
}

/// grammar.js: quotation
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Quotation<'a> {
    pub contents: Vec<ContentItem<'a>>,
}

// ═══════════════════════════════════════════════════════════════
// Header ASTs
// ═══════════════════════════════════════════════════════════════

/// Parsed @ID header fields.
/// Mirrors talkbank_model::IDHeader.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct IdHeaderParsed<'a> {
    pub language: &'a str,
    pub corpus: &'a str,
    pub speaker: &'a str,
    pub age: &'a str,
    pub sex: &'a str,
    pub group: &'a str,
    pub ses: &'a str,
    pub role: &'a str,
    pub education: &'a str,
    pub custom_field: &'a str,
}

/// Parsed @Languages header.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LanguagesHeaderParsed<'a> {
    pub codes: Vec<&'a str>,
}

/// Parsed @Participants header.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ParticipantsHeaderParsed<'a> {
    pub entries: Vec<ParticipantEntryParsed<'a>>,
}

/// A single participant entry: SPK Name Role
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ParticipantEntryParsed<'a> {
    pub words: Vec<&'a str>,
}

/// Parsed @Media header.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MediaHeaderParsed<'a> {
    pub fields: Vec<&'a str>,
}

/// Parsed @Types header.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TypesHeaderParsed<'a> {
    pub raw: &'a str,
    pub design: &'a str,
    pub activity: &'a str,
    pub group: &'a str,
}

/// A generic header (prefix + content tokens).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HeaderParsed<'a> {
    pub prefix: Token<'a>,
    /// All content tokens (may be empty for @UTF8, @Begin, @End, etc.)
    pub content: Vec<Token<'a>>,
}

// ═══════════════════════════════════════════════════════════════
// Full file AST
// ═══════════════════════════════════════════════════════════════

/// A parsed CHAT file.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ChatFile<'a> {
    pub lines: Vec<Line<'a>>,
    /// Original source text — needed for lossless raw_text reconstruction via spans.
    pub source: &'a str,
}

/// A line in a CHAT file.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum Line<'a> {
    Header(HeaderParsed<'a>),
    Utterance(Box<Utterance<'a>>),
}

/// An utterance: main tier + dependent tiers.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Utterance<'a> {
    pub main_tier: MainTier<'a>,
    pub dependent_tiers: Vec<DependentTierParsed<'a>>,
}

/// A parsed dependent tier.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum DependentTierParsed<'a> {
    Mor(MorTier<'a>),
    Gra(GraTier<'a>),
    Pho(PhoTier<'a>),
    Mod(PhoTier<'a>),
    Sin(SinTierParsed<'a>),
    /// %wor tier: words with optional inline timing bullets.
    Wor {
        items: Vec<WorItemParsed<'a>>,
        terminator: Option<Token<'a>>,
    },
    /// Generic text tier (content is raw text segments + bullets).
    Text {
        prefix: Token<'a>,
        content: Vec<Token<'a>>,
    },
}

/// A parsed %wor item: word with optional timing bullet.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum WorItemParsed<'a> {
    /// A word with optional timing bullet.
    Word {
        word: WordWithAnnotations<'a>,
        bullet: Option<(u64, u64)>,
    },
    /// Separator (comma, tag marker, vocative marker).
    Separator(Token<'a>),
}

// ═══════════════════════════════════════════════════════════════
// %pho tier AST
// ═══════════════════════════════════════════════════════════════

/// Parsed %pho tier.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PhoTier<'a> {
    pub items: Vec<PhoItemParsed<'a>>,
    pub terminator: Option<Token<'a>>,
}

/// A parsed %pho item: either a single word or a ‹group› of words.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum PhoItemParsed<'a> {
    /// Single word (possibly compound with +)
    Word(PhoWordParsed<'a>),
    /// ‹grouped words›
    Group(Vec<PhoWordParsed<'a>>),
}

/// A phonological word (possibly compound with +).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PhoWordParsed<'a> {
    pub segments: Vec<&'a str>,
}

/// A parsed %sin tier — gesture/sign words with optional 〔groups〕.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SinTierParsed<'a> {
    pub items: Vec<SinItemParsed<'a>>,
}

/// A single %sin item: either a token or a 〔group〕.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum SinItemParsed<'a> {
    Token(&'a str),
    Group(Vec<&'a str>),
}

// ═══════════════════════════════════════════════════════════════
// Text tier AST (for %com, %act, %cod, %exp, etc.)
// ═══════════════════════════════════════════════════════════════

/// Parsed text tier content (text_with_bullets).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TextTierParsed<'a> {
    pub segments: Vec<TextTierSegment<'a>>,
}

/// A segment in a text tier.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum TextTierSegment<'a> {
    Text(&'a str),
    Bullet(Token<'a>),
    Pic(Token<'a>),
}

// ═══════════════════════════════════════════════════════════════
// %mor tier AST — mirrors talkbank_model::MorTier/Mor/MorWord
// ═══════════════════════════════════════════════════════════════

/// Parsed %mor tier.
/// grammar.js: mor_contents = seq(mor_content+, optional(terminator))
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MorTier<'a> {
    pub items: Vec<MorItem<'a>>,
    pub terminator: Option<Token<'a>>,
}

/// A single %mor item: main word + optional post-clitics.
/// grammar.js: mor_content = seq(mor_word, repeat(seq(tilde, mor_word)))
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MorItem<'a> {
    pub main: MorWordParsed<'a>,
    pub post_clitics: Vec<MorWordParsed<'a>>,
}

/// A parsed %mor word: POS, lemma, features.
/// Extracted from a single MorWord rich token.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MorWordParsed<'a> {
    /// Part-of-speech tag (e.g., "verb", "pro:sub")
    pub pos: &'a str,
    /// Lemma/stem (e.g., "want", "I")
    pub lemma: &'a str,
    /// Feature values (e.g., ["Fin", "Ind", "Pres"])
    pub features: Vec<&'a str>,
}

// ═══════════════════════════════════════════════════════════════
// %gra tier AST — mirrors talkbank_model::GraTier/GrammaticalRelation
// ═══════════════════════════════════════════════════════════════

/// Parsed %gra tier.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GraTier<'a> {
    pub relations: Vec<GraRelationParsed<'a>>,
}

/// A parsed %gra relation: index, head, relation name.
/// Extracted from a single GraRelation rich token.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GraRelationParsed<'a> {
    pub index: &'a str,
    pub head: &'a str,
    pub relation: &'a str,
}
