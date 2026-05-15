//! `%mor`↔`%gra` alignment behavior tests.
//!
//! This suite stresses chunk-count mismatch handling, placeholder pairing, and
//! diagnostic rendering for `%mor` post-clitic expansion cases.

use super::align_mor_to_gra;
use crate::model::{GraTier, GrammaticalRelation, Mor, MorTier, MorWord, PosCategory};

/// Aligns equal-size `%mor` and `%gra` tiers without errors, including the
/// trailing terminator / `PUNCT` relation.
#[test]
fn test_gra_alignment_perfect_match() {
    // Create %mor tier with 3 lexical chunks + terminator
    let mor = MorTier::new_mor(
        vec![
            Mor::new(MorWord::new(PosCategory::new("pron"), "I")),
            Mor::new(MorWord::new(PosCategory::new("verb"), "go")),
            Mor::new(MorWord::new(PosCategory::new("noun"), "home")),
        ],
        crate::Terminator::Period {
            span: crate::Span::DUMMY,
        },
    );

    // Create %gra tier with 4 relations: 3 lexical + trailing PUNCT
    let gra = GraTier::new_gra(vec![
        GrammaticalRelation::new(1, 2, "SUBJ"),
        GrammaticalRelation::new(2, 0, "ROOT"),
        GrammaticalRelation::new(3, 2, "OBL"),
        GrammaticalRelation::new(4, 2, "PUNCT"),
    ]);

    let alignment = align_mor_to_gra(&mor, &gra);

    assert_eq!(alignment.pairs.len(), 4);
    assert!(alignment.is_error_free());
    assert_eq!(mor.count_chunks(), 4);
}

/// Handles post-clitic `%mor` chunks when `%gra` supplies matching relations,
/// including the terminator / `PUNCT` slot.
#[test]
fn test_gra_alignment_with_post_clitic() {
    // Create %mor tier with 1 item that has a post-clitic
    // (3 chunks total including terminator)
    let mor_item = Mor::new(MorWord::new(PosCategory::new("pron"), "it"))
        .with_post_clitic(MorWord::new(PosCategory::new("aux"), "be"));
    let mor = MorTier::new_mor(
        vec![mor_item],
        crate::Terminator::Period {
            span: crate::Span::DUMMY,
        },
    );

    // Create %gra tier with 3 relations (main, clitic, trailing PUNCT)
    let gra = GraTier::new_gra(vec![
        GrammaticalRelation::new(1, 2, "EXPL"),
        GrammaticalRelation::new(2, 0, "ROOT"),
        GrammaticalRelation::new(3, 2, "PUNCT"),
    ]);

    let alignment = align_mor_to_gra(&mor, &gra);

    assert_eq!(mor.count_chunks(), 3); // main + 1 post-clitic + terminator
    assert_eq!(alignment.pairs.len(), 3);
    assert!(alignment.is_error_free());
}

/// Emits placeholder `%gra` entries when `%mor` has extra chunks.
#[test]
fn test_gra_alignment_mor_longer() {
    // %mor has 4 chunks (3 lexical + terminator), %gra has only 1
    let mor = MorTier::new_mor(
        vec![
            Mor::new(MorWord::new(PosCategory::new("verb"), "a")),
            Mor::new(MorWord::new(PosCategory::new("verb"), "b")),
            Mor::new(MorWord::new(PosCategory::new("verb"), "c")),
        ],
        crate::Terminator::Period {
            span: crate::Span::DUMMY,
        },
    );

    let gra = GraTier::new_gra(vec![GrammaticalRelation::new(1, 0, "ROOT")]);

    let alignment = align_mor_to_gra(&mor, &gra);

    assert_eq!(alignment.pairs.len(), 4); // 1 valid + 3 placeholders
    assert!(!alignment.is_error_free());
    assert_eq!(alignment.errors.len(), 1);
    // E720 (MorGraCountMismatch) reflects that the tier cardinalities disagree.
    // E712/E713 are reserved for per-relation index validation.
    assert_eq!(alignment.errors[0].code.as_str(), "E720");

    // First pair valid, next three are placeholders
    assert!(alignment.pairs[0].is_complete());
    assert!(alignment.pairs[1].is_placeholder());
    assert!(alignment.pairs[2].is_placeholder());
    assert!(alignment.pairs[3].is_placeholder());
}

/// Emits placeholder `%mor` entries when `%gra` has extra relations.
#[test]
fn test_gra_alignment_gra_longer() {
    // %mor has 2 chunks (verb + terminator), %gra has 3 relations
    let mor = MorTier::new_mor(
        vec![Mor::new(MorWord::new(PosCategory::new("verb"), "go"))],
        crate::Terminator::Period {
            span: crate::Span::DUMMY,
        },
    );

    let gra = GraTier::new_gra(vec![
        GrammaticalRelation::new(1, 0, "ROOT"),
        GrammaticalRelation::new(2, 1, "PUNCT"),
        GrammaticalRelation::new(3, 1, "OBJ"),
    ]);

    let alignment = align_mor_to_gra(&mor, &gra);

    assert_eq!(alignment.pairs.len(), 3); // 2 valid + 1 placeholder
    assert!(!alignment.is_error_free());
    assert_eq!(alignment.errors.len(), 1);
    // E720 (MorGraCountMismatch) reflects that the tier cardinalities disagree.
    // E712/E713 are reserved for per-relation index validation.
    assert_eq!(alignment.errors[0].code.as_str(), "E720");

    // First two pairs valid, final pair is a placeholder
    assert!(alignment.pairs[0].is_complete());
    assert!(alignment.pairs[1].is_complete());
    assert!(alignment.pairs[2].is_placeholder());
}

/// Treats a terminator-only `%mor` tier as a clean one-pair alignment when
/// `%gra` supplies the matching trailing `PUNCT` relation.
#[test]
fn test_gra_alignment_empty() {
    let mor = MorTier::new_mor(
        vec![],
        crate::Terminator::Period {
            span: crate::Span::DUMMY,
        },
    );
    let gra = GraTier::new_gra(vec![GrammaticalRelation::new(1, 0, "PUNCT")]);

    let alignment = align_mor_to_gra(&mor, &gra);

    assert_eq!(alignment.pairs.len(), 1);
    assert!(alignment.is_error_free());
}

/// Includes a column-style mismatch diagnostic for `%mor`-longer cases.
#[test]
fn test_gra_mismatch_shows_column_diagnostic() {
    // MWT case: "I'll" → pron|I~aux|will (2 chunks) + 2 more words + terminator = 5 chunks
    // But %gra only has 3 relations
    let mor_ill = Mor::new(MorWord::new(PosCategory::new("pron"), "I"))
        .with_post_clitic(MorWord::new(PosCategory::new("aux"), "will"));
    let mor = MorTier::new_mor(
        vec![
            mor_ill,
            Mor::new(MorWord::new(PosCategory::new("verb"), "give")),
            Mor::new(MorWord::new(PosCategory::new("pron"), "you")),
        ],
        crate::model::content::Terminator::Period {
            span: crate::Span::DUMMY,
        },
    );

    // Only 3 %gra relations (but 4 mor chunks + terminator = 5 total)
    let gra = GraTier::new_gra(vec![
        GrammaticalRelation::new(1, 2, "NSUBJ"),
        GrammaticalRelation::new(2, 0, "ROOT"),
        GrammaticalRelation::new(3, 2, "IOBJ"),
    ]);

    let alignment = align_mor_to_gra(&mor, &gra);
    assert!(!alignment.is_error_free());

    let msg = &alignment.errors[0].message;
    // Should contain column-by-column layout with headers
    assert!(
        msg.contains("%mor chunks"),
        "should have %mor header: {msg}"
    );
    assert!(
        msg.contains("%gra relations"),
        "should have %gra header: {msg}"
    );
    // Should show the actual items
    assert!(msg.contains("pron|I"), "should show pron|I chunk: {msg}");
    assert!(
        msg.contains("aux|will"),
        "should show aux|will clitic: {msg}"
    );
    assert!(msg.contains("1|2|NSUBJ"), "should show gra relation: {msg}");
    // Should show the overflow marker for missing gra entries
    assert!(msg.contains("⊖"), "should mark missing gra entries: {msg}");
}

/// Includes a column-style mismatch diagnostic for `%gra`-longer cases.
#[test]
fn test_gra_mismatch_gra_longer_shows_diagnostic() {
    let mor = MorTier::new_mor(
        vec![Mor::new(MorWord::new(PosCategory::new("verb"), "go"))],
        crate::Terminator::Period {
            span: crate::Span::DUMMY,
        },
    );

    let gra = GraTier::new_gra(vec![
        GrammaticalRelation::new(1, 0, "ROOT"),
        GrammaticalRelation::new(2, 1, "PUNCT"),
        GrammaticalRelation::new(3, 1, "OBJ"),
    ]);

    let alignment = align_mor_to_gra(&mor, &gra);
    assert!(!alignment.is_error_free());

    let msg = &alignment.errors[0].message;
    assert!(msg.contains("verb|go"), "should show mor chunk: {msg}");
    assert!(msg.contains("3|1|OBJ"), "should show extra gra: {msg}");
    assert!(msg.contains("⊕"), "should mark extra gra entries: {msg}");
}
