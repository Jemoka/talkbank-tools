//! Test module for test parse health recovery in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

use talkbank_model::model::{Line, ParseHealth, ParseHealthState, ParseHealthTier};
use talkbank_model::{ErrorCode, ErrorCollector};
use talkbank_parser::TreeSitterParser;

/// Returns whether a specific parse-health tier remains clean.
fn tier_is_clean(health: &ParseHealth, tier: ParseHealthTier) -> bool {
    health.is_tier_clean(tier)
}

/// Parses first utterance.
fn parse_first_utterance(
    input: &str,
) -> (
    talkbank_model::model::Utterance,
    Vec<talkbank_model::ParseError>,
) {
    let parser = TreeSitterParser::new().expect("grammar loads");
    let errors = ErrorCollector::new();
    let file = parser.parse_chat_file_streaming(input, &errors);
    let diagnostics = errors.to_vec();
    let utterance = file
        .lines
        .into_iter()
        .find_map(|line| match line {
            Line::Utterance(utt) => Some(*utt),
            _ => None,
        })
        .expect("expected utterance");
    (utterance, diagnostics)
}

/// Verifies malformed standard dependent tiers taint only their own alignment domain.
#[test]
fn malformed_standard_dependent_tiers_taint_only_their_alignment_domain() {
    let cases = [
        ("%mor no_tab_separator", ParseHealthTier::Mor),
        ("%gra no_tab_separator", ParseHealthTier::Gra),
        ("%pho no_tab_separator", ParseHealthTier::Pho),
        ("%wor no_tab_separator", ParseHealthTier::Wor),
    ];
    let dependent_tiers = [
        ParseHealthTier::Mor,
        ParseHealthTier::Gra,
        ParseHealthTier::Pho,
        ParseHealthTier::Wor,
        ParseHealthTier::Mod,
        ParseHealthTier::Sin,
    ];

    for (malformed_tier, expected_taint) in cases {
        let input = format!("@UTF8\n@Begin\n*CHI:\thello .\n{malformed_tier}\n@End\n");
        let (utterance, diagnostics) = parse_first_utterance(&input);
        assert!(
            !diagnostics.is_empty(),
            "Expected parse diagnostics for malformed tier {malformed_tier}"
        );

        let ParseHealthState::Tainted(health) = utterance.parse_health else {
            panic!("Expected parse health taint from malformed dependent tier");
        };
        assert!(
            health.is_tier_clean(ParseHealthTier::Main),
            "Main tier should remain clean for malformed dependent tier {malformed_tier}"
        );

        for tier in dependent_tiers {
            let expected_clean = tier != expected_taint;
            assert_eq!(
                tier_is_clean(&health, tier),
                expected_clean,
                "Unexpected parse-health state for {tier:?} from malformed tier {malformed_tier}"
            );
        }
    }
}

/// Verifies malformed unknown dependent tiers taint all alignment-dependent domains.
#[test]
fn malformed_unknown_dependent_tier_taints_all_alignment_dependents() {
    let input = "@UTF8\n@Begin\n*CHI:\thello .\n%xfoo no_tab_separator\n@End\n";
    let (utterance, diagnostics) = parse_first_utterance(input);
    assert!(
        !diagnostics.is_empty(),
        "Expected diagnostics for malformed unknown dependent tier"
    );

    let ParseHealthState::Tainted(health) = utterance.parse_health else {
        panic!("Expected parse health taint from malformed unknown dependent tier");
    };
    assert!(health.is_tier_clean(ParseHealthTier::Main));
    assert!(health.is_tier_tainted(ParseHealthTier::Mor));
    assert!(health.is_tier_tainted(ParseHealthTier::Gra));
    assert!(health.is_tier_tainted(ParseHealthTier::Pho));
    assert!(health.is_tier_tainted(ParseHealthTier::Wor));
    assert!(health.is_tier_tainted(ParseHealthTier::Mod));
    assert!(health.is_tier_tainted(ParseHealthTier::Sin));
}

/// Verifies malformed `%gra` relations are rejected without fabricated defaults.
#[test]
fn malformed_gra_relation_does_not_fabricate_default_relation_values() {
    let input = "@UTF8\n@Begin\n*CHI:\thello .\n%gra:\t0|0|ROOT\n@End\n";
    let (utterance, diagnostics) = parse_first_utterance(input);

    assert!(
        diagnostics
            .iter()
            .any(|err| err.code == ErrorCode::InvalidGrammarIndex),
        "Expected InvalidGrammarIndex diagnostic, got: {:?}",
        diagnostics
            .iter()
            .map(|err| (&err.code, &err.message))
            .collect::<Vec<_>>()
    );

    let gra = utterance
        .gra_tier()
        .expect("Expected %gra tier to remain attached for downstream diagnostics");
    assert!(
        gra.relations().is_empty(),
        "Malformed relation must not be recovered as a fabricated default entry: {:?}",
        gra.relations()
    );

    let ParseHealthState::Tainted(health) = utterance.parse_health else {
        panic!("Expected parse health taint from malformed %gra relation");
    };
    assert!(
        health.is_tier_tainted(ParseHealthTier::Gra),
        "Malformed %gra relation must taint gra alignment domain"
    );
}

/// Verifies a missing speaker marker does not fabricate an empty speaker utterance.
#[test]
fn missing_speaker_does_not_create_empty_speaker_utterance() {
    let parser = TreeSitterParser::new().expect("grammar loads");
    let input = "@UTF8\n@Begin\n*:\thello .\n@End\n";
    let errors = ErrorCollector::new();
    let file = parser.parse_chat_file_streaming(input, &errors);
    let diagnostics = errors.to_vec();
    assert!(
        diagnostics.iter().any(|err| matches!(
            err.code,
            ErrorCode::MissingSpeaker | ErrorCode::MissingMainTier
        )),
        "Expected MissingSpeaker or MissingMainTier diagnostic, got: {:?}",
        diagnostics
            .iter()
            .map(|err| (&err.code, &err.message))
            .collect::<Vec<_>>()
    );
    assert!(
        !file.lines.iter().any(|line| {
            matches!(
                line,
                Line::Utterance(utt) if utt.main.speaker.as_str().is_empty()
            )
        }),
        "Parser must not fabricate utterances with empty speaker codes"
    );
}

/// Verifies malformed `@Participants` entries do not emit empty role placeholders.
#[test]
fn participants_missing_role_does_not_emit_empty_role_entry() {
    let parser = TreeSitterParser::new().expect("grammar loads");
    let input = "@UTF8\n@Begin\n@Participants:\tCHI\n*CHI:\thello .\n@End\n";
    let errors = ErrorCollector::new();
    let file = parser.parse_chat_file_streaming(input, &errors);
    let diagnostics = errors.to_vec();
    assert!(
        diagnostics
            .iter()
            .any(|err| err.code == ErrorCode::EmptyParticipantRole),
        "Expected EmptyParticipantRole diagnostic, got: {:?}",
        diagnostics
            .iter()
            .map(|err| (&err.code, &err.message))
            .collect::<Vec<_>>()
    );

    for line in &file.lines {
        if let Line::Header { header, .. } = line {
            if let talkbank_model::model::Header::Participants { entries } = header.as_ref() {
                for entry in entries {
                    assert!(
                        !entry.speaker_code.as_str().is_empty(),
                        "Participant entry must not contain empty speaker code"
                    );
                    assert!(
                        !entry.role.as_str().is_empty(),
                        "Participant entry must not contain empty participant role"
                    );
                }
            }
        }
    }
}

/// Verifies malformed postcodes do not emit empty placeholder values.
#[test]
fn malformed_postcode_does_not_emit_empty_postcode_placeholder() {
    let input = "@UTF8\n@Begin\n*CHI:\thello . [+ ]\n@End\n";
    let (utterance, diagnostics) = parse_first_utterance(input);
    assert!(
        !diagnostics.is_empty(),
        "Expected diagnostics for malformed postcode"
    );
    assert!(
        utterance
            .main
            .content
            .postcodes
            .iter()
            .all(|postcode| !postcode.text.is_empty()),
        "Parser must not fabricate empty postcode values: {:?}",
        utterance.main.content.postcodes
    );
}

/// Verifies empty `@Date` headers are preserved for validation-phase checks.
#[test]
fn empty_date_header_is_preserved_for_validation_phase() {
    let parser = TreeSitterParser::new().expect("grammar loads");
    let input = "@UTF8\n@Begin\n@Date:\t\n*CHI:\thello .\n@End\n";
    let errors = ErrorCollector::new();
    let file = parser.parse_chat_file_streaming(input, &errors);
    let diagnostics = errors.to_vec();
    assert!(
        diagnostics.is_empty(),
        "Empty @Date should remain parse-successful and be validated later, got parse diagnostics: {:?}",
        diagnostics
    );

    let mut saw_empty_date_header = false;
    for line in &file.lines {
        if let Line::Header { header, .. } = line
            && let talkbank_model::model::Header::Date { date } = header.as_ref()
            && date.as_str().is_empty()
        {
            saw_empty_date_header = true;
        }
    }

    assert!(
        saw_empty_date_header,
        "Expected parser recovery to preserve an empty @Date value for validation checks"
    );
}
