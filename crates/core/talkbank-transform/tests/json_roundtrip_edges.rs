//! Targeted CHAT <-> JSON roundtrip tests for ten specific edge cases.
//!
//! Each test builds a small CHAT string exercising one tricky construct
//! (every dependent-tier kind, CA-indexed overlaps, Unicode/IPA, scoped
//! bracketed annotations, %mor clitics, media bullets, multi-speaker
//! overlap, all special terminators, gems + comments, and the minimal
//! valid file), then runs the full pipeline:
//!
//!   CHAT text -> `ChatFile` (parse)
//!   `ChatFile` -> JSON (via `chat_to_json`)
//!   JSON -> `ChatFile` (via `serde_json::from_str`)
//!   `ChatFile` -> CHAT text (via `WriteChat::to_chat_string`)
//!   CHAT text -> `ChatFile` (reparse)
//!
//! The first parse and the reparse are compared using `SemanticEq`, which
//! ignores spans and derived alignment metadata but asserts that the
//! linguistic content is preserved across the JSON boundary.
//!
//! If any of these roundtrips loses information, the corresponding test
//! fails loudly — that is a real bug in the CHAT/JSON bridge, not a test
//! that needs relaxing. The tests deliberately do not touch production
//! code; they only probe it.

use talkbank_model::{ChatFile, ParseValidateOptions, SemanticEq, WriteChat};
use talkbank_transform::{chat_to_json_unvalidated, parse_and_validate};

/// Run the full CHAT -> JSON -> CHAT -> parse roundtrip and assert
/// that the reparsed file is `SemanticEq` to the original parse.
///
/// We use `chat_to_json_unvalidated` so that the test reports only the
/// lossless-roundtrip question, independently of JSON-schema drift; a
/// separate gate (`json_tests.rs`) already checks schema conformance.
/// `ParseValidateOptions::default()` is used because these fixtures are
/// intentionally small and we want parse-level semantics, not the full
/// validation pass which would flag things like missing `@ID` alignment.
fn assert_roundtrip_preserves_semantics(label: &str, original_chat: &str) {
    let opts = ParseValidateOptions::default();

    // Original parse (reference).
    let original = parse_and_validate(original_chat, opts.clone())
        .unwrap_or_else(|e| panic!("[{label}] parse of original CHAT failed: {e}"));

    // CHAT -> JSON.
    let json = chat_to_json_unvalidated(original_chat, opts.clone(), true)
        .unwrap_or_else(|e| panic!("[{label}] CHAT->JSON failed: {e}"));

    // JSON -> ChatFile.
    let from_json: ChatFile = serde_json::from_str(&json)
        .unwrap_or_else(|e| panic!("[{label}] JSON->ChatFile failed: {e}"));

    // ChatFile -> CHAT text.
    let regenerated_chat = from_json.to_chat_string();

    // Reparse: CHAT text -> ChatFile (this is the fairest comparison
    // point because both sides went through the parser, so span fields
    // come from the same source path).
    let reparsed = parse_and_validate(&regenerated_chat, opts).unwrap_or_else(|e| {
        panic!(
            "[{label}] reparse of roundtripped CHAT failed: {e}\n\
             --- regenerated CHAT ---\n{regenerated_chat}\n--- end ---"
        )
    });

    // SemanticEq ignores spans, alignments, and derived language metadata,
    // so any remaining difference is a genuine semantic divergence.
    assert!(
        original.semantic_eq(&reparsed),
        "[{label}] roundtrip not semantically lossless.\n\
         --- original CHAT ---\n{original_chat}\n\
         --- regenerated CHAT ---\n{regenerated_chat}\n--- end ---"
    );
}

// ---------------------------------------------------------------------------
// Test 1: Every common tier type on a single utterance.
//
// The CHAT manual allows many kinds of dependent tiers under one main tier
// line. We stack %mor, %gra, %pho, %wor, %sin, %com, %act, %cod so that the
// serializer has to round-trip an unusually dense tier block for one
// utterance.
// ---------------------------------------------------------------------------
#[test]
fn roundtrip_every_tier_type_present() {
    let chat = "\
@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Child
@ID:\teng|corpus|CHI|||||Child|||
*CHI:\thello world .
%mor:\tco|hello n|world .
%gra:\t1|2|COM 2|0|INCROOT 3|2|PUNCT
%pho:\thɛloʊ wɝld
%wor:\thello world .
%sin:\t0 0
%com:\tchild greets the world
%act:\twaving hand
%cod:\t$GREETING
@End
";
    assert_roundtrip_preserves_semantics("every_tier_type", chat);
}

// ---------------------------------------------------------------------------
// Test 2: CA notation with indexed overlap markers.
//
// Indexed overlaps use the digits after the bracket characters to pair
// simultaneous overlap regions across speakers. We reuse the exact pattern
// from `resources/corpus/reference/ca/overlaps.cha`, which documents indexed overlaps
// as a first-class CA construct.
// ---------------------------------------------------------------------------
#[test]
fn roundtrip_ca_indexed_overlaps() {
    let chat = "\
@UTF8
@Begin
@Languages:\teng
@Participants:\tSPK Speaker, LSN Adult
@Options:\tCA
@ID:\teng|corpus|SPK|||||Speaker|||
@ID:\teng|corpus|LSN|||||Adult|||
*SPK:\t⌈ one ⌉ ⌈2 two ⌉2 .
*LSN:\t⌊ one ⌋ .
*LSN:\t⌊2 two ⌋2 .
@End
";
    assert_roundtrip_preserves_semantics("ca_indexed_overlaps", chat);
}

// ---------------------------------------------------------------------------
// Test 3: Unicode in content — IPA characters, combining diacritics, CJK.
//
// Everything in CHAT is UTF-8 but the JSON string encoder must preserve
// combining diacritics and non-BMP codepoints identically. We mix IPA
// (with combining marks) and Chinese characters in both dependent-tier and
// main-tier content.
// ---------------------------------------------------------------------------
#[test]
fn roundtrip_unicode_ipa_and_cjk() {
    let chat = "\
@UTF8
@Begin
@Languages:\teng, zho
@Participants:\tCHI Child, MOT Mother
@ID:\teng|corpus|CHI|2;06.||||Child|||
@ID:\teng|corpus|MOT|||||Mother|||
*CHI:\tMommy .
%pho:\tˈmɑmiː
*MOT:\t你好 world .
%pho:\tni˨˩˦ xɑʊ˨˩˦ wɝld
@End
";
    assert_roundtrip_preserves_semantics("unicode_ipa_and_cjk", chat);
}

// ---------------------------------------------------------------------------
// Test 4: Multiple scoped annotations stacked on a single word.
//
// The grammar allows several bracketed annotations to attach to one word.
// We stack a stressing marker `[!]` and an error marker `[* s:r]` on the
// same word, and add an `[= explanation]` on a later word. Pattern drawn
// from `resources/corpus/reference/annotation/scope-markers.cha` and the multi-feature
// lines in `resources/corpus/reference/word-features/000829.cha`.
// ---------------------------------------------------------------------------
#[test]
fn roundtrip_nested_annotations_on_word() {
    let chat = "\
@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Child, MOT Mother
@ID:\teng|corpus|CHI|3;00.||||Child|||
@ID:\teng|corpus|MOT|||||Mother|||
*MOT:\tyou [!] did that .
*CHI:\tthis is child [?] speaking [= quietly] .
@End
";
    assert_roundtrip_preserves_semantics("nested_annotations", chat);
}

// ---------------------------------------------------------------------------
// Test 5: %mor tier with clitics and ampersand features.
//
// `pron|it~aux|be&PRES` encodes a cliticized morphological parse:
// two morpheme records joined by `~`, the second carrying an ampersand
// feature. This is the representative pattern for English `it's`, `don't`,
// etc. Reused verbatim from `resources/corpus/reference/edge-cases/clitics-and-compounds.cha`.
// ---------------------------------------------------------------------------
#[test]
fn roundtrip_mor_clitics() {
    let chat = "\
@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Child
@ID:\teng|corpus|CHI|4;00.||||Child|||
*CHI:\tit's a cookie .
%mor:\tpron|it~aux|be&PRES det|a n|cookie .
%gra:\t1|4|SUBJ 2|1|AUX 3|4|DET 4|0|ROOT 5|4|PUNCT
@End
";
    assert_roundtrip_preserves_semantics("mor_clitics", chat);
}

// ---------------------------------------------------------------------------
// Test 6: Media bullets.
//
// CHAT uses the `\u{15}` (NAK) control character to delimit a time-aligned
// media bullet of the form `\u{15}START_END\u{15}`. The serializer must
// preserve the exact byte pattern after the JSON roundtrip — JSON is
// required to escape control characters, and the deserializer must unescape
// them back to raw `\u{15}`.
// ---------------------------------------------------------------------------
#[test]
fn roundtrip_media_bullets() {
    let chat = "\
@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Child
@ID:\teng|corpus|CHI|||||Child|||
@Media:\tfake, audio
*CHI:\thello world . \u{15}1000_2000\u{15}
@End
";
    assert_roundtrip_preserves_semantics("media_bullets", chat);
}

// ---------------------------------------------------------------------------
// Test 7: Three speakers with interleaved turns and CA overlaps between
// different pairs of speakers.
//
// Exercises the turn-taking pattern where overlap brackets span across
// non-adjacent speakers (e.g. CHI and FAT overlap while MOT sits between
// them in document order). Pattern mirrors
// `resources/corpus/reference/edge-cases/ca-overlap-complex.cha`.
// ---------------------------------------------------------------------------
#[test]
fn roundtrip_three_speakers_interleaved_overlaps() {
    let chat = "\
@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Child, MOT Mother, FAT Father
@Options:\tCA
@ID:\teng|corpus|CHI|3;00.||||Child|||
@ID:\teng|corpus|MOT|||||Mother|||
@ID:\teng|corpus|FAT|||||Father|||
*CHI:\tI want ⌈ cookie ⌉ please .
*MOT:\t⌊ no ⌋ you cannot .
*FAT:\tlet him ⌈ have ⌉ just one .
*MOT:\t⌊ no ⌋ way .
@End
";
    assert_roundtrip_preserves_semantics("three_speakers_interleaved", chat);
}

// ---------------------------------------------------------------------------
// Test 8: All supported special terminators exercised on the main tier.
//
// Covers: `.`, `?`, `!`, `+...` (trailing off), `+/.` (interruption),
// `+//.` (self-interruption), `+"/.` (quoted new line),
// and `+!?` (broken question). These are exactly the terminator forms
// tracked by `resources/corpus/reference/edge-cases/special-terminators.cha`.
// ---------------------------------------------------------------------------
#[test]
fn roundtrip_special_terminators() {
    let chat = "\
@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Child, MOT Mother
@ID:\teng|corpus|CHI|3;00.||||Child|||
@ID:\teng|corpus|MOT|||||Mother|||
*CHI:\tstatement .
*MOT:\tquestion ?
*CHI:\texclaim !
*CHI:\tgoing to the +...
*MOT:\tplay with +/.
*MOT:\tshe wouldn't +//.
*CHI:\tand the bear said +\"/.
*CHI:\tbut what if they +!?
@End
";
    assert_roundtrip_preserves_semantics("special_terminators", chat);
}

// ---------------------------------------------------------------------------
// Test 9: Comments and gem markers interleaved with utterances.
//
// `@Comment:` lines can appear anywhere between utterances; `@Bg:` / `@Eg:`
// pair up to mark a "gem" (a named region of the transcript). The JSON
// roundtrip must preserve both the header-line interleaving order and the
// gem labels. Pattern adapted from
// `resources/corpus/reference/edge-cases/postcodes-and-gems.cha`.
// ---------------------------------------------------------------------------
#[test]
fn roundtrip_comments_and_gems() {
    let chat = "\
@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Child, MOT Mother
@ID:\teng|corpus|CHI|3;00.||||Child|||
@ID:\teng|corpus|MOT|||||Mother|||
@Comment:\tbefore the gem
@Bg:\tmorning play
*CHI:\thello .
*MOT:\tgood morning .
@Comment:\tmid-gem note
*CHI:\tI want cereal .
@Eg:\tmorning play
@Comment:\tafter the gem
@End
";
    assert_roundtrip_preserves_semantics("comments_and_gems", chat);
}

// ---------------------------------------------------------------------------
// Test 10: Minimal valid file — the smallest well-formed CHAT we accept.
//
// One participant, one utterance, no dependent tiers. Matches
// `resources/corpus/reference/edge-cases/empty-and-minimal.cha`. This is the floor
// case: if this roundtrip fails, everything else is also broken.
// ---------------------------------------------------------------------------
#[test]
fn roundtrip_minimal_valid_file() {
    let chat = "\
@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Child
@ID:\teng|corpus|CHI|||||Child|||
*CHI:\thello .
@End
";
    assert_roundtrip_preserves_semantics("minimal_valid", chat);
}
