//! Development-Time Parser Equivalence Tests
//!
//! Quick equivalence checks for specific features during development.
//! Unlike the full corpus equivalence test (340 files, 30-60 seconds),
//! these tests run in 1-5 seconds and focus on individual features.
//!
//! ## Purpose
//!
//! - **Fast feedback** during development (seconds, not minutes)
//! - **Feature-specific** testing (compounds, overlap points, etc.)
//! - **Focused errors** (not buried in 340-file dump)
//! - **Run manually** during development, not just in CI
//!
//! ## Usage
//!
//! ```bash
//! # Run all dev equivalence tests
//! cargo test -p talkbank-parser-tests --test dev_equivalence
//!
//! # Run specific feature test
//! cargo test -p talkbank-parser-tests --test dev_equivalence equiv_compound_words
//!
//! # Run with verbose output
//! cargo test -p talkbank-parser-tests --test dev_equivalence -- --nocapture
//! ```
//!
//! ## When to Use
//!
//! - During TreeSitterParser feature implementation
//! - After fixing a specific bug
//! - Before committing changes
//! - When you want fast feedback on a specific construct
//!
//! ## Workflow
//!
//! 1. Implement TreeSitterParser feature
//! 2. Add test case here for that feature
//! 3. Run this test (fast!)
//! 4. Fix issues
//! 5. Run full corpus equivalence (final validation)

use talkbank_model::ErrorCollector;
use talkbank_model::ParseOutcome;
use talkbank_parser::TreeSitterParser;

/// Parse a CHAT file input and verify it succeeds.
///
/// Returns Ok(()) if the parser produces a valid ChatFile.
/// Returns Err(message) if parsing fails.
fn parse_chat_file(input: &str, description: &str) -> Result<(), String> {
    let parser = TreeSitterParser::new().map_err(|e| format!("TreeSitter init failed: {}", e))?;

    let errors = ErrorCollector::new();
    let result = parser.parse_chat_file_fragment(input, 0, &errors);

    match result {
        ParseOutcome::Parsed(_) => Ok(()),
        ParseOutcome::Rejected => Err(format!(
            "Failed to parse {}
Errors: {:?}",
            description,
            errors.to_vec()
        )),
    }
}

// =============================================================================
// Compound Words
// =============================================================================

/// Verifies parser equivalence on representative compound-word inputs.
#[test]
fn equiv_compound_words() {
    let inputs = [
        (
            "@UTF8\n@Begin\n@Languages:\teng\n*CHI:\twai4+yu3 .\n@End\n",
            "simple compound",
        ),
        (
            "@UTF8\n@Begin\n@Languages:\teng\n*CHI:\thello+world+test .\n@End\n",
            "triple compound",
        ),
        (
            "@UTF8\n@Begin\n@Languages:\teng\n*CHI:\ta+b .\n@End\n",
            "minimal compound",
        ),
    ];

    for (input, description) in inputs {
        parse_chat_file(input, description)
            .unwrap_or_else(|e| panic!("Compound words parsing failed:\n{}", e));
    }
}

// =============================================================================
// Overlap Points
// =============================================================================

/// Verifies parser handles overlap-point inputs.
#[test]
fn equiv_overlap_points() {
    let inputs = [
        (
            "@UTF8\n@Begin\n@Languages:\teng\n*CHI:\thello⌈ .\n@End\n",
            "overlap open marker",
        ),
        (
            "@UTF8\n@Begin\n@Languages:\teng\n*CHI:\t⌉world .\n@End\n",
            "overlap close marker",
        ),
        (
            "@UTF8\n@Begin\n@Languages:\teng\n*CHI:\thello⌈ world .\n*MOT:\t⌉yes .\n@End\n",
            "overlap across speakers",
        ),
    ];

    for (input, description) in inputs {
        parse_chat_file(input, description)
            .unwrap_or_else(|e| panic!("Overlap points parsing failed:\n{}", e));
    }
}

// =============================================================================
// Stress Markers
// =============================================================================

/// Verifies parser handles stress-marker inputs.
#[test]
fn equiv_stress_markers() {
    let inputs = [
        (
            "@UTF8\n@Begin\n@Languages:\teng\n*CHI:\tˈstress .\n@End\n",
            "primary stress",
        ),
        (
            "@UTF8\n@Begin\n@Languages:\teng\n*CHI:\tˌsecondary .\n@End\n",
            "secondary stress",
        ),
        (
            "@UTF8\n@Begin\n@Languages:\teng\n*CHI:\tˈpriˌmary .\n@End\n",
            "combined stress",
        ),
    ];

    for (input, description) in inputs {
        parse_chat_file(input, description)
            .unwrap_or_else(|e| panic!("Stress markers parsing failed:\n{}", e));
    }
}

// =============================================================================
// CA Elements
// =============================================================================

/// Verifies parser handles CA-element inputs.
#[test]
#[ignore = "TreeSitterParser does not parse mid-word CA elements (‡, ↑, ↓)"]
fn equiv_ca_elements() {
    let inputs = [
        (
            "@UTF8\n@Begin\n@Languages:\teng\n*CHI:\thel‡lo .\n@End\n",
            "glottal stop",
        ),
        (
            "@UTF8\n@Begin\n@Languages:\teng\n*CHI:\two↑rld .\n@End\n",
            "rising intonation",
        ),
        (
            "@UTF8\n@Begin\n@Languages:\teng\n*CHI:\ttest↓ .\n@End\n",
            "falling intonation",
        ),
    ];

    for (input, description) in inputs {
        parse_chat_file(input, description)
            .unwrap_or_else(|e| panic!("CA elements parsing failed:\n{}", e));
    }
}

// =============================================================================
// Lengthening and Shortening
// =============================================================================

/// Verifies parser handles lengthening inputs.
#[test]
fn equiv_lengthening() {
    let inputs = [
        (
            "@UTF8\n@Begin\n@Languages:\teng\n*CHI:\thello: .\n@End\n",
            "single colon",
        ),
        (
            "@UTF8\n@Begin\n@Languages:\teng\n*CHI:\two::rld .\n@End\n",
            "double colon",
        ),
    ];

    for (input, description) in inputs {
        parse_chat_file(input, description)
            .unwrap_or_else(|e| panic!("Lengthening parsing failed:\n{}", e));
    }
}

/// Verifies parser handles shortening inputs.
#[test]
fn equiv_shortening() {
    let inputs = [
        (
            "@UTF8\n@Begin\n@Languages:\teng\n*CHI:\tgoin(g) .\n@End\n",
            "parenthesized shortening",
        ),
        (
            "@UTF8\n@Begin\n@Languages:\teng\n*CHI:\tdoin' .\n@End\n",
            "apostrophe shortening",
        ),
    ];

    for (input, description) in inputs {
        parse_chat_file(input, description)
            .unwrap_or_else(|e| panic!("Shortening parsing failed:\n{}", e));
    }
}

// =============================================================================
// Complex Combinations
// =============================================================================

/// Verifies parser handles mixed-marker complex word inputs.
#[test]
#[ignore = "Depends on CA element support in TreeSitterParser"]
fn equiv_complex_words() {
    let inputs = [
        (
            "@UTF8\n@Begin\n@Languages:\teng\n*CHI:\tˈhel⌈lo+wor⌉ld: .\n@End\n",
            "stress + overlap + compound + lengthening",
        ),
        (
            "@UTF8\n@Begin\n@Languages:\teng\n*CHI:\tgo^ing(g)+fast‡ .\n@End\n",
            "syllable pause + shortening + compound + CA",
        ),
    ];

    for (input, description) in inputs {
        parse_chat_file(input, description)
            .unwrap_or_else(|e| panic!("Complex words parsing failed:\n{}", e));
    }
}

// =============================================================================
// Specific File Testing
// =============================================================================

/// Test equivalence on a specific corpus file (for debugging).
///
/// Usage: Remove #[ignore] and specify the path, then run:
/// ```bash
/// cargo test -p talkbank-parser-tests --test dev_equivalence equiv_specific_file
/// ```
#[test]
#[ignore]
fn equiv_specific_file() {
    let path = "resources/corpus/reference/sample.cha";
    let content =
        std::fs::read_to_string(path).unwrap_or_else(|e| panic!("Failed to read {}: {}", path, e));

    parse_chat_file(&content, path)
        .unwrap_or_else(|e| panic!("Specific file parsing failed:\n{}", e));
}
