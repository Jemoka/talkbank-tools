//! Tests for this subsystem.
//!

use talkbank_model::model::{GraTier, GraTierType, GrammaticalRelation, WriteChat};

// Note: These tests are for the old string-based API
// New CST-based tests should be in the integration tests

/// Tests parse relation properties.
#[test]
fn test_parse_relation_properties() {
    let rel = GrammaticalRelation::new(1, 2, "SUBJ");
    assert_eq!(rel.index, 1);
    assert_eq!(rel.head, 2);
    assert_eq!(rel.relation.as_str(), "SUBJ");
    assert!(!rel.is_root());

    let root = GrammaticalRelation::new(2, 0, "ROOT");
    assert!(root.is_root());
}

/// Tests gra tier construction.
#[test]
fn test_gra_tier_construction() {
    let relations = vec![
        GrammaticalRelation::new(1, 2, "SUBJ"),
        GrammaticalRelation::new(2, 0, "ROOT"),
        GrammaticalRelation::new(3, 2, "OBJ"),
    ];
    let tier = GraTier::new(GraTierType::Gra, relations);
    assert_eq!(tier.relations().len(), 3);
    assert!(tier.is_gra());

    let output = tier.to_chat_string();
    assert_eq!(output, "%gra:\t1|2|SUBJ 2|0|ROOT 3|2|OBJ");
}
