//! Validation tests for golden tier files.
//!
//! These tests ensure the golden tier files stay valid according to TreeSitterParser.
//!
//! If tests fail after grammar/parser changes, regenerate the golden files:
//! ```bash
//! cargo run --release -p talkbank-parser-tests --bin generate_golden_mor_tiers
//! cargo run --release -p talkbank-parser-tests --bin generate_golden_gra_tiers
//! cargo run --release -p talkbank-parser-tests --bin generate_golden_pho_tiers
//! ```

use talkbank_model::ErrorCollector;
use talkbank_model::ParseOutcome;
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::test_error::TestError;

/// Validates tiers.
fn validate_tiers<T>(
    parser: &TreeSitterParser,
    tiers: &[String],
    tier_name: &str,
    parse: fn(&TreeSitterParser, &str, usize, &ErrorCollector) -> ParseOutcome<T>,
    regen_hint: &str,
) -> Result<(), TestError> {
    let mut invalid_count = 0;

    for tier in tiers {
        let errors = ErrorCollector::new();
        let result = parse(parser, tier, 0, &errors);

        if result.is_rejected() || !errors.is_empty() {
            invalid_count += 1;
            eprintln!("INVALID {} TIER: {:?}", tier_name, tier);
            for err in errors.to_vec() {
                eprintln!("  {}", err.message);
            }
        }
    }

    if invalid_count > 0 {
        return Err(TestError::Failure(format!(
            "{} out of {} golden {} tiers are invalid!\n\
             \n\
             This means the grammar/parser changed and the golden tiers are out of sync.\n\
             \n\
             To fix:\n\
             1. Verify the parser changes are intentional\n\
             2. Regenerate golden {} tiers:\n\
                {}\n\
             3. Review the diff to ensure it makes sense\n\
             4. Commit the updated golden tiers file\n\
             \n\
             See stderr for list of invalid tiers.",
            invalid_count,
            tiers.len(),
            tier_name,
            tier_name,
            regen_hint
        )));
    }

    Ok(())
}

/// Test that ensures golden_mor_tiers.txt stays valid according to TreeSitterParser.
#[test]
fn golden_mor_tiers_are_valid_for_tree_sitter() -> Result<(), TestError> {
    let parser = TreeSitterParser::new().map_err(|err| TestError::ParserInit(err.to_string()))?;
    let golden_tiers = talkbank_parser_tests::golden::golden_mor_tiers();
    let tiers: Vec<String> = golden_tiers.iter().map(|s| s.to_string()).collect();
    validate_tiers(
        &parser,
        &tiers,
        "%mor",
        TreeSitterParser::parse_mor_tier_fragment,
        "cargo run --release -p talkbank-parser-tests --bin generate_golden_mor_tiers",
    )
}

/// Test that ensures golden_gra_tiers.txt stays valid according to TreeSitterParser.
#[test]
fn golden_gra_tiers_are_valid_for_tree_sitter() -> Result<(), TestError> {
    let parser = TreeSitterParser::new().map_err(|err| TestError::ParserInit(err.to_string()))?;
    let golden_tiers = talkbank_parser_tests::golden::golden_gra_tiers();
    let tiers: Vec<String> = golden_tiers.iter().map(|s| s.to_string()).collect();
    validate_tiers(
        &parser,
        &tiers,
        "%gra",
        TreeSitterParser::parse_gra_tier_fragment,
        "cargo run --release -p talkbank-parser-tests --bin generate_golden_gra_tiers",
    )
}

/// Test that ensures golden_pho_tiers.txt stays valid according to TreeSitterParser.
#[test]
fn golden_pho_tiers_are_valid_for_tree_sitter() -> Result<(), TestError> {
    let parser = TreeSitterParser::new().map_err(|err| TestError::ParserInit(err.to_string()))?;
    let golden_tiers = talkbank_parser_tests::golden::golden_pho_tiers();
    let tiers: Vec<String> = golden_tiers.iter().map(|s| s.to_string()).collect();
    validate_tiers(
        &parser,
        &tiers,
        "%pho",
        TreeSitterParser::parse_pho_tier_fragment,
        "cargo run --release -p talkbank-parser-tests --bin generate_golden_pho_tiers",
    )
}
