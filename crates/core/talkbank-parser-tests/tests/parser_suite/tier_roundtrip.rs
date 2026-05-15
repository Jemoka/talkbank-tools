//! Golden tier roundtrip tests: main, %mor, and %gra.
//!
//! Each test verifies that every line in the golden tier corpus
//! round-trips through parse -> serialize for both parser backends.

use talkbank_model::ErrorCollector;
use talkbank_model::ParseOutcome;
use talkbank_model::model::{GraTier, MorTier, WriteChat};
use talkbank_parser_tests::test_error::TestError;

use super::parser_impl::parser_suite;

fn requires_fragment_context(errors: &ErrorCollector) -> bool {
    errors
        .to_vec()
        .iter()
        .any(|error| error.message.contains("requires file context"))
}

/// Serialize %mor tier content only (without %mor:\t prefix).
///
/// The golden data contains content only, so we need to strip the prefix for comparison.
fn mor_tier_to_content(tier: &MorTier) -> String {
    tier.to_content()
}

/// Serialize %gra tier content only (without %gra:\t prefix).
fn gra_tier_to_content(tier: &GraTier) -> String {
    tier.to_content()
}

/// Verifies golden main-tier inputs round-trip for every parser backend.
#[test]
fn golden_main_tier_roundtrip_for_every_parser() -> Result<(), TestError> {
    use talkbank_model::model::{SemanticEq, WriteChat};
    use talkbank_parser_tests::golden::golden_main_tiers;

    let main_tiers = golden_main_tiers();
    println!("\n=== Golden Main Tier Roundtrip Test ===");
    println!("Testing {} golden main tiers", main_tiers.len());
    println!("==========================================\n");

    let mut failed_tiers = Vec::new();
    for parser in parser_suite()? {
        let mut skipped_context_tiers = 0usize;
        println!(
            "[{}] Testing {} main tiers...",
            "tree-sitter",
            main_tiers.len()
        );

        for (i, tier_line) in main_tiers.iter().enumerate() {
            // Print progress every 100 tiers
            if i > 0 && i % 100 == 0 {
                println!(
                    "[{}] Progress: {}/{} tiers...",
                    "tree-sitter",
                    i,
                    main_tiers.len()
                );
            }

            let sink = ErrorCollector::new();
            let parsed = parser.parse_main_tier_fragment(tier_line, 0, &sink);

            // Check for parsing errors
            if !sink.is_empty() {
                if requires_fragment_context(&sink) {
                    skipped_context_tiers += 1;
                    continue;
                }
                let error_msg = format!("Parse errors: {:?}", sink.to_vec());
                eprintln!("[{}] Tier #{} FAILED: {}", "tree-sitter", i, tier_line);
                eprintln!("  Error: {}", error_msg);
                failed_tiers.push(("tree-sitter", i, tier_line, error_msg));
                continue;
            }

            let parsed = match parsed {
                ParseOutcome::Parsed(p) => p,
                ParseOutcome::Rejected => {
                    eprintln!("[{}] Tier #{} FAILED: {}", "tree-sitter", i, tier_line);
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

            // Roundtrip test
            let mut serialized = String::new();
            if let Err(e) = parsed.write_chat(&mut serialized) {
                failed_tiers.push((
                    "tree-sitter",
                    i,
                    tier_line,
                    format!("Serialization error: {}", e),
                ));
                continue;
            }

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
                "[{}] \u{2705} All {} context-free main tiers passed! ({} skipped: require file context)",
                "tree-sitter",
                main_tiers.len() - skipped_context_tiers,
                skipped_context_tiers
            );
        } else {
            println!("[{}] \u{274c} {} failed", "tree-sitter", failed_tiers.len());
        }
    }

    if !failed_tiers.is_empty() {
        eprintln!("\n=== Failed Main Tiers ===");
        for (parser_name, index, tier, reason) in &failed_tiers {
            eprintln!("[{}] Tier #{}: {}", parser_name, index, tier);
            eprintln!("  Reason: {}\n", reason);
        }
        return Err(TestError::Failure(format!(
            "{} golden main tiers failed roundtrip test",
            failed_tiers.len()
        )));
    }
    Ok(())
}

/// Verifies golden `%mor` tier inputs round-trip for every parser backend.
#[test]
fn golden_mor_tier_roundtrip_for_every_parser() -> Result<(), TestError> {
    use talkbank_parser_tests::golden::golden_mor_tiers;

    let mor_tiers = golden_mor_tiers();
    println!("\n=== Golden %mor Tier Roundtrip Test ===");
    println!("Testing {} golden %mor tiers", mor_tiers.len());
    println!("==========================================\n");

    let mut failed_tiers = Vec::new();

    for parser in parser_suite()? {
        println!(
            "[{}] Testing {} %mor tiers...",
            "tree-sitter",
            mor_tiers.len()
        );

        for (i, tier_line) in mor_tiers.iter().enumerate() {
            let sink = ErrorCollector::new();
            let parsed = parser.parse_mor_tier_fragment(tier_line, 0, &sink);

            // Check for parsing errors
            if !sink.is_empty() {
                let error_msg = format!("Parse errors: {:?}", sink.to_vec());
                eprintln!("[{}] %mor Tier #{} FAILED: {}", "tree-sitter", i, tier_line);
                eprintln!("  Error: {}", error_msg);
                failed_tiers.push(("tree-sitter", i, tier_line, error_msg));
                continue;
            }

            let parsed = match parsed {
                ParseOutcome::Parsed(p) => p,
                ParseOutcome::Rejected => {
                    eprintln!("[{}] %mor Tier #{} FAILED: {}", "tree-sitter", i, tier_line);
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
            let serialized = mor_tier_to_content(&parsed);

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
                "[{}] All {} %mor tiers passed!",
                "tree-sitter",
                mor_tiers.len()
            );
        } else {
            println!(
                "[{}] {} %mor tiers failed",
                "tree-sitter",
                failed_tiers.len()
            );
        }
    }

    if !failed_tiers.is_empty() {
        eprintln!("\n=== Failed %mor Tiers ===");
        for (parser_name, index, tier, reason) in &failed_tiers {
            eprintln!("[{}] %mor Tier #{}: {}", parser_name, index, tier);
            eprintln!("  Reason: {}\n", reason);
        }
        return Err(TestError::Failure(format!(
            "{} golden %mor tiers failed roundtrip test",
            failed_tiers.len()
        )));
    }
    Ok(())
}

/// Verifies golden `%gra` tier inputs round-trip for every parser backend.
#[test]
fn golden_gra_tier_roundtrip_for_every_parser() -> Result<(), TestError> {
    use talkbank_parser_tests::golden::golden_gra_tiers;

    let gra_tiers = golden_gra_tiers();
    println!("\n=== Golden %gra Tier Roundtrip Test ===");
    println!("Testing {} golden %gra tiers", gra_tiers.len());
    println!("==========================================\n");

    let mut failed_tiers = Vec::new();

    for parser in parser_suite()? {
        println!(
            "[{}] Testing {} %gra tiers...",
            "tree-sitter",
            gra_tiers.len()
        );

        for (i, tier_line) in gra_tiers.iter().enumerate() {
            let sink = ErrorCollector::new();
            let parsed = parser.parse_gra_tier_fragment(tier_line, 0, &sink);

            // Check for parsing errors
            if !sink.is_empty() {
                let error_msg = format!("Parse errors: {:?}", sink.to_vec());
                eprintln!("[{}] %gra Tier #{} FAILED: {}", "tree-sitter", i, tier_line);
                eprintln!("  Error: {}", error_msg);
                failed_tiers.push(("tree-sitter", i, tier_line, error_msg));
                continue;
            }

            let parsed = match parsed {
                ParseOutcome::Parsed(p) => p,
                ParseOutcome::Rejected => {
                    eprintln!("[{}] %gra Tier #{} FAILED: {}", "tree-sitter", i, tier_line);
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
            let serialized = gra_tier_to_content(&parsed);

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
                "[{}] All {} %gra tiers passed!",
                "tree-sitter",
                gra_tiers.len()
            );
        } else {
            println!(
                "[{}] {} %gra tiers failed",
                "tree-sitter",
                failed_tiers.len()
            );
        }
    }

    if !failed_tiers.is_empty() {
        eprintln!("\n=== Failed %gra Tiers ===");
        for (parser_name, index, tier, reason) in &failed_tiers {
            eprintln!("[{}] %gra Tier #{}: {}", parser_name, index, tier);
            eprintln!("  Reason: {}\n", reason);
        }
        return Err(TestError::Failure(format!(
            "{} golden %gra tiers failed roundtrip test",
            failed_tiers.len()
        )));
    }
    Ok(())
}
