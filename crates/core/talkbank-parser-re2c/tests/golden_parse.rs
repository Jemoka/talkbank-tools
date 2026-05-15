//! Golden tests for main tier parsing.
//!
//! Test inputs from ~/talkbank/talkbank-tools/:
//! - resources/spec/constructs/main_tier/
//! - resources/corpus/reference/content/
//! - grammar/test/resources/corpus/main_tier/

use talkbank_parser_re2c::ast::*;
use talkbank_parser_re2c::parser;
use talkbank_parser_re2c::token::Token;

// ── Simple utterances ───────────────────────────────────────────

#[test]
fn simple_utterance() {
    let mt = parser::parse_main_tier("*CHI:\thello .\n").unwrap();
    assert!(matches!(mt.speaker, Token::Speaker("CHI")));
    assert_eq!(mt.tier_body.contents.len(), 1);
    assert!(matches!(mt.tier_body.terminator, Some(Token::Period(_))));
}

#[test]
fn multi_word() {
    let mt = parser::parse_main_tier("*CHI:\thello world .\n").unwrap();
    assert_eq!(mt.tier_body.contents.len(), 2);
}

// ── Terminators ─────────────────────────────────────────────────

#[test]
fn terminator_period() {
    let mt = parser::parse_main_tier("*CHI:\tI see a cat .\n").unwrap();
    assert!(matches!(mt.tier_body.terminator, Some(Token::Period(_))));
}

#[test]
fn terminator_question() {
    let mt = parser::parse_main_tier("*MOT:\twhere is the cat ?\n").unwrap();
    assert!(matches!(mt.tier_body.terminator, Some(Token::Question(_))));
}

#[test]
fn terminator_exclamation() {
    let mt = parser::parse_main_tier("*CHI:\twow !\n").unwrap();
    assert!(matches!(
        mt.tier_body.terminator,
        Some(Token::Exclamation(_))
    ));
}

#[test]
fn terminator_trailing_off() {
    let mt = parser::parse_main_tier("*CHI:\tI was going to the +...\n").unwrap();
    assert!(matches!(
        mt.tier_body.terminator,
        Some(Token::TrailingOff(_))
    ));
}

#[test]
fn terminator_interruption() {
    let mt = parser::parse_main_tier("*MOT:\twhat do they do with +/.\n").unwrap();
    assert!(matches!(
        mt.tier_body.terminator,
        Some(Token::Interruption(_))
    ));
}

#[test]
fn terminator_self_interruption() {
    let mt = parser::parse_main_tier("*MOT:\tand then she wouldn't +//.\n").unwrap();
    assert!(matches!(
        mt.tier_body.terminator,
        Some(Token::SelfInterruption(_))
    ));
}

#[test]
fn terminator_break_for_coding() {
    let mt = parser::parse_main_tier("*EXP:\tfoo bar +.\n").unwrap();
    assert!(matches!(
        mt.tier_body.terminator,
        Some(Token::BreakForCoding(_))
    ));
}

#[test]
fn terminator_interrupted_question() {
    let mt = parser::parse_main_tier("*MOT:\twhat would they have in a +/?\n").unwrap();
    assert!(matches!(
        mt.tier_body.terminator,
        Some(Token::InterruptedQuestion(_))
    ));
}

#[test]
fn terminator_self_interrupted_question() {
    let mt = parser::parse_main_tier("*MOT:\twhat were you going to say about +//?\n").unwrap();
    assert!(matches!(
        mt.tier_body.terminator,
        Some(Token::SelfInterruptedQuestion(_))
    ));
}

#[test]
fn terminator_trailing_off_question() {
    let mt = parser::parse_main_tier("*MOT:\tanything to belong to the club or +..?\n").unwrap();
    assert!(matches!(
        mt.tier_body.terminator,
        Some(Token::TrailingOffQuestion(_))
    ));
}

#[test]
fn terminator_broken_question() {
    let mt = parser::parse_main_tier("*MOT:\tbut what if they +!?\n").unwrap();
    assert!(matches!(
        mt.tier_body.terminator,
        Some(Token::BrokenQuestion(_))
    ));
}

#[test]
fn terminator_quoted_new_line() {
    let mt = parser::parse_main_tier("*CHI:\tthe bear said +\"/.\n").unwrap();
    assert!(matches!(
        mt.tier_body.terminator,
        Some(Token::QuotedNewLine(_))
    ));
}

#[test]
fn terminator_quoted_period_simple() {
    let mt = parser::parse_main_tier("*CHI:\tthe bear said +\".\n").unwrap();
    assert!(matches!(
        mt.tier_body.terminator,
        Some(Token::QuotedPeriodSimple(_))
    ));
}

#[test]
fn terminator_optional() {
    // Terminator is optional per grammar.js
    let mt = parser::parse_main_tier("*SPK:\trising to high \u{21D7}\n").unwrap();
    assert!(!mt.tier_body.contents.is_empty());
}

// ── Word features ───────────────────────────────────────────────

#[test]
fn compound_word() {
    let mt = parser::parse_main_tier("*CHI:\tI want ice+cream .\n").unwrap();
    let words: Vec<_> = mt
        .tier_body
        .contents
        .iter()
        .filter_map(|c| match c {
            ContentItem::Word(w) => Some(w),
            _ => None,
        })
        .collect();
    assert_eq!(words.len(), 3);
    assert!(
        words[2]
            .body
            .iter()
            .any(|b| matches!(b, WordBodyItem::CompoundMarker))
    );
}

#[test]
fn word_with_form_marker() {
    let mt = parser::parse_main_tier("*MOT:\tmama@f is here .\n").unwrap();
    let words: Vec<_> = mt
        .tier_body
        .contents
        .iter()
        .filter_map(|c| match c {
            ContentItem::Word(w) => Some(w),
            _ => None,
        })
        .collect();
    assert!(
        words[0].form_marker.is_some(),
        "expected form marker on first word"
    );
}

#[test]
fn word_with_lengthening() {
    let mt = parser::parse_main_tier("*CHI:\tno:: .\n").unwrap();
    let words: Vec<_> = mt
        .tier_body
        .contents
        .iter()
        .filter_map(|c| match c {
            ContentItem::Word(w) => Some(w),
            _ => None,
        })
        .collect();
    assert!(
        words[0]
            .body
            .iter()
            .any(|b| matches!(b, WordBodyItem::Lengthening(2)))
    );
}

#[test]
fn filler_word() {
    let mt = parser::parse_main_tier("*CHI:\t&-um I want cookies .\n").unwrap();
    let words: Vec<_> = mt
        .tier_body
        .contents
        .iter()
        .filter_map(|c| match c {
            ContentItem::Word(w) => Some(w),
            _ => None,
        })
        .collect();
    assert!(
        matches!(words[0].category, Some(WordCategory::Filler)),
        "expected filler category on first word"
    );
}

#[test]
fn event_happening() {
    let mt = parser::parse_main_tier("*CHI:\t&=laughs .\n").unwrap();
    let has_event = mt
        .tier_body
        .contents
        .iter()
        .any(|c| matches!(c, ContentItem::Event(_)));
    assert!(has_event, "got {:?}", mt.tier_body.contents);
}

// ── Annotations ─────────────────────────────────────────────────

#[test]
fn retracing_annotation() {
    let mt = parser::parse_main_tier("*CHI:\tthe the [/] dog .\n").unwrap();
    let has_retrace = mt
        .tier_body
        .contents
        .iter()
        .any(|c| matches!(c, ContentItem::Retrace(_)));
    assert!(has_retrace, "got {:?}", mt.tier_body.contents);
}

#[test]
fn explanation_annotation() {
    let mt = parser::parse_main_tier("*CHI:\tit [= the cookie] .\n").unwrap();
    let has = mt.tier_body.contents.iter().any(|c| match c {
        ContentItem::Word(w) => w
            .annotations
            .iter()
            .any(|a| matches!(a, ParsedAnnotation::Explanation(_))),
        _ => false,
    });
    assert!(has, "got {:?}", mt.tier_body.contents);
}

#[test]
fn pause_in_utterance() {
    let mt = parser::parse_main_tier("*CHI:\tI (.) want cookies .\n").unwrap();
    let has_pause = mt
        .tier_body
        .contents
        .iter()
        .any(|c| matches!(c, ContentItem::Pause(_)));
    assert!(has_pause, "got {:?}", mt.tier_body.contents);
}

// ── Linkers ─────────────────────────────────────────────────────

#[test]
fn linker_quick_uptake() {
    let mt = parser::parse_main_tier("*MOT:\t+^ hi there .\n").unwrap();
    assert_eq!(mt.tier_body.linkers.len(), 1);
}

// ── Postcodes ───────────────────────────────────────────────────

#[test]
fn postcodes_after_terminator() {
    let mt = parser::parse_main_tier("*CHI:\thello . [+ bch] [+ foo]\n").unwrap();
    assert!(matches!(mt.tier_body.terminator, Some(Token::Period(_))));
    assert_eq!(
        mt.tier_body.postcodes.len(),
        2,
        "got {:?}",
        mt.tier_body.postcodes
    );
}

// ── Groups ──────────────────────────────────────────────────────

#[test]
fn angle_bracket_group() {
    // <I want> [//] is a group retrace (complete), not a plain group
    let mt = parser::parse_main_tier("*CHI:\t<I want> [//] I need cookies .\n").unwrap();
    let has_retrace = mt
        .tier_body
        .contents
        .iter()
        .any(|c| matches!(c, ContentItem::Retrace(r) if r.is_group));
    assert!(has_retrace, "got {:?}", mt.tier_body.contents);
}

// ── Corpus smoke test ───────────────────────────────────────────

#[test]
#[ignore] // Slow: parses every *-line from all corpus dirs individually. Run with --ignored.
fn reference_corpus_main_tiers() {
    let base =
        std::path::Path::new(&std::env::var("HOME").unwrap_or_else(|_| String::from("/tmp")))
            .join("talkbank/talkbank-tools/resources/corpus/reference");

    if !base.exists() {
        return;
    }

    let mut total = 0;
    let mut parsed = 0;
    for dir in ["core", "content", "annotation", "tiers", "ca", "languages"] {
        let dir_path = base.join(dir);
        if !dir_path.exists() {
            continue;
        }
        for entry in std::fs::read_dir(&dir_path).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|e| e == "cha") {
                let content = std::fs::read_to_string(&path).unwrap();
                for line in content.lines() {
                    if line.starts_with('*') {
                        total += 1;
                        let input = format!("{line}\n");
                        if parser::parse_main_tier(&input).is_some() {
                            parsed += 1;
                        }
                    }
                }
            }
        }
    }
    eprintln!("Parsed {parsed}/{total} main tier lines");
    assert!(parsed as f64 / total as f64 > 0.9, "{parsed}/{total}");
}
