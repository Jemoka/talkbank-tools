//! Golden tier roundtrip tests: %wor and %com.
//!
//! Each test verifies that every line in the golden tier corpus
//! round-trips through parse -> serialize for both parser backends.

use talkbank_model::ErrorCollector;
use talkbank_model::ParseOutcome;
use talkbank_model::model::{ComTier, WorTier, WriteChat};
use talkbank_parser_tests::test_error::TestError;

use super::parser_impl::parser_suite;

// =============================================================================
// Golden %wor Tier Roundtrip
// =============================================================================

/// Serialize %wor tier content only (without %wor:\t prefix).
///
/// Uses WriteChat to serialize the full tier, then strips the prefix.
fn wor_tier_to_content(tier: &WorTier) -> String {
    let mut full = String::new();
    let _ = tier.write_chat(&mut full);
    full.strip_prefix("%wor:\t").unwrap_or(&full).to_string()
}

/// Verifies golden `%wor` tier inputs round-trip for every parser backend.
#[test]
fn golden_wor_tier_roundtrip_for_every_parser() -> Result<(), TestError> {
    use talkbank_parser_tests::golden::golden_wor_tiers;

    let wor_tiers = golden_wor_tiers();
    println!("\n=== Golden %wor Tier Roundtrip Test ===");
    println!("Testing {} golden %wor tiers", wor_tiers.len());
    println!("==========================================\n");

    let mut failed_tiers = Vec::new();

    for parser in parser_suite()? {
        println!(
            "[{}] Testing {} %wor tiers...",
            "tree-sitter",
            wor_tiers.len()
        );

        for (i, tier_line) in wor_tiers.iter().enumerate() {
            let sink = ErrorCollector::new();
            let parsed = parser.parse_wor_tier_fragment(tier_line, 0, &sink);

            if !sink.is_empty() {
                let error_msg = format!("Parse errors: {:?}", sink.to_vec());
                eprintln!("[{}] %wor Tier #{} FAILED: {}", "tree-sitter", i, tier_line);
                eprintln!("  Error: {}", error_msg);
                failed_tiers.push(("tree-sitter", i, tier_line, error_msg));
                continue;
            }

            let parsed = match parsed {
                ParseOutcome::Parsed(p) => p,
                ParseOutcome::Rejected => {
                    eprintln!("[{}] %wor Tier #{} FAILED: {}", "tree-sitter", i, tier_line);
                    eprintln!("  Error: Parser rejected input");
                    failed_tiers.push((
                        "tree-sitter",
                        i,
                        tier_line,
                        "Parser rejected input".to_string(),
                    ));
                    continue;
                }
            };

            let serialized = wor_tier_to_content(&parsed);

            if serialized != *tier_line {
                failed_tiers.push((
                    "tree-sitter",
                    i,
                    tier_line,
                    format!(
                        "Roundtrip mismatch:
  Original: {}
  Roundtrip: {}",
                        tier_line, serialized
                    ),
                ));
            }
        }

        if failed_tiers.is_empty() {
            println!(
                "[{}] All {} %wor tiers passed!",
                "tree-sitter",
                wor_tiers.len()
            );
        } else {
            println!(
                "[{}] {} %wor tiers failed",
                "tree-sitter",
                failed_tiers.len()
            );
        }
    }

    if !failed_tiers.is_empty() {
        eprintln!("\n=== Failed %wor Tiers ===");
        for (parser_name, index, tier, reason) in &failed_tiers {
            eprintln!("[{}] %wor Tier #{}: {}", parser_name, index, tier);
            eprintln!("  Reason: {}\n", reason);
        }
        return Err(TestError::Failure(format!(
            "{} golden %wor tiers failed roundtrip test",
            failed_tiers.len()
        )));
    }
    Ok(())
}

// =============================================================================
// Golden %com Tier Roundtrip
// =============================================================================

/// Serialize %com tier content only (without %com:\t prefix).
///
/// Uses WriteChat to serialize the full tier, then strips the prefix.
fn com_tier_to_content(tier: &ComTier) -> String {
    let mut full = String::new();
    let _ = tier.write_chat(&mut full);
    full.strip_prefix("%com:\t").unwrap_or(&full).to_string()
}

/// Verifies golden `%com` tier inputs round-trip for every parser backend.
#[test]
fn golden_com_tier_roundtrip_for_every_parser() -> Result<(), TestError> {
    use talkbank_parser_tests::golden::golden_com_tiers;

    let com_tiers = golden_com_tiers();
    println!("\n=== Golden %com Tier Roundtrip Test ===");
    println!("Testing {} golden %com tiers", com_tiers.len());
    println!("==========================================\n");

    let mut failed_tiers = Vec::new();

    for parser in parser_suite()? {
        println!(
            "[{}] Testing {} %com tiers...",
            "tree-sitter",
            com_tiers.len()
        );

        for (i, tier_line) in com_tiers.iter().enumerate() {
            let sink = ErrorCollector::new();
            let parsed = parser.parse_com_tier_fragment(tier_line, 0, &sink);

            if !sink.is_empty() {
                let error_msg = format!("Parse errors: {:?}", sink.to_vec());
                eprintln!("[{}] %com Tier #{} FAILED: {}", "tree-sitter", i, tier_line);
                eprintln!("  Error: {}", error_msg);
                failed_tiers.push(("tree-sitter", i, tier_line, error_msg));
                continue;
            }

            let parsed = match parsed {
                ParseOutcome::Parsed(p) => p,
                ParseOutcome::Rejected => {
                    eprintln!("[{}] %com Tier #{} FAILED: {}", "tree-sitter", i, tier_line);
                    eprintln!("  Error: Parser rejected input");
                    failed_tiers.push((
                        "tree-sitter",
                        i,
                        tier_line,
                        "Parser rejected input".to_string(),
                    ));
                    continue;
                }
            };

            let serialized = com_tier_to_content(&parsed);

            if serialized != *tier_line {
                failed_tiers.push((
                    "tree-sitter",
                    i,
                    tier_line,
                    format!(
                        "Roundtrip mismatch:
  Original: {}
  Roundtrip: {}",
                        tier_line, serialized
                    ),
                ));
            }
        }

        if failed_tiers.is_empty() {
            println!(
                "[{}] All {} %com tiers passed!",
                "tree-sitter",
                com_tiers.len()
            );
        } else {
            println!(
                "[{}] {} %com tiers failed",
                "tree-sitter",
                failed_tiers.len()
            );
        }
    }

    if !failed_tiers.is_empty() {
        eprintln!("\n=== Failed %com Tiers ===");
        for (parser_name, index, tier, reason) in &failed_tiers {
            eprintln!("[{}] %com Tier #{}: {}", parser_name, index, tier);
            eprintln!("  Reason: {}\n", reason);
        }
        return Err(TestError::Failure(format!(
            "{} golden %com tiers failed roundtrip test",
            failed_tiers.len()
        )));
    }
    Ok(())
}
