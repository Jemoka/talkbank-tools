//! Audit golden words corpus and generate featured subset.
//!
//! Analyzes the 769 golden words to:
//! - Classify by feature signatures
//! - Identify plain text vs featured words
//! - Detect redundancy (multiple words with identical signatures)
//! - Generate a curated featured subset (~150-200 words)
//!
//! ## Usage
//!
//! ```bash
//! cargo run -p talkbank-parser-tests --bin audit_golden_words
//! ```
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use std::collections::HashMap;
use std::fs;
use talkbank_model::ErrorCollector;
use talkbank_model::ParseOutcome;
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::feature_signature::WordFeatureSignature;
use talkbank_parser_tests::golden::golden_words;
use talkbank_parser_tests::test_error::TestError;

/// Entry point for this binary target.
fn main() -> Result<(), TestError> {
    println!("=== Golden Words Feature Audit ===\n");

    let parser = TreeSitterParser::new().map_err(|err| TestError::ParserInit(err.to_string()))?;
    let words = golden_words();

    println!("Total golden words: {}\n", words.len());

    // Parse each word and compute signature
    let mut signatures: HashMap<WordFeatureSignature, Vec<&str>> = HashMap::new();
    let mut parse_failures = Vec::new();

    for word in &words {
        let errors = ErrorCollector::new();
        let parsed = parser.parse_word_fragment(word, 0, &errors);

        if let ParseOutcome::Parsed(parsed_word) = parsed {
            let sig = WordFeatureSignature::from_word(&parsed_word);
            signatures.entry(sig).or_default().push(word);
        } else {
            parse_failures.push(*word);
        }
    }

    // Report statistics
    println!("Parse results:");
    println!(
        "  Parsed successfully: {}",
        words.len() - parse_failures.len()
    );
    println!("  Parse failures: {}", parse_failures.len());
    if !parse_failures.is_empty() {
        println!("\nFailed words:");
        for word in &parse_failures {
            println!("  - {}", word);
        }
    }
    println!();

    // Signature statistics
    println!("Signature statistics:");
    println!("  Unique signatures: {}", signatures.len());
    println!(
        "  Average words per signature: {:.1}",
        words.len() as f64 / signatures.len() as f64
    );
    println!();

    // Plain text analysis
    let plain_text_words: Vec<_> = signatures
        .iter()
        .filter(|(sig, _)| sig.is_plain_text())
        .flat_map(|(_, words)| words.iter())
        .collect();

    println!(
        "Plain text words: {} ({:.1}%)",
        plain_text_words.len(),
        100.0 * plain_text_words.len() as f64 / words.len() as f64
    );
    println!();

    // Distribution analysis
    let mut distribution: HashMap<usize, usize> = HashMap::new();
    for words_list in signatures.values() {
        *distribution.entry(words_list.len()).or_default() += 1;
    }

    println!("Words per signature distribution:");
    let mut dist_vec: Vec<_> = distribution.iter().collect();
    dist_vec.sort_by_key(|(count, _)| *count);
    for (words_per_sig, num_signatures) in dist_vec {
        println!(
            "  {} signatures with {} word{}",
            num_signatures,
            words_per_sig,
            if *words_per_sig == 1 { "" } else { "s" }
        );
    }
    println!();

    // Top signatures by count
    let mut sig_counts: Vec<_> = signatures.iter().collect();
    sig_counts.sort_by_key(|(_, words)| std::cmp::Reverse(words.len()));

    println!("Top 10 signatures by word count:");
    for (sig, words) in sig_counts.iter().take(10) {
        println!(
            "\n  {} words with signature: {}",
            words.len(),
            sig.describe()
        );
        println!(
            "  Examples: {}",
            words
                .iter()
                .take(3)
                .map(|w| format!("\"{}\"", w))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    println!();

    // Generate TWO lists: featured (medium) and minimal (fast)

    // === FEATURED LIST (84 words) ===
    let mut featured_words = Vec::new();
    let mut featured_plain_text_count = 0;
    let mut featured_count_by_sig: HashMap<String, usize> = HashMap::new();

    // Strategy for featured:
    // - For signatures with 1 word: include it
    // - For signatures with 2-5 words: include 1-2 representatives
    // - For signatures with 6+ words: include 2-3 representatives
    // - For plain text: include exactly 5 representatives

    for (sig, words) in &signatures {
        let desc = sig.describe();
        let count_to_include = if sig.is_plain_text() {
            if featured_plain_text_count < 5 {
                let available = 5 - featured_plain_text_count;
                available.min(words.len())
            } else {
                0
            }
        } else {
            match words.len() {
                1 => 1,
                2..=5 => 2.min(words.len()),
                _ => 3.min(words.len()),
            }
        };

        for &word in words.iter().take(count_to_include) {
            featured_words.push(word);
            if sig.is_plain_text() {
                featured_plain_text_count += 1;
            }
        }

        if count_to_include > 0 {
            *featured_count_by_sig.entry(desc).or_default() += count_to_include;
        }
    }

    // === MINIMAL LIST (30 words) ===
    let mut minimal_words = Vec::new();
    let mut minimal_plain_text_count = 0;
    let mut minimal_count_by_sig: HashMap<String, usize> = HashMap::new();

    // Strategy for minimal:
    // - Exactly ONE representative per signature
    // - Plain text: exactly 3 representatives (not 557!)

    for (sig, words) in &signatures {
        let desc = sig.describe();
        let count_to_include = if sig.is_plain_text() {
            if minimal_plain_text_count < 3 { 1 } else { 0 }
        } else {
            1 // ONE per signature
        };

        if count_to_include > 0 {
            minimal_words.push(words[0]);
            if sig.is_plain_text() {
                minimal_plain_text_count += 1;
            }
            *minimal_count_by_sig.entry(desc).or_default() += 1;
        }
    }

    // Sort both lists for stability
    featured_words.sort();
    minimal_words.sort();

    // === Write FEATURED list ===
    println!("=== Featured Word List (Medium) ===");
    println!("Total featured words: {}", featured_words.len());
    println!(
        "  Plain text representatives: {}",
        featured_plain_text_count
    );
    println!(
        "  Featured (non-plain): {}",
        featured_words.len() - featured_plain_text_count
    );
    println!();

    let featured_content = format!(
        "# Featured Golden Words\n\
         # Generated by audit_golden_words binary\n\
         # Total: {} words (from {} unique signatures)\n\
         # Plain text: {} words\n\
         # Featured: {} words\n\
         #\n\
         # This is a curated subset of golden_words.txt with:\n\
         # - 1-3 representatives per feature signature\n\
         # - 5 plain text representatives\n\
         #\n\
         # Use this for cross-parser equivalence tests.\n\n{}\n",
        featured_words.len(),
        signatures.len(),
        featured_plain_text_count,
        featured_words.len() - featured_plain_text_count,
        featured_words.join("\n")
    );

    fs::write("golden_words_featured.txt", featured_content)?;

    println!(
        "✓ Wrote {} to golden_words_featured.txt",
        featured_words.len()
    );

    // === Write MINIMAL list ===
    println!("\n=== Minimal Word List (Fast) ===");
    println!("Total minimal words: {}", minimal_words.len());
    println!("  Plain text representatives: {}", minimal_plain_text_count);
    println!(
        "  Featured (non-plain): {}",
        minimal_words.len() - minimal_plain_text_count
    );
    println!();

    let minimal_content = format!(
        "# Minimal Golden Words - Fast Core Tests\n\
         # Generated by audit_golden_words binary\n\
         # Total: {} words (ONE per unique signature)\n\
         # Plain text: {} words\n\
         # Featured: {} words\n\
         #\n\
         # This is the MINIMAL test set with exactly one representative per feature.\n\
         # Use this for:\n\
         # - Fast CI tests (runs in <1 second)\n\
         # - Core feature regression detection\n\
         # - TDD feedback loop\n\
         #\n\
         # For comprehensive testing, use golden_words_featured.txt (84 words)\n\
         # For full corpus validation, use golden_words.txt (768 words)\n\n{}\n",
        minimal_words.len(),
        minimal_plain_text_count,
        minimal_words.len() - minimal_plain_text_count,
        minimal_words.join("\n")
    );

    fs::write("golden_words_minimal.txt", minimal_content)?;

    println!(
        "✓ Wrote {} to golden_words_minimal.txt",
        minimal_words.len()
    );
    println!();

    // Summary by category
    println!("Featured words by signature category:");
    let mut categories: Vec<_> = featured_count_by_sig.iter().collect();
    categories.sort_by_key(|(desc, _)| *desc);
    for (desc, count) in categories.iter().take(20) {
        println!(
            "  {}: {} word{}",
            desc,
            count,
            if **count == 1 { "" } else { "s" }
        );
    }

    if categories.len() > 20 {
        println!("  ... and {} more signatures", categories.len() - 20);
    }

    println!("\n=== Summary ===");
    println!("Generated three test corpora:");
    println!("  golden_words.txt (768 words)         - Full corpus (reference)");
    println!("  golden_words_featured.txt (84 words)  - Medium testing");
    println!(
        "  golden_words_minimal.txt ({} words)   - Fast testing (<1sec)",
        minimal_words.len()
    );
    println!(
        "
Recommendation: Use minimal for CI, featured for pre-commit, full for weekly validation"
    );

    Ok(())
}
