//! Token types emitted by the re2c lexer.
//!
//! Each variant corresponds to a grammar.js rule or token.
//! Names are taken directly from grammar.js where possible.

use serde::Serialize;
use strum::EnumDiscriminants;

/// Token returned by the lexer. Each variant carries
/// a borrowed slice of the source input (zero-copy).
#[derive(Clone, Debug, PartialEq, EnumDiscriminants, Serialize)]
pub enum Token<'a> {
    // ── Structure ───────────────────────────────────────────
    /// BOM (byte order mark) at start of file.
    BOM(&'a str),
    /// Newline(s): /[\r\n]+/
    Newline(&'a str),
    /// Continuation: /[\r\n]+\t/ (newline followed by tab)
    Continuation(&'a str),
    /// Whitespace: one or more spaces.
    Whitespace(&'a str),

    // ── Headers ─────────────────────────────────────────────
    /// Header prefix: @HeaderName:\t (includes the colon+tab for structured headers)
    /// Or just @HeaderName for no-content headers or catch-all.
    HeaderPrefix(&'a str),
    /// Header separator: ":\t" (only for unknown headers via HEADER_AFTER_NAME)
    HeaderSep(&'a str),
    /// Header content: free text after colon+tab
    HeaderContent(&'a str),

    /// `@Birth of SPK:\t` — carries tag-extracted speaker code.
    HeaderBirthOf(&'a str),
    /// `@Birthplace of SPK:\t` — carries tag-extracted speaker code.
    HeaderBirthplaceOf(&'a str),
    /// `@L1 of SPK:\t` — carries tag-extracted speaker code.
    HeaderL1Of(&'a str),

    // ── No-content headers (distinct tokens) ────────────────
    /// @UTF8 header (must be first line)
    HeaderUtf8(&'a str),
    /// @Begin header (marks start of main content)
    HeaderBegin(&'a str),
    /// @End header (marks end of file)
    HeaderEnd(&'a str),
    /// @Blank header
    HeaderBlank(&'a str),
    /// @New Episode header
    HeaderNewEpisode(&'a str),

    // ── Structured header content ───────────────────────────
    /// @ID rich token: 10 pipe-delimited fields extracted by tags.
    IdFields {
        language: &'a str,
        corpus: &'a str,
        speaker: &'a str,
        age: &'a str,
        sex: &'a str,
        group: &'a str,
        ses: &'a str,
        role: &'a str,
        education: &'a str,
        custom: &'a str,
    },

    /// @Types rich token: 3 comma-separated fields extracted by tags.
    TypesFields {
        design: &'a str,
        activity: &'a str,
        group: &'a str,
    },

    // ── Main tier ───────────────────────────────────────────
    /// Star: '*' (main tier marker)
    Star(&'a str),
    /// Speaker code: /[A-Za-z0-9_\'+\-]+/
    Speaker(&'a str),

    // ── Dependent tier ──────────────────────────────────────
    /// Tier prefix: %label (e.g., "%mor", "%gra", "%com")
    TierPrefix(&'a str),
    /// Tier separator: ":\t" (colon + tab, after tier label)
    TierSep(&'a str),

    // ── Terminators (grammar.js: terminator supertype) ──────
    /// grammar.js: period = '.'
    Period(&'a str),
    /// grammar.js: question = '?'
    Question(&'a str),
    /// grammar.js: exclamation = '!'
    Exclamation(&'a str),
    /// grammar.js: trailing_off = token(prec(10, '+...'))
    TrailingOff(&'a str),
    /// grammar.js: interruption = token(prec(10, '+/.'))
    Interruption(&'a str),
    /// grammar.js: self_interruption = token(prec(10, '+//.'))
    SelfInterruption(&'a str),
    /// grammar.js: interrupted_question = token(prec(10, '+/?'))
    InterruptedQuestion(&'a str),
    /// grammar.js: broken_question = token(prec(10, '+!?'))
    BrokenQuestion(&'a str),
    /// grammar.js: quoted_new_line = token(prec(10, '+"/.'))
    QuotedNewLine(&'a str),
    /// grammar.js: quoted_period_simple = token(prec(10, '+"..'))
    QuotedPeriodSimple(&'a str),
    /// grammar.js: self_interrupted_question = token(prec(10, '+//?'))
    SelfInterruptedQuestion(&'a str),
    /// grammar.js: trailing_off_question = token(prec(10, '+..?'))
    TrailingOffQuestion(&'a str),
    /// grammar.js: break_for_coding = token(prec(10, '+.'))
    BreakForCoding(&'a str),
    /// grammar.js: ca_no_break = token(prec(10, '≈'))
    CaNoBreak(&'a str),
    /// grammar.js: ca_technical_break = token(prec(10, '≋'))
    CaTechnicalBreak(&'a str),

    // ── Linkers ─────────────────────────────────────────────
    /// grammar.js: linker_lazy_overlap = token(prec(10, '+<'))
    LinkerLazyOverlap(&'a str),
    /// grammar.js: linker_quick_uptake = token(prec(10, '++'))
    LinkerQuickUptake(&'a str),
    /// grammar.js: linker_quick_uptake_overlap = token(prec(10, '+^'))
    LinkerQuickUptakeOverlap(&'a str),
    /// grammar.js: linker_quotation_follows = token(prec(10, '+"'))
    LinkerQuotationFollows(&'a str),
    /// grammar.js: linker_self_completion = token(prec(10, '+,'))
    LinkerSelfCompletion(&'a str),
    /// grammar.js: ca_no_break_linker = token(prec(10, '+≈'))
    CaNoBreakLinker(&'a str),
    /// grammar.js: ca_technical_break_linker = token(prec(10, '+≋'))
    CaTechnicalBreakLinker(&'a str),

    // ── Annotations (atomic brackets) ───────────────────────
    /// grammar.js: retrace_complete = token('[//]')
    RetraceComplete(&'a str),
    /// grammar.js: retrace_partial = token('[/]')
    RetracePartial(&'a str),
    /// grammar.js: retrace_multiple = token('[///]')
    RetraceMultiple(&'a str),
    /// grammar.js: retrace_reformulation = token('[/-]')
    RetraceReformulation(&'a str),
    /// grammar.js: scoped_stressing = token('[!]')
    ScopedStressing(&'a str),
    /// grammar.js: scoped_contrastive_stressing = token('[!!]')
    ScopedContrastiveStressing(&'a str),
    /// grammar.js: scoped_uncertain = token('[?]')
    ScopedUncertain(&'a str),
    /// grammar.js: exclude_marker = token('[e]')
    ExcludeMarker(&'a str),
    /// grammar.js: freecode = token(/\[\^ [^\]\r\n]+\]/)
    /// Rich token: [^ content] with tag marking content boundaries.
    Freecode(&'a str),
    /// grammar.js: ca_continuation_marker = token('[^c]')
    CaContinuationMarker(&'a str),
    /// grammar.js: error_marker_annotation = token(prec(8, /\[\*[^\]]*\]/))
    ErrorMarkerAnnotation(&'a str),

    // ── Annotations with content ────────────────────────────
    /// grammar.js: indexed_overlap_precedes = token(prec(8, /\[< ?[1-9]? ?\]/))
    OverlapPrecedes(&'a str),
    /// grammar.js: indexed_overlap_follows = token(prec(8, /\[> ?[1-9]? ?\]/))
    OverlapFollows(&'a str),
    /// [= text] — explanation
    ExplanationAnnotation(&'a str),
    /// [=! text] — paralinguistic
    ParaAnnotation(&'a str),
    /// [=? text] — alternative transcription
    AltAnnotation(&'a str),
    /// [% text] — percent annotation
    PercentAnnotation(&'a str),
    /// [+ code] — postcode
    Postcode(&'a str),
    /// [- lang] — language code
    Langcode(&'a str),
    /// [: replacement words] — replacement
    Replacement(&'a str),

    // ── Pauses ──────────────────────────────────────────────
    /// grammar.js: token(prec(10, '(...)'))
    PauseLong(&'a str),
    /// grammar.js: token(prec(10, '(..)'))
    PauseMedium(&'a str),
    /// grammar.js: token(prec(10, '(.)'))
    PauseShort(&'a str),
    /// grammar.js: token(prec(10, /\(\d+(?::\d+)?\.\d*\)/))
    PauseTimed(&'a str),

    // ── Word (rich token) ─────────────────────────────────
    /// A complete word matched by the lexer as a single token.
    /// Tags mark coarse field boundaries; the parser handles body internals.
    ///
    /// `raw_text` is the full word text from source (prefix + body + suffixes).
    /// `body` is the word body slice (parser splits into segments, compounds, CA, etc.).
    /// Suffix fields carry tag-extracted content (no delimiters).
    Word {
        /// Full word text (everything matched by the word rule).
        raw_text: &'a str,
        /// Category prefix if present: `&-`, `&~`, `&+`, or `0`.
        prefix: Option<&'a str>,
        /// Word body: text segments, shortenings, compounds, CA markers, etc.
        /// Parser handles fine-grained body parsing.
        body: &'a str,
        /// Form marker content: `f`, `z:grm`, etc. (without `@` prefix).
        form_marker: Option<&'a str>,
        /// Language suffix codes: `eng`, `eng+zho`, etc. (without `@s:` prefix).
        /// `None` means absent; bare `@s` shortcut carries `Some("")`.
        lang_suffix: Option<&'a str>,
        /// POS tag content: `n`, `adj`, etc. (without `$` prefix).
        pos_tag: Option<&'a str>,
    },

    // ── Word structure (sub-tokens for body parsing) ─────
    /// A word text segment (from WORD_SEGMENT regex).
    WordSegment(&'a str),
    /// grammar.js: shortening = seq('(', word_segment, ')')
    Shortening(&'a str),
    /// grammar.js: lengthening = token(prec(5, /:{1,}/))
    Lengthening(&'a str),
    // ── Stress markers (typed) ──
    /// ˈ primary stress (U+02C8)
    StressPrimary(&'a str),
    /// ˌ secondary stress (U+02CC)
    StressSecondary(&'a str),
    // ── Overlap points (typed — one variant per OverlapPointKind) ──
    /// ⌈ with optional digit
    OverlapTopBegin(&'a str),
    /// ⌉ with optional digit
    OverlapTopEnd(&'a str),
    /// ⌊ with optional digit
    OverlapBottomBegin(&'a str),
    /// ⌋ with optional digit
    OverlapBottomEnd(&'a str),
    /// grammar.js: syllable_pause = '^'
    SyllablePause(&'a str),
    /// grammar.js: tilde = '~'
    Tilde(&'a str),
    /// Compound marker: '+' between word segments.
    CompoundMarker(&'a str),
    /// grammar.js: underline_begin = token(prec(5, '\u0002\u0001'))
    UnderlineBegin(&'a str),
    /// grammar.js: underline_end = token(prec(5, '\u0002\u0002'))
    UnderlineEnd(&'a str),

    // ── Word prefixes ───────────────────────────────────────
    /// grammar.js: token('&-') — filler
    PrefixFiller(&'a str),
    /// grammar.js: token('&~') — nonword
    PrefixNonword(&'a str),
    /// grammar.js: token('&+') — fragment
    PrefixFragment(&'a str),
    /// grammar.js: event = seq(event_marker, event_segment+)
    /// Complete event token: the tag-extracted event description text
    /// (e.g., "laughs" from `&=laughs`, "clears:throat" from `&=clears:throat`).
    Event(&'a str),
    /// grammar.js: zero = token(prec(3, '0'))
    Zero(&'a str),

    /// Rich other_spoken_event: &*SPK:word
    /// Fields extracted by tags: speaker (t1..t2), text (t3..end).
    OtherSpokenEvent {
        speaker: &'a str,
        text: &'a str,
    },

    // ── Word suffixes ───────────────────────────────────────
    /// grammar.js: form_marker = token.immediate(/@[ubcdfgiklnopqtxz]|@(fp|ls|sas|si|sl|wp)/)
    FormMarker(&'a str),
    /// grammar.js: word_lang_suffix = token.immediate(/@s(?::[a-z]{2,3}(?:[+&][a-z]{2,3})*)? /)
    /// `@s` (bare shortcut) carries `None`; `@s:eng+zho` carries `Some("eng+zho")`.
    WordLangSuffix(Option<&'a str>),
    /// grammar.js: pos_tag = seq(token.immediate('$'), /[a-zA-Z:]+/)
    PosTag(&'a str),

    // ── Separators ──────────────────────────────────────────
    /// grammar.js: comma = ','
    Comma(&'a str),
    /// grammar.js: semicolon = ';'
    Semicolon(&'a str),
    /// grammar.js: colon = ':' (standalone separator, not word-internal lengthening)
    Colon(&'a str),
    /// grammar.js: tag_marker = '\u201E' (double low-9 quotation mark)
    TagMarker(&'a str),
    /// grammar.js: vocative_marker = '\u2021' (double dagger)
    VocativeMarker(&'a str),
    /// grammar.js: unmarked_ending = '\u221E' (infinity)
    UnmarkedEnding(&'a str),
    /// grammar.js: uptake_symbol = '\u2261' (identical to)
    UptakeSymbol(&'a str),

    // ── Intonation contours ─────────────────────────────────
    /// grammar.js: rising_to_high = '\u21D7'
    RisingToHigh(&'a str),
    /// grammar.js: rising_to_mid = '\u2197'
    RisingToMid(&'a str),
    /// grammar.js: level_pitch = '\u2192'
    LevelPitch(&'a str),
    /// grammar.js: falling_to_mid = '\u2198'
    FallingToMid(&'a str),
    /// grammar.js: falling_to_low = '\u21D8'
    FallingToLow(&'a str),

    // ── Groups ──────────────────────────────────────────────
    /// grammar.js: less_than = '<'
    LessThan(&'a str),
    /// grammar.js: greater_than = '>'
    GreaterThan(&'a str),
    /// grammar.js: left_double_quote = '\u201C'
    LeftDoubleQuote(&'a str),
    /// grammar.js: right_double_quote = '\u201D'
    RightDoubleQuote(&'a str),
    /// grammar.js: pho_begin_group = '‹' (U+2039)
    PhoGroupBegin(&'a str),
    /// grammar.js: pho_end_group = '›' (U+203A)
    PhoGroupEnd(&'a str),
    /// grammar.js: sin_begin_group = '〔' (U+3014)
    SinGroupBegin(&'a str),
    /// grammar.js: sin_end_group = '〕' (U+3015)
    SinGroupEnd(&'a str),

    // ── Misc structural ─────────────────────────────────────
    /// grammar.js: long_feature_begin = seq('&', '{l=', label)
    LongFeatureBegin(&'a str),
    /// grammar.js: long_feature_end = seq('&', '}l=', label)
    LongFeatureEnd(&'a str),
    /// grammar.js: nonvocal_begin = seq('&', '{n=', label)
    NonvocalBegin(&'a str),
    /// grammar.js: nonvocal_end = seq('&', '}n=', label)
    NonvocalEnd(&'a str),
    /// grammar.js: nonvocal_simple = seq('&', '{n=', label, '}')
    NonvocalSimple(&'a str),

    /// grammar.js: ampersand = '&'
    Ampersand(&'a str),
    /// grammar.js: left_bracket = '['
    LeftBracket(&'a str),
    /// grammar.js: right_bracket = ']'
    RightBracket(&'a str),
    /// grammar.js: media_url = token(/\u0015\d+_\d+-?\u0015/)
    /// Media bullet with tag-extracted timestamps.
    /// Pattern: `\u{0015}start_end-?\u{0015}`, tags mark start (t1..t2) and end (t3..t4).
    /// `raw_text` carries the full original slice including NAK delimiters,
    /// so no downstream reconstruction is needed.
    MediaBullet {
        raw_text: &'a str,
        start_time: &'a str,
        end_time: &'a str,
    },

    // ── CA elements (typed — one variant per CAElementType) ──
    CaBlockedSegments(&'a str), // ≠
    CaConstriction(&'a str),    // ∾
    CaHardening(&'a str),       // ⁑
    CaHurriedStart(&'a str),    // ⤇
    CaInhalation(&'a str),      // ∙
    CaLaughInWord(&'a str),     // Ἡ
    CaPitchDown(&'a str),       // ↓
    CaPitchReset(&'a str),      // ↻
    CaPitchUp(&'a str),         // ↑
    CaSuddenStop(&'a str),      // ⤆

    // ── CA delimiters (typed — one variant per CADelimiterType) ──
    CaUnsure(&'a str),            // ⁇
    CaPrecise(&'a str),           // §
    CaCreaky(&'a str),            // ⁎
    CaSofter(&'a str),            // °
    CaSegmentRepetition(&'a str), // ↫
    CaFaster(&'a str),            // ∆
    CaSlower(&'a str),            // ∇
    CaWhisper(&'a str),           // ∬
    CaSinging(&'a str),           // ∮
    CaLowPitch(&'a str),          // ▁
    CaHighPitch(&'a str),         // ▔
    CaLouder(&'a str),            // ◉
    CaSmileVoice(&'a str),        // ☺
    CaBreathyVoice(&'a str),      // ♋
    CaYawn(&'a str),              // Ϋ

    // ── %mor tier ───────────────────────────────────────────
    /// Rich MorWord: POS and lemma+features extracted by tags.
    /// Example: pos="verb", lemma_features="want-Fin-Ind-Pres"
    MorWord {
        pos: &'a str,
        lemma_features: &'a str,
    },
    /// Tilde in %mor: '~' (clitic separator between mor words)
    MorTilde(&'a str),

    // ── %gra tier ───────────────────────────────────────────
    /// Rich GraRelation: all 3 fields extracted by tags.
    /// Example: index="1", head="2", relation="SUBJ"
    GraRelation {
        index: &'a str,
        head: &'a str,
        relation: &'a str,
    },

    // ── %pho/%mod tier ────────────────────────────────────────
    /// PHO word: IPA phonological transcription segment.
    PhoWord(&'a str),
    /// Plus joining compound phonological words.
    PhoPlus(&'a str),

    // ── %sin tier ───────────────────────────────────────────
    /// SIN word: sign/gesture notation segment.
    SinWord(&'a str),

    // ── @Languages content ────────────────────────────────────
    /// Language code: /[a-z]{2,4}/ (e.g., "eng", "fra", "zho")
    LanguageCode(&'a str),

    // ── @Participants content ───────────────────────────────
    /// Participant word (speaker code, name, or role word)
    ParticipantWord(&'a str),

    // ── @Media content ──────────────────────────────────────
    /// Quoted media filename: "filename.mp4"
    MediaFilename(&'a str),
    /// Media word: unquoted filename, type (audio/video), or status
    MediaWord(&'a str),

    // ── Generic tier content ────────────────────────────────
    /// grammar.js: text_segment = /[^\u0015\r\n]+/
    TextSegment(&'a str),
    /// grammar.js: inline_pic = /\u0015%pic:"filename"\u0015/
    InlinePic(&'a str),

    // ── Errors (context-specific, one per condition) ───────
    //
    // Each error token tells the parser WHERE the error occurred.
    // The lexer always makes progress (consumes at least 1 char)
    // and stays in the same condition, so lexing never stops.
    /// Unrecognized character (global fallback — should rarely fire).
    ErrorUnrecognized(&'a str),
    /// Invalid line at top level (INITIAL condition).
    ErrorLine(&'a str),
    /// Junk after header name (expected :\t or newline).
    ErrorHeaderAfterName(&'a str),
    /// Invalid speaker code (SPEAKER condition).
    ErrorSpeaker(&'a str),
    /// Junk after tier label (TIER_AFTER_LABEL, expected :\t).
    ErrorTierAfterLabel(&'a str),
    /// Junk after speaker (TIER_SEP, expected :\t).
    ErrorTierSep(&'a str),
    /// Unclosed parenthesis in main content.
    ErrorUnclosedParen(&'a str),
    /// Unexpected char in main tier body (MAIN_CONTENT).
    ErrorInMainContent(&'a str),
    /// Unexpected char in %mor body (MOR_CONTENT).
    ErrorInMorContent(&'a str),
    /// Unexpected char in %gra body (GRA_CONTENT).
    ErrorInGraContent(&'a str),
    /// Unexpected char in %pho body (PHO_CONTENT).
    ErrorInPhoContent(&'a str),
    /// Unexpected char in %sin body (SIN_CONTENT).
    ErrorInSinContent(&'a str),
    /// Unexpected char in generic tier body (TIER_CONTENT).
    ErrorInTierContent(&'a str),
    /// Unexpected char in header value (HEADER_CONTENT).
    ErrorInHeaderContent(&'a str),
    /// Malformed @ID content (ID_CONTENT).
    ErrorInIdContent(&'a str),
    /// Malformed @Types content (TYPES_CONTENT).
    ErrorInTypesContent(&'a str),
    /// Unexpected char in @Languages content.
    ErrorInLanguagesContent(&'a str),
    /// Unexpected char in @Participants content.
    ErrorInParticipantsContent(&'a str),
    /// Unexpected char in @Media content.
    ErrorInMediaContent(&'a str),
}

impl<'a> Token<'a> {
    /// Check if this is an error token.
    pub fn is_err(&self) -> bool {
        matches!(
            self,
            Token::ErrorUnrecognized(_)
                | Token::ErrorLine(_)
                | Token::ErrorHeaderAfterName(_)
                | Token::ErrorSpeaker(_)
                | Token::ErrorTierAfterLabel(_)
                | Token::ErrorTierSep(_)
                | Token::ErrorUnclosedParen(_)
                | Token::ErrorInMainContent(_)
                | Token::ErrorInMorContent(_)
                | Token::ErrorInGraContent(_)
                | Token::ErrorInPhoContent(_)
                | Token::ErrorInSinContent(_)
                | Token::ErrorInTierContent(_)
                | Token::ErrorInHeaderContent(_)
                | Token::ErrorInIdContent(_)
                | Token::ErrorInTypesContent(_)
                | Token::ErrorInLanguagesContent(_)
                | Token::ErrorInParticipantsContent(_)
                | Token::ErrorInMediaContent(_)
        )
    }

    /// Human-readable context description for error tokens.
    pub fn error_context(&self) -> Option<&'static str> {
        match self {
            Token::ErrorUnrecognized(_) => Some("unrecognized character"),
            Token::ErrorLine(_) => Some("invalid line (expected @, *, or %)"),
            Token::ErrorHeaderAfterName(_) => {
                Some("expected colon+tab or newline after header name")
            }
            Token::ErrorSpeaker(_) => Some("invalid speaker code"),
            Token::ErrorTierAfterLabel(_) => Some("expected colon+tab after tier label"),
            Token::ErrorTierSep(_) => Some("expected colon+tab after speaker"),
            Token::ErrorUnclosedParen(_) => Some("unclosed parenthesis"),
            Token::ErrorInMainContent(_) => Some("unexpected character in main tier"),
            Token::ErrorInMorContent(_) => Some("unexpected character in %mor tier"),
            Token::ErrorInGraContent(_) => Some("unexpected character in %gra tier"),
            Token::ErrorInPhoContent(_) => Some("unexpected character in %pho tier"),
            Token::ErrorInSinContent(_) => Some("unexpected character in %sin tier"),
            Token::ErrorInTierContent(_) => Some("unexpected character in dependent tier"),
            Token::ErrorInHeaderContent(_) => Some("unexpected character in header"),
            Token::ErrorInIdContent(_) => {
                Some("malformed @ID content (expected 10 pipe-delimited fields)")
            }
            Token::ErrorInTypesContent(_) => {
                Some("malformed @Types content (expected 3 comma-separated fields)")
            }
            Token::ErrorInLanguagesContent(_) => Some("unexpected character in @Languages content"),
            Token::ErrorInParticipantsContent(_) => {
                Some("unexpected character in @Participants content")
            }
            Token::ErrorInMediaContent(_) => Some("unexpected character in @Media content"),
            _ => None,
        }
    }

    /// The text slice this token carries.
    pub fn text(&self) -> &'a str {
        match self {
            // Use a macro-like approach: every variant carries &str
            Token::BOM(s)
            | Token::Newline(s)
            | Token::Continuation(s)
            | Token::Whitespace(s)
            | Token::HeaderPrefix(s)
            | Token::HeaderSep(s)
            | Token::HeaderContent(s)
            | Token::HeaderUtf8(s)
            | Token::HeaderBegin(s)
            | Token::HeaderEnd(s)
            | Token::HeaderBlank(s)
            | Token::HeaderNewEpisode(s)
            | Token::Star(s)
            | Token::Speaker(s)
            | Token::TierPrefix(s)
            | Token::TierSep(s)
            | Token::Period(s)
            | Token::Question(s)
            | Token::Exclamation(s)
            | Token::TrailingOff(s)
            | Token::Interruption(s)
            | Token::SelfInterruption(s)
            | Token::InterruptedQuestion(s)
            | Token::BrokenQuestion(s)
            | Token::QuotedNewLine(s)
            | Token::QuotedPeriodSimple(s)
            | Token::SelfInterruptedQuestion(s)
            | Token::TrailingOffQuestion(s)
            | Token::BreakForCoding(s)
            | Token::CaNoBreak(s)
            | Token::CaTechnicalBreak(s)
            | Token::LinkerLazyOverlap(s)
            | Token::LinkerQuickUptake(s)
            | Token::LinkerQuickUptakeOverlap(s)
            | Token::LinkerQuotationFollows(s)
            | Token::LinkerSelfCompletion(s)
            | Token::CaNoBreakLinker(s)
            | Token::CaTechnicalBreakLinker(s)
            | Token::RetraceComplete(s)
            | Token::RetracePartial(s)
            | Token::RetraceMultiple(s)
            | Token::RetraceReformulation(s)
            | Token::ScopedStressing(s)
            | Token::ScopedContrastiveStressing(s)
            | Token::ScopedUncertain(s)
            | Token::ExcludeMarker(s)
            | Token::Freecode(s)
            | Token::CaContinuationMarker(s)
            | Token::LongFeatureBegin(s)
            | Token::LongFeatureEnd(s)
            | Token::NonvocalBegin(s)
            | Token::NonvocalEnd(s)
            | Token::NonvocalSimple(s)
            | Token::ErrorMarkerAnnotation(s)
            | Token::OverlapPrecedes(s)
            | Token::OverlapFollows(s)
            | Token::ExplanationAnnotation(s)
            | Token::ParaAnnotation(s)
            | Token::AltAnnotation(s)
            | Token::PercentAnnotation(s)
            | Token::Postcode(s)
            | Token::Langcode(s)
            | Token::Replacement(s)
            | Token::PauseLong(s)
            | Token::PauseMedium(s)
            | Token::PauseShort(s)
            | Token::PauseTimed(s)
            | Token::WordSegment(s)
            | Token::Shortening(s)
            | Token::Lengthening(s)
            | Token::StressPrimary(s)
            | Token::StressSecondary(s)
            | Token::OverlapTopBegin(s)
            | Token::OverlapTopEnd(s)
            | Token::OverlapBottomBegin(s)
            | Token::OverlapBottomEnd(s)
            | Token::SyllablePause(s)
            | Token::Tilde(s)
            | Token::CompoundMarker(s)
            | Token::UnderlineBegin(s)
            | Token::UnderlineEnd(s)
            | Token::PrefixFiller(s)
            | Token::PrefixNonword(s)
            | Token::PrefixFragment(s)
            | Token::Event(s)
            | Token::Zero(s)
            | Token::FormMarker(s)
            | Token::PosTag(s)
            | Token::Comma(s)
            | Token::Semicolon(s)
            | Token::Colon(s)
            | Token::TagMarker(s)
            | Token::VocativeMarker(s)
            | Token::UnmarkedEnding(s)
            | Token::UptakeSymbol(s)
            | Token::RisingToHigh(s)
            | Token::RisingToMid(s)
            | Token::LevelPitch(s)
            | Token::FallingToMid(s)
            | Token::FallingToLow(s)
            | Token::LessThan(s)
            | Token::GreaterThan(s)
            | Token::LeftDoubleQuote(s)
            | Token::RightDoubleQuote(s)
            | Token::PhoGroupBegin(s)
            | Token::PhoGroupEnd(s)
            | Token::SinGroupBegin(s)
            | Token::SinGroupEnd(s)
            | Token::Ampersand(s)
            | Token::LeftBracket(s)
            | Token::RightBracket(s)
            | Token::CaBlockedSegments(s)
            | Token::CaConstriction(s)
            | Token::CaHardening(s)
            | Token::CaHurriedStart(s)
            | Token::CaInhalation(s)
            | Token::CaLaughInWord(s)
            | Token::CaPitchDown(s)
            | Token::CaPitchReset(s)
            | Token::CaPitchUp(s)
            | Token::CaSuddenStop(s)
            | Token::CaUnsure(s)
            | Token::CaPrecise(s)
            | Token::CaCreaky(s)
            | Token::CaSofter(s)
            | Token::CaSegmentRepetition(s)
            | Token::CaFaster(s)
            | Token::CaSlower(s)
            | Token::CaWhisper(s)
            | Token::CaSinging(s)
            | Token::CaLowPitch(s)
            | Token::CaHighPitch(s)
            | Token::CaLouder(s)
            | Token::CaSmileVoice(s)
            | Token::CaBreathyVoice(s)
            | Token::CaYawn(s)
            | Token::MorTilde(s)
            | Token::PhoWord(s)
            | Token::PhoPlus(s)
            | Token::SinWord(s)
            | Token::LanguageCode(s)
            | Token::ParticipantWord(s)
            | Token::MediaFilename(s)
            | Token::MediaWord(s)
            | Token::TextSegment(s)
            | Token::InlinePic(s)
            | Token::ErrorUnrecognized(s)
            | Token::ErrorLine(s)
            | Token::ErrorHeaderAfterName(s)
            | Token::ErrorSpeaker(s)
            | Token::ErrorTierAfterLabel(s)
            | Token::ErrorTierSep(s)
            | Token::ErrorUnclosedParen(s)
            | Token::ErrorInMainContent(s)
            | Token::ErrorInMorContent(s)
            | Token::ErrorInGraContent(s)
            | Token::ErrorInPhoContent(s)
            | Token::ErrorInSinContent(s)
            | Token::ErrorInTierContent(s)
            | Token::ErrorInHeaderContent(s)
            | Token::ErrorInIdContent(s)
            | Token::ErrorInTypesContent(s)
            | Token::ErrorInLanguagesContent(s)
            | Token::ErrorInParticipantsContent(s)
            | Token::ErrorInMediaContent(s) => s,
            Token::Word { raw_text, .. } => raw_text,
            Token::WordLangSuffix(opt) => opt.unwrap_or("@s"),
            Token::OtherSpokenEvent { speaker, .. } => speaker,
            Token::MediaBullet { raw_text, .. } => raw_text,
            Token::HeaderBirthOf(s) | Token::HeaderBirthplaceOf(s) | Token::HeaderL1Of(s) => s,
            Token::MorWord { pos, .. } => pos,
            Token::GraRelation { index, .. } => index,
            Token::IdFields { language, .. } => language,
            Token::TypesFields { design, .. } => design,
        }
    }
}

/// Result of lexing a line: tokens + any errors found.
#[derive(Debug)]
pub struct LexResult<'a> {
    pub tokens: Vec<(Token<'a>, std::ops::Range<usize>)>,
}

impl<'a> LexResult<'a> {
    /// Returns all error tokens with their positions and context.
    pub fn errors(&self) -> Vec<LexError<'a>> {
        self.tokens
            .iter()
            .filter(|(t, _)| t.is_err())
            .map(|(t, span)| LexError {
                token: t.clone(),
                span: span.clone(),
                context: t.error_context().unwrap_or("unknown error"),
            })
            .collect()
    }

    /// True if the token stream contains no error tokens.
    pub fn is_clean(&self) -> bool {
        !self.tokens.iter().any(|(t, _)| t.is_err())
    }

    /// Human-readable error report.
    pub fn error_report(&self, source: &str) -> String {
        let errors = self.errors();
        if errors.is_empty() {
            return String::new();
        }
        let mut report = String::new();
        for e in &errors {
            let snippet = &source[e.span.clone()];
            let escaped = snippet.escape_debug().to_string();
            report.push_str(&format!(
                "  [{}-{}] {}: {:?} (text: \"{}\")\n",
                e.span.start, e.span.end, e.context, e.token, escaped
            ));
        }
        report
    }
}

/// A single lexer error with context.
#[derive(Debug, Clone)]
pub struct LexError<'a> {
    pub token: Token<'a>,
    pub span: std::ops::Range<usize>,
    pub context: &'static str,
}
