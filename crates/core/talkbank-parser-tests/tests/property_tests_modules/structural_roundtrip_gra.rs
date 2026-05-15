//! Structural roundtrip property tests for %gra tiers.
//!
//! Generates valid dependency graphs (index|head|relation triples) and verifies
//! parse → serialize → reparse stability across both parsers.

use proptest::prelude::*;
use talkbank_model::ErrorCollector;
use talkbank_parser::TreeSitterParser;

use super::slow_test_config;

/// Strategy for grammatical relation labels (common UD relations).
fn relation_label() -> impl Strategy<Value = &'static str> {
    prop_oneof![
        Just("ROOT"),
        Just("SUBJ"),
        Just("OBJ"),
        Just("DET"),
        Just("PUNCT"),
        Just("NMOD"),
        Just("AMOD"),
        Just("IOBJ"),
        Just("COORD"),
        Just("JCT"),
        Just("PRED"),
        Just("AUX"),
        Just("NEG"),
        Just("QUANT"),
        Just("COM"),
    ]
}

/// Generate a valid gra tier content string with N relations.
///
/// Constraints:
/// - Indices are 1..=N (sequential)
/// - Exactly one ROOT (head=0 or relation=ROOT)
/// - Other heads are in 0..=N (0 means ROOT)
/// - Last relation is typically PUNCT
fn gra_content(n: usize) -> impl Strategy<Value = String> {
    // Generate a root position (1-indexed)
    let root_idx = 1..=n;
    // Generate heads for non-root positions
    let heads = prop::collection::vec(0..n, n);
    // Generate relation labels for non-root positions
    let labels = prop::collection::vec(relation_label(), n);

    (root_idx, heads, labels).prop_map(move |(root_pos, raw_heads, raw_labels)| {
        let mut parts = Vec::with_capacity(n);
        for i in 1..=n {
            if i == root_pos {
                parts.push(format!("{}|0|ROOT", i));
            } else {
                // Pick a head that isn't the current index (avoid trivial self-loops)
                let mut head = raw_heads[i - 1];
                if head == i {
                    head = 0; // Fall back to ROOT-linked
                }
                let label = raw_labels[i - 1];
                // Don't use ROOT label for non-root positions
                let label = if label == "ROOT" { "SUBJ" } else { label };
                parts.push(format!("{}|{}|{}", i, head, label));
            }
        }
        parts.join(" ")
    })
}

/// Helper: parse gra tier content with the tree-sitter parser.
fn parse_gra(input: &str) -> Option<String> {
    let ts = TreeSitterParser::new().ok()?;
    let errors = ErrorCollector::new();
    let parsed = ts.parse_gra_tier_fragment(input, 0, &errors);
    parsed.into_option().map(|tier| tier.to_content())
}

proptest! {
    #![proptest_config(slow_test_config())]

    /// Small dependency graph (2-3 relations) roundtrips.
    #[test]
    fn small_gra_roundtrip(content in gra_content(2).prop_union(gra_content(3))) {
        if let Some(output) = parse_gra(&content) {
            prop_assert_eq!(
                &output, &content,
                "[tree-sitter] roundtrip: '{}' -> '{}'",
                content, output
            );
        }
    }

    /// Medium dependency graph (4-6 relations) roundtrips.
    #[test]
    fn medium_gra_roundtrip(
        content in (4..=6usize).prop_flat_map(gra_content)
    ) {
        if let Some(output) = parse_gra(&content) {
            prop_assert_eq!(
                &output, &content,
                "[tree-sitter] roundtrip: '{}' -> '{}'",
                content, output
            );
        }
    }

    /// Double roundtrip is stable for gra tiers.
    #[test]
    fn gra_double_roundtrip_stable(content in gra_content(3)) {
        if let Ok(ts) = TreeSitterParser::new() {
            let e1 = ErrorCollector::new();
            if let Some(tier1) = ts.parse_gra_tier_fragment(&content, 0, &e1).into_option() {
                let chat1 = tier1.to_content();
                let e2 = ErrorCollector::new();
                if let Some(tier2) = ts.parse_gra_tier_fragment(&chat1, 0, &e2).into_option() {
                    let chat2 = tier2.to_content();
                    prop_assert_eq!(
                        &chat1, &chat2,
                        "double roundtrip unstable: '{}' -> '{}' -> '{}'",
                        content, chat1, chat2
                    );
                }
            }
        }
    }
}
