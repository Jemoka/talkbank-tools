//! TDD tests for legacy CLAN %mor format parsing.
//!
//! These tests prove that our parser **cannot** correctly handle the legacy
//! CLAN %mor format used by virtually all existing TalkBank corpora.
//!
//! Legacy format features not supported by the current grammar/model:
//! - POS subcategories: `n:prop`, `v:aux`, `v:cop`, `pro:sub`, `det:art`
//! - Fusional inflection: `be&3S`, `make&PROG`
//! - Prefix morphemes: `anti#dis#v|establish`
//! - Compound stems: `n|+n|phone+n|man`
//!
//! Evidence from ~/data (real TalkBank corpora):
//! - 19,000+ POS subcategory occurrences (det:art, pro:rel, pro:obj, etc.)
//! - 11,831 %mor lines with fusional `&` markers
//!
//! Each test documents the EXPECTED correct behavior. Currently they FAIL
//! because the grammar/model doesn't support these constructs. Fix the
//! grammar and model to make them pass.

use talkbank_model::ErrorCollector;
use talkbank_model::ParseOutcome;
use talkbank_model::model::WriteChat;
use talkbank_parser_tests::test_error::TestError;

use super::parser_impl::parser_suite;

// =============================================================================
// POS subcategory tests
// =============================================================================

/// `n:prop|Mommy` must parse with POS="n:prop", not POS="n" with `:prop` lost.
///
/// Currently the grammar's `mor_pos` regex excludes `:`, so tree-sitter
/// error-recovers: POS="n", ERROR swallows ":prop", lemma="Mommy".
/// This silently loses the proper noun subcategory.
#[test]
#[ignore = "grammar does not yet support POS subcategories (n:prop)"]
fn legacy_mor_pos_subcategory_noun_prop() -> Result<(), TestError> {
    let input = "n:prop|Mommy .";

    for parser in parser_suite()? {
        let sink = ErrorCollector::new();
        let parsed = parser.parse_mor_tier_fragment(input, 0, &sink);

        // Must parse without errors (currently produces ERROR node)
        assert!(
            sink.is_empty(),
            "[{}] n:prop|Mommy produced parse errors: {:?}",
            "tree-sitter",
            sink.to_vec()
        );

        let tier = match parsed {
            ParseOutcome::Parsed(t) => t,
            ParseOutcome::Rejected => {
                return Err(TestError::Failure(format!(
                    "[{}] rejected n:prop|Mommy",
                    "tree-sitter"
                )));
            }
        };

        // Must have exactly 1 item
        assert_eq!(tier.items().len(), 1, "[{}] expected 1 item", "tree-sitter");

        let word = &tier.items()[0].main;
        // POS must be "n:prop", not just "n"
        assert_eq!(
            word.pos.as_str(),
            "n:prop",
            "[{}] POS subcategory lost: expected 'n:prop', got '{}'",
            "tree-sitter",
            word.pos.as_str()
        );
        assert_eq!(word.lemma.as_str(), "Mommy");
    }
    Ok(())
}

/// `v:aux|be&3S` must parse with POS="v:aux", lemma="be", fusional="3S".
///
/// Currently: POS="v", `:aux` dropped, lemma="be&3S" (fusional stuck in lemma).
#[test]
#[ignore = "grammar does not yet support POS subcategories (v:aux) or fusional inflection (&3S)"]
fn legacy_mor_pos_subcategory_verb_aux_with_fusional() -> Result<(), TestError> {
    let input = "v:aux|be&3S .";

    for parser in parser_suite()? {
        let sink = ErrorCollector::new();
        let parsed = parser.parse_mor_tier_fragment(input, 0, &sink);

        assert!(
            sink.is_empty(),
            "[{}] v:aux|be&3S produced parse errors: {:?}",
            "tree-sitter",
            sink.to_vec()
        );

        let tier = match parsed {
            ParseOutcome::Parsed(t) => t,
            ParseOutcome::Rejected => {
                return Err(TestError::Failure(format!(
                    "[{}] rejected v:aux|be&3S",
                    "tree-sitter"
                )));
            }
        };

        assert_eq!(tier.items().len(), 1);
        let word = &tier.items()[0].main;

        // POS must preserve subcategory
        assert_eq!(
            word.pos.as_str(),
            "v:aux",
            "[{}] POS subcategory lost: expected 'v:aux', got '{}'",
            "tree-sitter",
            word.pos.as_str()
        );

        // Lemma must be just "be", not "be&3S"
        assert_eq!(
            word.lemma.as_str(),
            "be",
            "[{}] Fusional marker stuck in lemma: expected 'be', got '{}'",
            "tree-sitter",
            word.lemma.as_str()
        );
    }
    Ok(())
}

/// `det:art|the` — the most common subcategory in real data (5,204 occurrences).
#[test]
#[ignore = "grammar does not yet support POS subcategories (det:art)"]
fn legacy_mor_pos_subcategory_det_art() -> Result<(), TestError> {
    let input = "det:art|the .";

    for parser in parser_suite()? {
        let sink = ErrorCollector::new();
        let parsed = parser.parse_mor_tier_fragment(input, 0, &sink);

        assert!(
            sink.is_empty(),
            "[{}] det:art|the produced parse errors: {:?}",
            "tree-sitter",
            sink.to_vec()
        );

        let tier = match parsed {
            ParseOutcome::Parsed(t) => t,
            ParseOutcome::Rejected => {
                return Err(TestError::Failure(format!(
                    "[{}] rejected det:art|the",
                    "tree-sitter"
                )));
            }
        };

        assert_eq!(tier.items().len(), 1);
        assert_eq!(tier.items()[0].main.pos.as_str(), "det:art");
        assert_eq!(tier.items()[0].main.lemma.as_str(), "the");
    }
    Ok(())
}

/// `pro:sub|I` — common pronoun subcategory (1,383 occurrences).
#[test]
#[ignore = "grammar does not yet support POS subcategories (pro:sub)"]
fn legacy_mor_pos_subcategory_pro_sub() -> Result<(), TestError> {
    let input = "pro:sub|I v|want n|cookie-PL .";

    for parser in parser_suite()? {
        let sink = ErrorCollector::new();
        let parsed = parser.parse_mor_tier_fragment(input, 0, &sink);

        assert!(
            sink.is_empty(),
            "[{}] pro:sub|I produced parse errors: {:?}",
            "tree-sitter",
            sink.to_vec()
        );

        let tier = match parsed {
            ParseOutcome::Parsed(t) => t,
            ParseOutcome::Rejected => {
                return Err(TestError::Failure(format!(
                    "[{}] rejected pro:sub|I ...",
                    "tree-sitter"
                )));
            }
        };

        assert_eq!(tier.items().len(), 3);
        assert_eq!(tier.items()[0].main.pos.as_str(), "pro:sub");
        assert_eq!(tier.items()[0].main.lemma.as_str(), "I");
    }
    Ok(())
}

/// Nested subcategory: `pro:poss:det|your` (from spec example gra_2).
#[test]
#[ignore = "grammar does not yet support nested POS subcategories (pro:poss:det)"]
fn legacy_mor_pos_nested_subcategory() -> Result<(), TestError> {
    let input = "pro:poss:det|your .";

    for parser in parser_suite()? {
        let sink = ErrorCollector::new();
        let parsed = parser.parse_mor_tier_fragment(input, 0, &sink);

        assert!(
            sink.is_empty(),
            "[{}] pro:poss:det|your produced parse errors: {:?}",
            "tree-sitter",
            sink.to_vec()
        );

        let tier = match parsed {
            ParseOutcome::Parsed(t) => t,
            ParseOutcome::Rejected => {
                return Err(TestError::Failure(format!(
                    "[{}] rejected pro:poss:det|your",
                    "tree-sitter"
                )));
            }
        };

        assert_eq!(tier.items().len(), 1);
        assert_eq!(tier.items()[0].main.pos.as_str(), "pro:poss:det");
        assert_eq!(tier.items()[0].main.lemma.as_str(), "your");
    }
    Ok(())
}

// =============================================================================
// Fusional inflection tests
// =============================================================================

/// `v|make&PROG` must separate lemma "make" from fusional marker "PROG".
///
/// Currently the `&PROG` stays in the lemma string: lemma="make&PROG".
#[test]
#[ignore = "grammar does not yet support fusional inflection markers (&PROG)"]
fn legacy_mor_fusional_inflection() -> Result<(), TestError> {
    let input = "v|make&PROG .";

    for parser in parser_suite()? {
        let sink = ErrorCollector::new();
        let parsed = parser.parse_mor_tier_fragment(input, 0, &sink);

        assert!(
            sink.is_empty(),
            "[{}] v|make&PROG produced parse errors: {:?}",
            "tree-sitter",
            sink.to_vec()
        );

        let tier = match parsed {
            ParseOutcome::Parsed(t) => t,
            ParseOutcome::Rejected => {
                return Err(TestError::Failure(format!(
                    "[{}] rejected v|make&PROG",
                    "tree-sitter"
                )));
            }
        };

        assert_eq!(tier.items().len(), 1);
        let word = &tier.items()[0].main;

        // Lemma must be "make", not "make&PROG"
        assert_eq!(
            word.lemma.as_str(),
            "make",
            "[{}] Fusional marker stuck in lemma: expected 'make', got '{}'",
            "tree-sitter",
            word.lemma.as_str()
        );
    }
    Ok(())
}

// =============================================================================
// Roundtrip tests — legacy format must survive parse -> serialize -> reparse
// =============================================================================

/// Full legacy %mor line must roundtrip exactly.
///
/// Input: `n:prop|Mommy v|look prep|at pro|me .`
/// After parse -> serialize, the output must be identical to input.
#[test]
#[ignore = "depends on POS subcategory support (n:prop)"]
fn legacy_mor_roundtrip_subcategories() -> Result<(), TestError> {
    let input = "n:prop|Mommy v|look prep|at pro|me .";

    for parser in parser_suite()? {
        let sink = ErrorCollector::new();
        let parsed = parser.parse_mor_tier_fragment(input, 0, &sink);

        assert!(
            sink.is_empty(),
            "[{}] roundtrip input produced errors: {:?}",
            "tree-sitter",
            sink.to_vec()
        );

        let tier = match parsed {
            ParseOutcome::Parsed(t) => t,
            ParseOutcome::Rejected => {
                return Err(TestError::Failure(format!(
                    "[{}] rejected roundtrip input",
                    "tree-sitter"
                )));
            }
        };

        let serialized = tier.to_content();
        assert_eq!(
            serialized, input,
            "[{}] Roundtrip mismatch:\n  Input:    {}\n  Output:   {}",
            "tree-sitter", input, serialized
        );
    }
    Ok(())
}

/// Roundtrip with fusional inflection and subcategories.
///
/// Input: `pro|he v:aux|be&3S adj|noisy .`
#[test]
#[ignore = "depends on POS subcategory (v:aux) and fusional inflection (&3S) support"]
fn legacy_mor_roundtrip_fusional() -> Result<(), TestError> {
    let input = "pro|he v:aux|be&3S adj|noisy .";

    for parser in parser_suite()? {
        let sink = ErrorCollector::new();
        let parsed = parser.parse_mor_tier_fragment(input, 0, &sink);

        assert!(
            sink.is_empty(),
            "[{}] roundtrip input produced errors: {:?}",
            "tree-sitter",
            sink.to_vec()
        );

        let tier = match parsed {
            ParseOutcome::Parsed(t) => t,
            ParseOutcome::Rejected => {
                return Err(TestError::Failure(format!(
                    "[{}] rejected roundtrip input",
                    "tree-sitter"
                )));
            }
        };

        let serialized = tier.to_content();
        assert_eq!(
            serialized, input,
            "[{}] Roundtrip mismatch:\n  Input:    {}\n  Output:   {}",
            "tree-sitter", input, serialized
        );
    }
    Ok(())
}

/// Roundtrip with clitic containing subcategory and fusional.
///
/// Input: `n:prop|Mommy~v:cop|be&3S det:art|the n|cup .`
#[test]
#[ignore = "depends on POS subcategory (n:prop, v:cop, det:art) and fusional inflection support"]
fn legacy_mor_roundtrip_clitic_with_subcategory() -> Result<(), TestError> {
    let input = "n:prop|Mommy~v:cop|be&3S det:art|the n|cup .";

    for parser in parser_suite()? {
        let sink = ErrorCollector::new();
        let parsed = parser.parse_mor_tier_fragment(input, 0, &sink);

        assert!(
            sink.is_empty(),
            "[{}] roundtrip input produced errors: {:?}",
            "tree-sitter",
            sink.to_vec()
        );

        let tier = match parsed {
            ParseOutcome::Parsed(t) => t,
            ParseOutcome::Rejected => {
                return Err(TestError::Failure(format!(
                    "[{}] rejected roundtrip input",
                    "tree-sitter"
                )));
            }
        };

        let serialized = tier.to_content();
        assert_eq!(
            serialized, input,
            "[{}] Roundtrip mismatch:\n  Input:    {}\n  Output:   {}",
            "tree-sitter", input, serialized
        );
    }
    Ok(())
}
