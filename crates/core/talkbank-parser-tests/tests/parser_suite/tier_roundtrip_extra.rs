//! Golden tier roundtrip tests: %pho and %sin.
//!
//! Each test verifies that every line in the golden tier corpus
//! round-trips through parse -> serialize for both parser backends.

use talkbank_model::ErrorCollector;
use talkbank_model::ParseOutcome;
use talkbank_model::model::{PhoTier, SinTier, WriteChat};
use talkbank_parser_tests::test_error::TestError;

use super::parser_impl::parser_suite;

// =============================================================================
// Golden %pho Tier Roundtrip
// =============================================================================

/// Serialize %pho tier content only (without %pho:\t prefix).
fn pho_tier_to_content(tier: &PhoTier) -> String {
    tier.to_content()
}

/// Verifies golden `%pho` tier inputs round-trip for every parser backend.
#[test]
fn golden_pho_tier_roundtrip_for_every_parser() -> Result<(), TestError> {
    use talkbank_parser_tests::golden::golden_pho_tiers;

    let pho_tiers = golden_pho_tiers();
    println!("\n=== Golden %pho Tier Roundtrip Test ===");
    println!("Testing {} golden %pho tiers", pho_tiers.len());
    println!("==========================================\n");

    let mut failed_tiers = Vec::new();

    for parser in parser_suite()? {
        println!(
            "[{}] Testing {} %pho tiers...",
            "tree-sitter",
            pho_tiers.len()
        );

        for (i, tier_line) in pho_tiers.iter().enumerate() {
            let sink = ErrorCollector::new();
            let parsed = parser.parse_pho_tier_fragment(tier_line, 0, &sink);

            // Check for parsing errors
            if !sink.is_empty() {
                let error_msg = format!("Parse errors: {:?}", sink.to_vec());
                eprintln!("[{}] %pho Tier #{} FAILED: {}", "tree-sitter", i, tier_line);
                eprintln!("  Error: {}", error_msg);
                failed_tiers.push(("tree-sitter", i, tier_line, error_msg));
                continue;
            }

            let parsed = match parsed {
                ParseOutcome::Parsed(p) => p,
                ParseOutcome::Rejected => {
                    eprintln!("[{}] %pho Tier #{} FAILED: {}", "tree-sitter", i, tier_line);
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

            // Roundtrip test - compare content only (golden data doesn't have prefix)
            let serialized = pho_tier_to_content(&parsed);

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
                "[{}] All {} %pho tiers passed!",
                "tree-sitter",
                pho_tiers.len()
            );
        } else {
            println!(
                "[{}] {} %pho tiers failed",
                "tree-sitter",
                failed_tiers.len()
            );
        }
    }

    if !failed_tiers.is_empty() {
        eprintln!("\n=== Failed %pho Tiers ===");
        for (parser_name, index, tier, reason) in &failed_tiers {
            eprintln!("[{}] %pho Tier #{}: {}", parser_name, index, tier);
            eprintln!("  Reason: {}\n", reason);
        }
        return Err(TestError::Failure(format!(
            "{} golden %pho tiers failed roundtrip test",
            failed_tiers.len()
        )));
    }
    Ok(())
}

// =============================================================================
// Golden %sin Tier Roundtrip
// =============================================================================

/// Serialize %sin tier content only (without %sin:\t prefix).
///
/// Writes each SinItem space-separated via WriteChat.
fn sin_tier_to_content(tier: &SinTier) -> String {
    let mut content = String::new();
    for (i, item) in tier.items.iter().enumerate() {
        if i > 0 {
            content.push(' ');
        }
        let _ = item.write_chat(&mut content);
    }
    content
}

/// Verifies golden `%sin` tier inputs round-trip for every parser backend.
#[test]
fn golden_sin_tier_roundtrip_for_every_parser() -> Result<(), TestError> {
    use talkbank_parser_tests::golden::golden_sin_tiers;

    let sin_tiers = golden_sin_tiers();
    println!("\n=== Golden %sin Tier Roundtrip Test ===");
    println!("Testing {} golden %sin tiers", sin_tiers.len());
    println!("==========================================\n");

    let mut failed_tiers = Vec::new();

    for parser in parser_suite()? {
        println!(
            "[{}] Testing {} %sin tiers...",
            "tree-sitter",
            sin_tiers.len()
        );

        for (i, tier_line) in sin_tiers.iter().enumerate() {
            let sink = ErrorCollector::new();
            let parsed = parser.parse_sin_tier_fragment(tier_line, 0, &sink);

            if !sink.is_empty() {
                let error_msg = format!("Parse errors: {:?}", sink.to_vec());
                eprintln!("[{}] %sin Tier #{} FAILED: {}", "tree-sitter", i, tier_line);
                eprintln!("  Error: {}", error_msg);
                failed_tiers.push(("tree-sitter", i, tier_line, error_msg));
                continue;
            }

            let parsed = match parsed {
                ParseOutcome::Parsed(p) => p,
                ParseOutcome::Rejected => {
                    eprintln!("[{}] %sin Tier #{} FAILED: {}", "tree-sitter", i, tier_line);
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

            let serialized = sin_tier_to_content(&parsed);

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
                "[{}] All {} %sin tiers passed!",
                "tree-sitter",
                sin_tiers.len()
            );
        } else {
            println!(
                "[{}] {} %sin tiers failed",
                "tree-sitter",
                failed_tiers.len()
            );
        }
    }

    if !failed_tiers.is_empty() {
        eprintln!("\n=== Failed %sin Tiers ===");
        for (parser_name, index, tier, reason) in &failed_tiers {
            eprintln!("[{}] %sin Tier #{}: {}", parser_name, index, tier);
            eprintln!("  Reason: {}\n", reason);
        }
        return Err(TestError::Failure(format!(
            "{} golden %sin tiers failed roundtrip test",
            failed_tiers.len()
        )));
    }
    Ok(())
}
