//! Integration tests for the CHAT-file sanitizer.
//!
//! All fixtures here are synthetic — built from scratch as string
//! literals so the test suite never depends on real contributor
//! transcripts. Every test asserts a specific structural-preservation
//! or content-redaction property; together they cover the v1 leak
//! surface documented in `book/src/user-guide/sanitize.md`.

use talkbank_model::WriteChat;
use talkbank_parser::TreeSitterParser;
use talkbank_transform::redact::{SanitizationPolicy, sanitize};

/// Parses `cha` and runs the strict sanitizer; returns the serialized output.
fn sanitize_to_string(cha: &str) -> String {
    let parser = TreeSitterParser::new().expect("parser construction");
    let parsed = parser.parse_chat_file(cha).expect("parse fixture");
    let policy = SanitizationPolicy::strict();
    let sanitized = sanitize(parsed, &policy).expect("sanitize");
    sanitized.to_chat_string()
}

/// Parses `cha` and re-serializes it through the model without sanitizing —
/// gives the round-trip baseline for byte comparisons.
fn roundtrip_only(cha: &str) -> String {
    let parser = TreeSitterParser::new().expect("parser construction");
    let parsed = parser.parse_chat_file(cha).expect("parse fixture");
    let mut out = String::new();
    parsed.write_chat(&mut out).expect("write_chat");
    out
}

const BULLET: char = '\u{0015}';

/// Wraps a single-participant PAR-only test body in the standard CHAT
/// header skeleton. `body` is the text between `@ID` and `@End` —
/// callers supply just the part their test cares about.
fn solo_par(body: &str) -> String {
    format!(
        "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tPAR Adult\n\
@ID:\teng|test|PAR|||||Adult|||\n\
{body}\
@End\n"
    )
}

/// Fixture: 4 utterances with distinct `•start_end•` patterns.
fn fixture_four_utterances_with_bullets() -> String {
    format!(
        "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tPAR Adult, INV Investigator\n\
@ID:\teng|test|PAR|||||Adult|||\n\
@ID:\teng|test|INV|||||Investigator|||\n\
*PAR:\tthe cat sat . {BULLET}1000_2000{BULLET}\n\
*INV:\tand then ? {BULLET}2100_2900{BULLET}\n\
*PAR:\tit ran away . {BULLET}3000_4500{BULLET}\n\
*INV:\twhy ? {BULLET}4600_5000{BULLET}\n\
@End\n",
        BULLET = BULLET,
    )
}

#[test]
fn t01_bullets_byte_exact() {
    let cha = fixture_four_utterances_with_bullets();
    let out = sanitize_to_string(&cha);
    for ms in ["1000_2000", "2100_2900", "3000_4500", "4600_5000"] {
        let needle = format!("{BULLET}{ms}{BULLET}");
        assert!(
            out.contains(&needle),
            "bullet pattern {needle:?} not preserved byte-exactly in:\n{out}"
        );
    }
}

#[test]
fn t02_wor_per_word_offsets_byte_exact() {
    // %wor uses bare `word START_END` triples on the dependent tier.
    // The main-tier bullet still uses the `\u{0015}` delimiter, but %wor
    // word-level offsets do not.
    let cha = solo_par(&format!(
        "*PAR:\tthe cat sat . {BULLET}1000_2000{BULLET}\n\
%wor:\tthe 1000_1100 cat 1100_1500 sat 1500_2000 .\n"
    ));
    let out = sanitize_to_string(&cha);
    for offset in ["1000_1100", "1100_1500", "1500_2000"] {
        assert!(
            out.contains(offset),
            "%wor offset {offset:?} not preserved byte-exactly in:\n{out}"
        );
    }
}

#[test]
fn t03_structural_counts_preserved() {
    let cha = fixture_four_utterances_with_bullets();
    let out = sanitize_to_string(&cha);

    let count_lines =
        |s: &str, prefix: &str| -> usize { s.lines().filter(|l| l.starts_with(prefix)).count() };
    assert_eq!(count_lines(&out, "*PAR:"), 2, "main *PAR utterance count");
    assert_eq!(count_lines(&out, "*INV:"), 2, "main *INV utterance count");
}

#[test]
fn t04_speaker_codes_preserved() {
    let cha = fixture_four_utterances_with_bullets();
    let out = sanitize_to_string(&cha);
    assert!(out.contains("*PAR:"), "*PAR speaker code missing");
    assert!(out.contains("*INV:"), "*INV speaker code missing");
    assert!(
        !out.contains("*MAR:") && !out.contains("*JOH:"),
        "no fabricated speaker codes"
    );
}

#[test]
fn t05_word_content_replaced_with_placeholders() {
    let cha = solo_par(&format!(
        "*PAR:\tthe cat sat on the mat . {BULLET}1000_2000{BULLET}\n"
    ));
    let out = sanitize_to_string(&cha);
    for word in ["the ", "cat ", "sat ", "on ", "mat "] {
        assert!(
            !out.contains(&format!("\t{word}")) && !out.contains(&format!(" {word}")),
            "source word {word:?} leaked into output:\n{out}"
        );
    }
    assert!(out.contains("w1"), "expected w1 placeholder, got:\n{out}");
    assert!(out.contains("w6"), "expected w6 placeholder, got:\n{out}");
}

#[test]
fn t06_compound_and_clitic_markers_preserved() {
    let cha = solo_par(&format!(
        "*PAR:\tice+cream and dog~s . {BULLET}1000_2000{BULLET}\n"
    ));
    let out = sanitize_to_string(&cha);
    assert!(
        out.contains('+'),
        "compound marker '+' not preserved:\n{out}"
    );
    assert!(
        out.contains('~'),
        "clitic boundary '~' not preserved:\n{out}"
    );
    assert!(
        !out.contains("ice") && !out.contains("cream") && !out.contains("dog"),
        "lexical content leaked:\n{out}"
    );
}

#[test]
fn t07_id_custom_field_anonymized() {
    // t07 has its own skeleton because it puts a name into the @ID
    // custom_field slot — that's the whole point of this test.
    let cha = format!(
        "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tPAR Adult\n\
@ID:\teng|test|PAR|||||Adult||Jane Smith|\n\
*PAR:\thello . {BULLET}1000_2000{BULLET}\n\
@End\n",
        BULLET = BULLET,
    );
    let out = sanitize_to_string(&cha);
    assert!(
        !out.contains("Jane Smith") && !out.contains("Jane") && !out.contains("Smith"),
        "@ID custom_field name leaked:\n{out}"
    );
}

#[test]
fn t08_participants_name_anonymized() {
    // Names land in @Participants' name field. @ID role slot is reserved for
    // closed-set roles (Adult, Target_Child, Mother, Investigator, etc.) and
    // is intentionally preserved.
    let cha = format!(
        "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tPAR Mary_Jones Adult, INV John_Doe Investigator\n\
@ID:\teng|test|PAR|||||Adult|||\n\
@ID:\teng|test|INV|||||Investigator|||\n\
*PAR:\thello . {BULLET}1000_2000{BULLET}\n\
@End\n",
        BULLET = BULLET,
    );
    let out = sanitize_to_string(&cha);
    assert!(
        !out.contains("Mary")
            && !out.contains("Jones")
            && !out.contains("John")
            && !out.contains("Doe"),
        "@Participants name leaked:\n{out}"
    );
}

#[test]
fn t09_mor_lemma_replaced_pos_preserved() {
    let cha = solo_par(&format!(
        "*PAR:\tthe cat sat . {BULLET}1000_2000{BULLET}\n\
%mor:\tdet|the n|cat v|sit-Past .\n"
    ));
    let out = sanitize_to_string(&cha);
    assert!(
        !out.contains("|the") && !out.contains("|cat") && !out.contains("|sit"),
        "%mor lemma leaked:\n{out}"
    );
    assert!(out.contains("det|"), "POS det| missing:\n{out}");
    assert!(out.contains("n|"), "POS n| missing:\n{out}");
    assert!(out.contains("v|"), "POS v| missing:\n{out}");
    assert!(out.contains("-Past"), "feature -Past missing:\n{out}");
}

#[test]
fn t10_phonological_tier_dropped() {
    let cha = solo_par(&format!(
        "*PAR:\thello . {BULLET}1000_2000{BULLET}\n\
%pho:\thəloʊ .\n"
    ));
    let out = sanitize_to_string(&cha);
    assert!(
        !out.contains("%pho"),
        "%pho tier should be dropped from output:\n{out}"
    );
    assert!(
        !out.contains("həloʊ"),
        "phonological content should not appear:\n{out}"
    );
}

#[test]
fn t11_freetext_dependent_tier_redacted() {
    let cha = solo_par(&format!(
        "*PAR:\thello . {BULLET}1000_2000{BULLET}\n\
%com:\tchild was tired and stopped responding\n"
    ));
    let out = sanitize_to_string(&cha);
    assert!(
        !out.contains("tired") && !out.contains("stopped") && !out.contains("responding"),
        "%com free-text leaked:\n{out}"
    );
    assert!(
        out.contains("[redacted]"),
        "expected '[redacted]' marker in %com:\n{out}"
    );
}

#[test]
fn t12_untranscribed_words_preserved() {
    let cha = solo_par(&format!("*PAR:\txxx and yyy . {BULLET}1000_2000{BULLET}\n"));
    let out = sanitize_to_string(&cha);
    assert!(
        out.contains("xxx"),
        "untranscribed marker xxx must be preserved:\n{out}"
    );
    assert!(
        out.contains("yyy"),
        "untranscribed marker yyy must be preserved:\n{out}"
    );
}

#[test]
fn t13_sanitized_output_parses_back() {
    let cha = fixture_four_utterances_with_bullets();
    let out = sanitize_to_string(&cha);
    let parser = TreeSitterParser::new().expect("parser construction");
    parser
        .parse_chat_file(&out)
        .expect("sanitized output should re-parse cleanly");
}

#[test]
fn t14_idempotent_under_repeat() {
    let cha = fixture_four_utterances_with_bullets();
    let once = sanitize_to_string(&cha);
    let twice = sanitize_to_string(&once);
    assert_eq!(
        once, twice,
        "sanitize must be idempotent (sanitize(sanitize(x)) == sanitize(x))"
    );
}

#[test]
fn t15_deterministic_across_runs() {
    let cha = fixture_four_utterances_with_bullets();
    let a = sanitize_to_string(&cha);
    let b = sanitize_to_string(&cha);
    assert_eq!(
        a, b,
        "two runs of sanitize on the same input must be byte-identical"
    );
}

#[test]
fn t16_event_and_freecode_redacted() {
    let cha = solo_par(&format!(
        "*PAR:\thello &=imitates:Mary [^ aside about Jane] . {BULLET}1000_2000{BULLET}\n"
    ));
    let out = sanitize_to_string(&cha);
    assert!(
        !out.contains("Mary")
            && !out.contains("Jane")
            && !out.contains("imitates")
            && !out.contains("aside"),
        "Event/Freecode free text leaked:\n{out}"
    );
}

#[test]
fn t17_replaced_word_both_sides_sanitized() {
    // ReplacedWord carries the actually-spoken word AND the intended
    // replacement(s) — both contain lexical content and both must be
    // redacted. Parser shape: `actual [: replacement]`.
    let cha = solo_par(&format!(
        "*PAR:\the goed [: went] home . {BULLET}1000_2000{BULLET}\n"
    ));
    let out = sanitize_to_string(&cha);
    assert!(
        !out.contains("goed") && !out.contains("went"),
        "ReplacedWord lexical content leaked:\n{out}"
    );
    assert!(
        !out.contains("home") && !out.contains(" he "),
        "surrounding words leaked:\n{out}"
    );
}

/// Sanity test on the round-trip helper itself — parser must round-trip
/// our fixture's bullet patterns without sanitization, so other test
/// failures aren't parsing problems disguised as sanitization bugs.
#[test]
fn t00_fixture_roundtrips_without_sanitize() {
    let cha = fixture_four_utterances_with_bullets();
    let out = roundtrip_only(&cha);
    for ms in ["1000_2000", "2100_2900", "3000_4500", "4600_5000"] {
        let needle = format!("{BULLET}{ms}{BULLET}");
        assert!(
            out.contains(&needle),
            "round-trip without sanitize lost bullet {needle:?}:\n{out}"
        );
    }
}
