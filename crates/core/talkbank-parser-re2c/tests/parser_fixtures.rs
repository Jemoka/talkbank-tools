//! Parser tests using extracted corpus fixtures.
//!
//! Each test loads real CHAT lines from tests/fixtures/,
//! verifies they lex cleanly, then parses them and checks
//! the resulting AST structure.

mod fixture_utils;

use fixture_utils::{load_and_verify_lex, load_fixture};
use insta::assert_yaml_snapshot;
use talkbank_model::{Separator, UtteranceContent};
use talkbank_parser_re2c::ast::*;
use talkbank_parser_re2c::convert::main_tier_to_model;
use talkbank_parser_re2c::parser;
use talkbank_parser_re2c::token::Token;

// ═══════════════════════════════════════════════════════════════
// Main tier parsing from fixtures
// ═══════════════════════════════════════════════════════════════

#[test]
fn parse_main_tier_fixtures() {
    let entries = load_and_verify_lex("main_tier");
    if entries.is_empty() {
        return;
    }

    let mut parsed = 0;
    let mut failed = 0;
    for entry in &entries {
        let input = if entry.ends_with('\n') {
            entry.clone()
        } else {
            format!("{entry}\n")
        };
        match parser::parse_main_tier(&input) {
            Some(mt) => {
                parsed += 1;
                // Basic structural checks
                assert!(
                    !matches!(mt.speaker, Token::ErrorSpeaker(_)),
                    "error speaker in: {}",
                    entry.chars().take(60).collect::<String>()
                );
            }
            None => {
                failed += 1;
                if failed <= 3 {
                    eprintln!(
                        "  PARSE FAILED: {}",
                        entry.chars().take(60).collect::<String>()
                    );
                }
            }
        }
    }
    eprintln!(
        "  main_tier: {parsed}/{} parsed, {failed} failed",
        entries.len()
    );
    assert_eq!(
        failed,
        0,
        "{failed}/{} main tier lines failed to parse",
        entries.len()
    );
}

// ═══════════════════════════════════════════════════════════════
// Main tier parser — snapshot specific fixture entries
// ═══════════════════════════════════════════════════════════════

/// Parse a single main tier fixture entry and snapshot it.
/// We snapshot individual entries to keep snapshots readable.
fn parse_and_snapshot_main_tier(name: &str, input: &str) {
    let result = parser::parse_main_tier(input);
    assert!(
        result.is_some(),
        "failed to parse: {}",
        input.chars().take(60).collect::<String>()
    );
    assert_yaml_snapshot!(name, result.unwrap());
}

#[test]
fn snapshot_parsed_main_tier_samples() {
    let entries = load_fixture("main_tier");
    if entries.is_empty() {
        return;
    }

    for (i, entry) in entries.iter().enumerate().take(10) {
        let input = if entry.ends_with('\n') {
            entry.clone()
        } else {
            format!("{entry}\n")
        };
        parse_and_snapshot_main_tier(&format!("main_tier_{i}"), &input);
    }
}

#[test]
fn snapshot_parsed_main_tier_rich_samples() {
    let entries = load_fixture("main_tier_rich");
    if entries.is_empty() {
        return;
    }

    // Snapshot the first 15 rich entries (these have brackets, annotations, etc.)
    for (i, entry) in entries.iter().enumerate().take(15) {
        let input = if entry.ends_with('\n') {
            entry.clone()
        } else {
            format!("{entry}\n")
        };
        parse_and_snapshot_main_tier(&format!("main_tier_rich_{i}"), &input);
    }
}

#[test]
fn parse_all_main_tier_rich() {
    let entries = load_fixture("main_tier_rich");
    if entries.is_empty() {
        return;
    }

    let mut parsed = 0;
    let mut failed = 0;
    for entry in &entries {
        let input = if entry.ends_with('\n') {
            entry.clone()
        } else {
            format!("{entry}\n")
        };
        match parser::parse_main_tier(&input) {
            Some(_) => parsed += 1,
            None => {
                failed += 1;
                if failed <= 5 {
                    eprintln!(
                        "  PARSE FAILED: {}",
                        entry.chars().take(60).collect::<String>()
                    );
                }
            }
        }
    }
    eprintln!(
        "  main_tier_rich: {parsed}/{} parsed, {failed} failed",
        entries.len()
    );
    assert_eq!(
        failed,
        0,
        "{failed}/{} rich main tier lines failed to parse",
        entries.len()
    );
}

// ═══════════════════════════════════════════════════════════════
// Lexer-only fixture verification (all line types)
// ═══════════════════════════════════════════════════════════════

#[test]
fn lex_fixture_header_id() {
    load_and_verify_lex("header_id");
}

#[test]
fn lex_fixture_header_types() {
    load_and_verify_lex("header_types");
}

#[test]
fn lex_fixture_header_languages() {
    load_and_verify_lex("header_languages");
}

#[test]
fn lex_fixture_header_participants() {
    load_and_verify_lex("header_participants");
}

#[test]
fn lex_fixture_header_media() {
    load_and_verify_lex("header_media");
}

#[test]
fn lex_fixture_header_date() {
    load_and_verify_lex("header_date");
}

#[test]
fn lex_fixture_header_comment() {
    load_and_verify_lex("header_comment");
}

#[test]
fn lex_fixture_header_bg() {
    load_and_verify_lex("header_bg");
}

#[test]
fn lex_fixture_header_eg() {
    load_and_verify_lex("header_eg");
}

#[test]
fn lex_fixture_header_pid() {
    load_and_verify_lex("header_pid");
}

#[test]
fn lex_fixture_header_situation() {
    load_and_verify_lex("header_situation");
}

#[test]
fn lex_fixture_header_activities() {
    load_and_verify_lex("header_activities");
}

#[test]
fn trailing_ca_no_break_stays_separator_in_model_conversion() {
    let ast = parser::parse_main_tier("*CHI:\tno break ≈\n").expect("expected main tier AST");
    let main_tier = main_tier_to_model(&ast);

    assert!(
        main_tier.content.terminator.is_none(),
        "trailing CA no-break must not become a terminator"
    );
    assert!(matches!(
        main_tier.content.content.last(),
        Some(UtteranceContent::Separator(Separator::CaNoBreak { .. }))
    ));
}

#[test]
fn trailing_ca_technical_break_stays_separator_in_model_conversion() {
    let ast =
        parser::parse_main_tier("*CHI:\ttechnical break ≋\n").expect("expected main tier AST");
    let main_tier = main_tier_to_model(&ast);

    assert!(
        main_tier.content.terminator.is_none(),
        "trailing CA technical-break must not become a terminator"
    );
    assert!(matches!(
        main_tier.content.content.last(),
        Some(UtteranceContent::Separator(
            Separator::CaTechnicalBreak { .. }
        ))
    ));
}

#[test]
fn lex_fixture_header_birth_of() {
    load_and_verify_lex("header_birth_of");
}

#[test]
fn lex_fixture_tier_mor() {
    load_and_verify_lex("tier_mor");
}

#[test]
fn lex_fixture_tier_gra() {
    load_and_verify_lex("tier_gra");
}

#[test]
fn lex_fixture_tier_com() {
    load_and_verify_lex("tier_com");
}

#[test]
fn lex_fixture_tier_act() {
    load_and_verify_lex("tier_act");
}

#[test]
fn lex_fixture_tier_spa() {
    load_and_verify_lex("tier_spa");
}

#[test]
fn lex_fixture_tier_eng() {
    load_and_verify_lex("tier_eng");
}

#[test]
fn lex_fixture_tier_ort() {
    load_and_verify_lex("tier_ort");
}

#[test]
fn lex_fixture_tier_wor() {
    load_and_verify_lex("tier_wor");
}

#[test]
fn lex_fixture_tier_err() {
    load_and_verify_lex("tier_err");
}

#[test]
fn lex_fixture_tier_add() {
    load_and_verify_lex("tier_add");
}

#[test]
fn lex_fixture_tier_gpx() {
    load_and_verify_lex("tier_gpx");
}

#[test]
fn lex_fixture_tier_sit() {
    load_and_verify_lex("tier_sit");
}

#[test]
fn lex_fixture_tier_int() {
    load_and_verify_lex("tier_int");
}

#[test]
fn lex_fixture_tier_pho() {
    load_and_verify_lex("tier_pho");
}

#[test]
fn lex_fixture_tier_xdb() {
    load_and_verify_lex("tier_xdb");
}

#[test]
fn lex_fixture_tier_xpho() {
    load_and_verify_lex("tier_xpho");
}

#[test]
fn lex_fixture_main_tier() {
    load_and_verify_lex("main_tier");
}

// ═══════════════════════════════════════════════════════════════
// %mor tier parsing from fixtures
// ═══════════════════════════════════════════════════════════════

#[test]
fn parse_all_mor_fixtures() {
    let entries = load_fixture("tier_mor");
    if entries.is_empty() {
        return;
    }

    let mut parsed = 0;
    let mut failed = 0;
    for entry in &entries {
        // Strip the %mor:\t prefix to get just the body
        let body = entry.strip_prefix("%mor:\t").unwrap_or(entry);
        let input = if body.ends_with('\n') {
            body.to_string()
        } else {
            format!("{body}\n")
        };
        let tier = parser::parse_mor_tier(&input);
        if tier.items.is_empty() && !input.trim().is_empty() {
            failed += 1;
            if failed <= 3 {
                eprintln!("  MOR EMPTY: {}", body.chars().take(60).collect::<String>());
            }
        } else {
            parsed += 1;
        }
    }
    eprintln!("  %mor: {parsed}/{} parsed, {failed} empty", entries.len());
    assert_eq!(failed, 0);
}

#[test]
fn snapshot_parsed_mor_samples() {
    let entries = load_fixture("tier_mor");
    if entries.is_empty() {
        return;
    }

    for (i, entry) in entries.iter().enumerate().take(5) {
        let body = entry.strip_prefix("%mor:\t").unwrap_or(entry);
        let input = if body.ends_with('\n') {
            body.to_string()
        } else {
            format!("{body}\n")
        };
        let tier = parser::parse_mor_tier(&input);
        assert_yaml_snapshot!(format!("mor_{i}"), tier);
    }
}

// ═══════════════════════════════════════════════════════════════
// %gra tier parsing from fixtures
// ═══════════════════════════════════════════════════════════════

#[test]
fn parse_all_gra_fixtures() {
    let entries = load_fixture("tier_gra");
    if entries.is_empty() {
        return;
    }

    let mut parsed = 0;
    for entry in &entries {
        let body = entry.strip_prefix("%gra:\t").unwrap_or(entry);
        let input = if body.ends_with('\n') {
            body.to_string()
        } else {
            format!("{body}\n")
        };
        let tier = parser::parse_gra_tier(&input);
        if !tier.relations.is_empty() {
            parsed += 1;
        }
    }
    eprintln!("  %gra: {parsed}/{} parsed", entries.len());
}

#[test]
fn snapshot_parsed_gra_samples() {
    let entries = load_fixture("tier_gra");
    if entries.is_empty() {
        return;
    }

    for (i, entry) in entries.iter().enumerate().take(5) {
        let body = entry.strip_prefix("%gra:\t").unwrap_or(entry);
        let input = if body.ends_with('\n') {
            body.to_string()
        } else {
            format!("{body}\n")
        };
        let tier = parser::parse_gra_tier(&input);
        assert_yaml_snapshot!(format!("gra_{i}"), tier);
    }
}

// ═══════════════════════════════════════════════════════════════
// @ID header parsing from fixtures
// ═══════════════════════════════════════════════════════════════

#[test]
fn parse_all_id_fixtures() {
    let entries = load_fixture("header_id");
    if entries.is_empty() {
        return;
    }
    let mut parsed = 0;
    for entry in &entries {
        let body = entry.strip_prefix("@ID:\t").unwrap_or(entry);
        let input = if body.ends_with('\n') {
            body.to_string()
        } else {
            format!("{body}\n")
        };
        if parser::parse_id_header(&input).is_some() {
            parsed += 1;
        }
    }
    eprintln!("  @ID: {parsed}/{} parsed", entries.len());
    assert_eq!(parsed, entries.len());
}

#[test]
fn snapshot_parsed_id_samples() {
    let entries = load_fixture("header_id");
    if entries.is_empty() {
        return;
    }
    for (i, entry) in entries.iter().enumerate().take(5) {
        let body = entry.strip_prefix("@ID:\t").unwrap_or(entry);
        let input = if body.ends_with('\n') {
            body.to_string()
        } else {
            format!("{body}\n")
        };
        let parsed = parser::parse_id_header(&input).unwrap();
        assert_yaml_snapshot!(format!("id_{i}"), parsed);
    }
}

// ═══════════════════════════════════════════════════════════════
// @Languages header parsing from fixtures
// ═══════════════════════════════════════════════════════════════

#[test]
fn parse_all_languages_fixtures() {
    let entries = load_fixture("header_languages");
    if entries.is_empty() {
        return;
    }
    for entry in &entries {
        let body = entry.strip_prefix("@Languages:\t").unwrap_or(entry);
        let input = if body.ends_with('\n') {
            body.to_string()
        } else {
            format!("{body}\n")
        };
        let result = parser::parse_languages_header(&input);
        assert!(!result.codes.is_empty(), "empty codes for: {body}");
    }
    eprintln!("  @Languages: {}/{} parsed", entries.len(), entries.len());
}

#[test]
fn snapshot_parsed_languages_samples() {
    let entries = load_fixture("header_languages");
    if entries.is_empty() {
        return;
    }
    for (i, entry) in entries.iter().enumerate().take(5) {
        let body = entry.strip_prefix("@Languages:\t").unwrap_or(entry);
        let input = if body.ends_with('\n') {
            body.to_string()
        } else {
            format!("{body}\n")
        };
        let result = parser::parse_languages_header(&input);
        assert_yaml_snapshot!(format!("languages_{i}"), result);
    }
}

// ═══════════════════════════════════════════════════════════════
// @Participants header parsing from fixtures
// ═══════════════════════════════════════════════════════════════

#[test]
fn parse_all_participants_fixtures() {
    let entries = load_fixture("header_participants");
    if entries.is_empty() {
        return;
    }
    for entry in &entries {
        let body = entry.strip_prefix("@Participants:\t").unwrap_or(entry);
        let input = if body.ends_with('\n') {
            body.to_string()
        } else {
            format!("{body}\n")
        };
        let result = parser::parse_participants_header(&input);
        assert!(
            !result.entries.is_empty(),
            "empty participants for: {}",
            body.chars().take(40).collect::<String>()
        );
    }
    eprintln!(
        "  @Participants: {}/{} parsed",
        entries.len(),
        entries.len()
    );
}

#[test]
fn snapshot_parsed_participants_samples() {
    let entries = load_fixture("header_participants");
    if entries.is_empty() {
        return;
    }
    for (i, entry) in entries.iter().enumerate().take(5) {
        let body = entry.strip_prefix("@Participants:\t").unwrap_or(entry);
        let input = if body.ends_with('\n') {
            body.to_string()
        } else {
            format!("{body}\n")
        };
        let result = parser::parse_participants_header(&input);
        assert_yaml_snapshot!(format!("participants_{i}"), result);
    }
}

// ═══════════════════════════════════════════════════════════════
// Full file parsing from fixtures
// ═══════════════════════════════════════════════════════════════

#[test]
fn snapshot_parsed_file() {
    let input = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child, MOT Mother\n@ID:\teng|corpus|CHI|3;00.||||Child|||\n*CHI:\thello .\n%mor:\tn|hello .\n@End\n";
    let file = parser::parse_chat_file(input);
    assert_yaml_snapshot!(file);
}

#[test]
fn parse_reference_corpus_file() {
    let path = format!(
        "{}/resources/corpus/reference/core/basic-conversation.cha",
        fixture_utils::workspace_root().display()
    );
    if let Ok(content) = std::fs::read_to_string(&path) {
        let file = parser::parse_chat_file(&content);
        assert!(!file.lines.is_empty(), "empty file parse");
        let utterances = file
            .lines
            .iter()
            .filter(|l| matches!(l, talkbank_parser_re2c::ast::Line::Utterance(_)))
            .count();
        assert!(utterances > 0, "no utterances parsed");
        eprintln!(
            "  basic-conversation.cha: {} lines, {} utterances",
            file.lines.len(),
            utterances
        );
    }
}

// ═══════════════════════════════════════════════════════════════
// Conversion to talkbank-model types
// ═══════════════════════════════════════════════════════════════

#[test]
#[cfg(feature = "trait_tests")]
fn convert_mor_to_model() {
    let input = "pro|I v|want n|cookie-PL .\n";
    let tier = parser::parse_mor_tier(input);
    assert_eq!(tier.items.len(), 3);

    // Convert to talkbank-model MorTier
    let model_tier: talkbank_model::dependent_tier::mor::MorTier = (&tier).into();
    assert_eq!(model_tier.items.len(), 3);

    // Verify first item: pro|I
    let first = &model_tier.items[0];
    assert_eq!(first.main.pos.as_str(), "pro");
    assert_eq!(first.main.lemma.as_str(), "I");
    assert!(first.main.features.is_empty());

    // Verify third item: n|cookie-PL
    let third = &model_tier.items[2];
    assert_eq!(third.main.pos.as_str(), "n");
    assert_eq!(third.main.lemma.as_str(), "cookie");
    assert_eq!(third.main.features.len(), 1);
}

#[test]
#[cfg(feature = "trait_tests")]
fn convert_mor_with_clitic_to_model() {
    let input = "pron|it~aux|be-Fin-Ind-Pres-S3 .\n";
    let tier = parser::parse_mor_tier(input);
    let model_tier: talkbank_model::dependent_tier::mor::MorTier = (&tier).into();

    assert_eq!(model_tier.items.len(), 1);
    let item = &model_tier.items[0];
    assert_eq!(item.main.pos.as_str(), "pron");
    assert_eq!(item.main.lemma.as_str(), "it");
    assert_eq!(item.post_clitics.len(), 1);
    assert_eq!(item.post_clitics[0].pos.as_str(), "aux");
    assert_eq!(item.post_clitics[0].lemma.as_str(), "be");
    assert_eq!(item.post_clitics[0].features.len(), 4);
}

// ═══════════════════════════════════════════════════════════════
// Single-item parsers
// ═══════════════════════════════════════════════════════════════

#[test]
fn parse_single_word() {
    let w = parser::parse_word("hello").unwrap();
    assert_eq!(w.body.len(), 1);
    assert!(matches!(w.body[0], WordBodyItem::Text("hello")));
}

#[test]
fn parse_single_compound_word() {
    let w = parser::parse_word("ice+cream").unwrap();
    assert!(w.body.len() >= 3);
    assert!(
        w.body
            .iter()
            .any(|t| matches!(t, WordBodyItem::CompoundMarker))
    );
}

#[test]
fn parse_single_mor_word() {
    let w = parser::parse_mor_word("verb|want-Fin-Ind-Pres").unwrap();
    assert_eq!(w.pos, "verb");
    assert_eq!(w.lemma, "want");
    assert_eq!(w.features, vec!["Fin", "Ind", "Pres"]);
}

#[test]
fn parse_single_gra_relation() {
    let r = parser::parse_gra_relation("1|2|SUBJ").unwrap();
    assert_eq!(r.index, "1");
    assert_eq!(r.head, "2");
    assert_eq!(r.relation, "SUBJ");
}

// ═══════════════════════════════════════════════════════════════
// %pho tier parsing from fixtures
// ═══════════════════════════════════════════════════════════════

#[test]
fn parse_all_pho_fixtures() {
    let entries = load_fixture("tier_pho");
    if entries.is_empty() {
        return;
    }
    let mut parsed = 0;
    for entry in &entries {
        let body = entry
            .strip_prefix("%pho:\t")
            .or_else(|| entry.strip_prefix("%mod:\t"))
            .unwrap_or(entry);
        let input = if body.ends_with('\n') {
            body.to_string()
        } else {
            format!("{body}\n")
        };
        let tier = parser::parse_pho_tier(&input);
        if !tier.items.is_empty() {
            parsed += 1;
        }
    }
    eprintln!("  %pho: {parsed}/{} with words", entries.len());
}

#[test]
fn snapshot_parsed_pho_samples() {
    let entries = load_fixture("tier_pho");
    if entries.is_empty() {
        return;
    }
    for (i, entry) in entries.iter().enumerate().take(5) {
        let body = entry
            .strip_prefix("%pho:\t")
            .or_else(|| entry.strip_prefix("%mod:\t"))
            .unwrap_or(entry);
        let input = if body.ends_with('\n') {
            body.to_string()
        } else {
            format!("{body}\n")
        };
        let tier = parser::parse_pho_tier(&input);
        assert_yaml_snapshot!(format!("pho_parsed_{i}"), tier);
    }
}

// ═══════════════════════════════════════════════════════════════
// Text tier parsing from fixtures (%com, %act, %eng, etc.)
// ═══════════════════════════════════════════════════════════════

#[test]
fn parse_all_com_fixtures() {
    let entries = load_fixture("tier_com");
    if entries.is_empty() {
        return;
    }
    let mut parsed = 0;
    for entry in &entries {
        let body = entry.strip_prefix("%com:\t").unwrap_or(entry);
        let input = if body.ends_with('\n') {
            body.to_string()
        } else {
            format!("{body}\n")
        };
        let tier = parser::parse_text_tier(&input);
        if !tier.segments.is_empty() {
            parsed += 1;
        }
    }
    eprintln!("  %com: {parsed}/{} with segments", entries.len());
    assert!(parsed > 0);
}

#[test]
fn snapshot_parsed_com_samples() {
    let entries = load_fixture("tier_com");
    if entries.is_empty() {
        return;
    }
    for (i, entry) in entries.iter().enumerate().take(5) {
        let body = entry.strip_prefix("%com:\t").unwrap_or(entry);
        let input = if body.ends_with('\n') {
            body.to_string()
        } else {
            format!("{body}\n")
        };
        let tier = parser::parse_text_tier(&input);
        assert_yaml_snapshot!(format!("com_parsed_{i}"), tier);
    }
}

#[test]
fn parse_all_act_fixtures() {
    let entries = load_fixture("tier_act");
    if entries.is_empty() {
        return;
    }
    for entry in &entries {
        let body = entry.strip_prefix("%act:\t").unwrap_or(entry);
        let input = if body.ends_with('\n') {
            body.to_string()
        } else {
            format!("{body}\n")
        };
        let tier = parser::parse_text_tier(&input);
        assert!(
            !tier.segments.is_empty(),
            "empty tier for: {}",
            body.chars().take(40).collect::<String>()
        );
    }
}

#[test]
fn parse_text_tier_with_bullet() {
    // Text with inline bullet
    let input = "some text \u{0015}100_200\u{0015} more text\n";
    let tier = parser::parse_text_tier(input);
    assert!(
        tier.segments
            .iter()
            .any(|s| matches!(s, talkbank_parser_re2c::ast::TextTierSegment::Text(_)))
    );
    assert!(
        tier.segments
            .iter()
            .any(|s| matches!(s, talkbank_parser_re2c::ast::TextTierSegment::Bullet(_)))
    );
}

// ═══════════════════════════════════════════════════════════════
// Conversion to talkbank-model types
// ═══════════════════════════════════════════════════════════════

// ═══════════════════════════════════════════════════════════════
// Remaining tier parsing from fixtures
// ═══════════════════════════════════════════════════════════════

#[test]
fn parse_all_eng_fixtures() {
    let entries = load_fixture("tier_eng");
    if entries.is_empty() {
        return;
    }
    for entry in &entries {
        let body = entry.strip_prefix("%eng:\t").unwrap_or(entry);
        let input = if body.ends_with('\n') {
            body.to_string()
        } else {
            format!("{body}\n")
        };
        let tier = parser::parse_text_tier(&input);
        assert!(
            !tier.segments.is_empty(),
            "empty for: {}",
            body.chars().take(40).collect::<String>()
        );
    }
}

#[test]
fn parse_all_ort_fixtures() {
    let entries = load_fixture("tier_ort");
    if entries.is_empty() {
        return;
    }
    for entry in &entries {
        let body = entry.strip_prefix("%ort:\t").unwrap_or(entry);
        let input = if body.ends_with('\n') {
            body.to_string()
        } else {
            format!("{body}\n")
        };
        let tier = parser::parse_text_tier(&input);
        assert!(!tier.segments.is_empty());
    }
}

#[test]
fn parse_all_spa_fixtures() {
    let entries = load_fixture("tier_spa");
    if entries.is_empty() {
        return;
    }
    for entry in &entries {
        let body = entry.strip_prefix("%spa:\t").unwrap_or(entry);
        let input = if body.ends_with('\n') {
            body.to_string()
        } else {
            format!("{body}\n")
        };
        let tier = parser::parse_text_tier(&input);
        assert!(!tier.segments.is_empty());
    }
}

#[test]
fn parse_all_gpx_fixtures() {
    let entries = load_fixture("tier_gpx");
    if entries.is_empty() {
        return;
    }
    for entry in &entries {
        let body = entry.strip_prefix("%gpx:\t").unwrap_or(entry);
        let input = if body.ends_with('\n') {
            body.to_string()
        } else {
            format!("{body}\n")
        };
        let tier = parser::parse_text_tier(&input);
        assert!(!tier.segments.is_empty());
    }
}

#[test]
fn parse_all_sit_fixtures() {
    let entries = load_fixture("tier_sit");
    if entries.is_empty() {
        return;
    }
    for entry in &entries {
        let body = entry.strip_prefix("%sit:\t").unwrap_or(entry);
        let input = if body.ends_with('\n') {
            body.to_string()
        } else {
            format!("{body}\n")
        };
        let tier = parser::parse_text_tier(&input);
        assert!(!tier.segments.is_empty());
    }
}

#[test]
fn parse_all_add_fixtures() {
    let entries = load_fixture("tier_add");
    if entries.is_empty() {
        return;
    }
    for entry in &entries {
        let body = entry.strip_prefix("%add:\t").unwrap_or(entry);
        let input = if body.ends_with('\n') {
            body.to_string()
        } else {
            format!("{body}\n")
        };
        let tier = parser::parse_text_tier(&input);
        assert!(!tier.segments.is_empty());
    }
}

#[test]
fn parse_all_err_fixtures() {
    let entries = load_fixture("tier_err");
    if entries.is_empty() {
        return;
    }
    for entry in &entries {
        let body = entry.strip_prefix("%err:\t").unwrap_or(entry);
        let input = if body.ends_with('\n') {
            body.to_string()
        } else {
            format!("{body}\n")
        };
        let tier = parser::parse_text_tier(&input);
        assert!(!tier.segments.is_empty());
    }
}

// ═══════════════════════════════════════════════════════════════
// Conversion to talkbank-model types
// ═══════════════════════════════════════════════════════════════

#[test]
#[cfg(feature = "trait_tests")]
fn convert_gra_to_model() {
    let input = "1|2|SUBJ 2|0|ROOT 3|2|OBJ\n";
    let tier = parser::parse_gra_tier(input);
    let model_tier: talkbank_model::model::GraTier = (&tier).into();
    assert_eq!(model_tier.relations.len(), 3);
    assert_eq!(model_tier.relations[0].index, 1);
    assert_eq!(model_tier.relations[0].head, 2);
    assert_eq!(model_tier.relations[1].head, 0); // ROOT
}

#[test]
#[cfg(feature = "trait_tests")]
fn convert_id_to_model() {
    let input = "eng|corpus|CHI|3;00.|female|typical||Child|||\n";
    let parsed = parser::parse_id_header(input).unwrap();
    let model: talkbank_model::model::IDHeader = (&parsed).into();
    assert_eq!(model.speaker.to_string(), "CHI");
    assert_eq!(model.role.to_string(), "Child");
}

#[test]
#[cfg(feature = "trait_tests")]
fn convert_participants_to_model() {
    let input = "CHI Target_Child, MOT Mother\n";
    let parsed = parser::parse_participants_header(input);
    let entries: Vec<talkbank_model::model::ParticipantEntry> =
        parsed.entries.iter().map(|e| e.into()).collect();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].speaker_code.to_string(), "CHI");
    assert_eq!(entries[0].role.to_string(), "Target_Child");
    assert_eq!(entries[1].speaker_code.to_string(), "MOT");
}

// ═══════════════════════════════════════════════════════════════
// ChatParser trait tests
// ═══════════════════════════════════════════════════════════════

#[test]
#[cfg(feature = "trait_tests")]
fn trait_parse_mor_tier() {
    use talkbank_model::ChatParser;
    use talkbank_model::errors::ErrorCollector;
    let p = talkbank_parser_re2c::Re2cParser::new();
    let errors = ErrorCollector::new();
    let result = p.parse_mor_tier("pro|I v|want n|cookie-PL .\n", 0, &errors);
    assert!(result.is_parsed());
    let tier = result.into_option().unwrap();
    assert_eq!(tier.items.len(), 3);
}

#[test]
#[cfg(feature = "trait_tests")]
fn trait_parse_gra_tier() {
    use talkbank_model::ChatParser;
    use talkbank_model::errors::ErrorCollector;
    let p = talkbank_parser_re2c::Re2cParser::new();
    let errors = ErrorCollector::new();
    let result = p.parse_gra_tier("1|2|SUBJ 2|0|ROOT 3|2|OBJ\n", 0, &errors);
    assert!(result.is_parsed());
    let tier = result.into_option().unwrap();
    assert_eq!(tier.relations.len(), 3);
}

#[test]
#[cfg(feature = "trait_tests")]
fn trait_parse_id_header() {
    use talkbank_model::ChatParser;
    use talkbank_model::errors::ErrorCollector;
    let p = talkbank_parser_re2c::Re2cParser::new();
    let errors = ErrorCollector::new();
    let result = p.parse_id_header(
        "eng|corpus|CHI|3;00.|female|typical||Child|||\n",
        0,
        &errors,
    );
    assert!(result.is_parsed());
    let id = result.into_option().unwrap();
    assert_eq!(id.speaker.to_string(), "CHI");
}

#[test]
#[cfg(feature = "trait_tests")]
fn trait_parse_mor_word() {
    use talkbank_model::ChatParser;
    use talkbank_model::errors::ErrorCollector;
    let p = talkbank_parser_re2c::Re2cParser::new();
    let errors = ErrorCollector::new();
    let result = p.parse_mor_word("verb|want-Fin-Ind-Pres\n", 0, &errors);
    assert!(result.is_parsed());
    let word = result.into_option().unwrap();
    assert_eq!(word.pos.as_str(), "verb");
    assert_eq!(word.lemma.as_str(), "want");
}

#[test]
#[cfg(feature = "trait_tests")]
fn trait_parse_gra_relation() {
    use talkbank_model::ChatParser;
    use talkbank_model::errors::ErrorCollector;
    let p = talkbank_parser_re2c::Re2cParser::new();
    let errors = ErrorCollector::new();
    let result = p.parse_gra_relation("1|2|SUBJ\n", 0, &errors);
    assert!(result.is_parsed());
    let rel = result.into_option().unwrap();
    assert_eq!(rel.index, 1);
    assert_eq!(rel.head, 2);
}

#[test]
#[cfg(feature = "trait_tests")]
fn trait_parse_main_tier() {
    use talkbank_model::ChatParser;
    use talkbank_model::errors::ErrorCollector;
    let p = talkbank_parser_re2c::Re2cParser::new();
    let errors = ErrorCollector::new();
    let result = p.parse_main_tier("*CHI:\thello world .\n", 0, &errors);
    assert!(result.is_parsed());
    let mt = result.into_option().unwrap();
    assert_eq!(mt.speaker.to_string(), "CHI");
}

#[test]
#[cfg(feature = "trait_tests")]
fn trait_parse_word() {
    use talkbank_model::ChatParser;
    use talkbank_model::errors::ErrorCollector;
    let p = talkbank_parser_re2c::Re2cParser::new();
    let errors = ErrorCollector::new();
    let result = p.parse_word("hello", 0, &errors);
    assert!(result.is_parsed());
}

#[test]
#[cfg(feature = "trait_tests")]
fn trait_parse_word_compound() {
    use talkbank_model::ChatParser;
    use talkbank_model::errors::ErrorCollector;
    let p = talkbank_parser_re2c::Re2cParser::new();
    let errors = ErrorCollector::new();
    let result = p.parse_word("ice+cream", 0, &errors);
    assert!(result.is_parsed());
    let word = result.into_option().unwrap();
    assert!(word.raw_text().contains("+"));
}

#[test]
#[cfg(feature = "trait_tests")]
fn trait_parse_act_tier() {
    use talkbank_model::ChatParser;
    use talkbank_model::errors::ErrorCollector;
    let p = talkbank_parser_re2c::Re2cParser::new();
    let errors = ErrorCollector::new();
    let result = p.parse_act_tier("CHI is playing with blocks\n", 0, &errors);
    assert!(result.is_parsed());
}

#[test]
#[cfg(feature = "trait_tests")]
fn trait_parse_com_tier() {
    use talkbank_model::ChatParser;
    use talkbank_model::errors::ErrorCollector;
    let p = talkbank_parser_re2c::Re2cParser::new();
    let errors = ErrorCollector::new();
    let result = p.parse_com_tier("child is waving to camera\n", 0, &errors);
    assert!(result.is_parsed());
}

#[test]
#[cfg(feature = "trait_tests")]
fn trait_parse_text_tier_with_bullet() {
    use talkbank_model::ChatParser;
    use talkbank_model::errors::ErrorCollector;
    let p = talkbank_parser_re2c::Re2cParser::new();
    let errors = ErrorCollector::new();
    let result = p.parse_act_tier("hello \u{0015}100_200\u{0015} world\n", 0, &errors);
    assert!(result.is_parsed());
    let tier = result.into_option().unwrap();
    assert!(!tier.content.segments.is_empty());
}

#[test]
#[cfg(feature = "trait_tests")]
fn trait_parse_all_text_tiers() {
    use talkbank_model::ChatParser;
    use talkbank_model::errors::ErrorCollector;
    let p = talkbank_parser_re2c::Re2cParser::new();
    let errors = ErrorCollector::new();
    let input = "some text content\n";
    assert!(p.parse_act_tier(input, 0, &errors).is_parsed());
    assert!(p.parse_cod_tier(input, 0, &errors).is_parsed());
    assert!(p.parse_com_tier(input, 0, &errors).is_parsed());
    assert!(p.parse_exp_tier(input, 0, &errors).is_parsed());
    assert!(p.parse_add_tier(input, 0, &errors).is_parsed());
    assert!(p.parse_gpx_tier(input, 0, &errors).is_parsed());
    assert!(p.parse_int_tier(input, 0, &errors).is_parsed());
    assert!(p.parse_spa_tier(input, 0, &errors).is_parsed());
    assert!(p.parse_sit_tier(input, 0, &errors).is_parsed());
}

#[test]
#[cfg(feature = "trait_tests")]
fn trait_parse_pho_tier() {
    use talkbank_model::ChatParser;
    use talkbank_model::errors::ErrorCollector;
    let p = talkbank_parser_re2c::Re2cParser::new();
    let errors = ErrorCollector::new();
    let result = p.parse_pho_tier("hɛloʊ wɝld\n", 0, &errors);
    assert!(result.is_parsed());
    let tier = result.into_option().unwrap();
    assert_eq!(tier.items.len(), 2);
}

#[test]
#[cfg(feature = "trait_tests")]
fn trait_parse_pho_word() {
    use talkbank_model::ChatParser;
    use talkbank_model::errors::ErrorCollector;
    let p = talkbank_parser_re2c::Re2cParser::new();
    let errors = ErrorCollector::new();
    let result = p.parse_pho_word("hɛloʊ\n", 0, &errors);
    assert!(result.is_parsed());
}

#[test]
#[cfg(feature = "trait_tests")]
fn trait_parse_chat_file() {
    use talkbank_model::ChatParser;
    use talkbank_model::errors::ErrorCollector;
    let p = talkbank_parser_re2c::Re2cParser::new();
    let errors = ErrorCollector::new();
    let input = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child, MOT Mother\n@ID:\teng|corpus|CHI|3;00.||||Child|||\n*CHI:\thello .\n%mor:\tn|hello .\n@End\n";
    let result = p.parse_chat_file(input, 0, &errors);
    assert!(result.is_parsed(), "parse_chat_file should succeed");
    let file = result.into_option().unwrap();
    assert!(!file.lines.is_empty());
}

#[test]
#[cfg(feature = "trait_tests")]
fn trait_parse_header() {
    use talkbank_model::ChatParser;
    use talkbank_model::errors::ErrorCollector;
    let p = talkbank_parser_re2c::Re2cParser::new();
    let errors = ErrorCollector::new();
    let result = p.parse_header("@Comment:\tsome text\n", 0, &errors);
    assert!(result.is_parsed());
}

#[test]
#[cfg(feature = "trait_tests")]
fn trait_parse_utterance() {
    use talkbank_model::ChatParser;
    use talkbank_model::errors::ErrorCollector;
    let p = talkbank_parser_re2c::Re2cParser::new();
    let errors = ErrorCollector::new();
    let result = p.parse_utterance("*CHI:\thello .\n%mor:\tn|hello .\n", 0, &errors);
    assert!(result.is_parsed());
    let utt = result.into_option().unwrap();
    assert_eq!(utt.main.speaker.to_string(), "CHI");
    assert!(!utt.dependent_tiers.is_empty());
}

#[test]
#[cfg(feature = "trait_tests")]
fn trait_parser_name() {
    use talkbank_model::ChatParser;
    let p = talkbank_parser_re2c::Re2cParser::new();
    assert_eq!(p.parser_name(), "Re2cParser");
}

#[test]
#[cfg(feature = "trait_tests")]
fn convert_languages_to_model() {
    let input = "eng, fra, zho\n";
    let parsed = parser::parse_languages_header(input);
    let model: talkbank_model::LanguageCodes = (&parsed).into();
    assert_eq!(model.as_slice().len(), 3);
}

#[test]
fn lex_fixture_header_location() {
    load_and_verify_lex("header_location");
}

#[test]
fn lex_fixture_main_tier_rich() {
    load_and_verify_lex("main_tier_rich");
}
