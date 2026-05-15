//! Test module for mor in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

use talkbank_model::model::{Mor, MorFeature, MorTier, MorWord, PosCategory};

/// Verifies a simple `%mor` tier round-trips through the model serializer.
#[test]
fn mor_tier_round_trip_simple() {
    let mor_items = vec![
        Mor::new(MorWord::new(PosCategory::new("pron"), "I")),
        Mor::new(
            MorWord::new(PosCategory::new("verb"), "go").with_feature(MorFeature::new("Past")),
        ),
    ];

    let tier = MorTier::new_mor(
        mor_items,
        talkbank_model::Terminator::Period {
            span: talkbank_model::Span::DUMMY,
        },
    );
    let output = tier.to_chat();
    assert_eq!(
        output, "%mor:\tpron|I verb|go-Past .",
        "MorTier roundtrip failed: {}",
        output
    );
}

/// Verifies `%mor` post-clitic serialization round-trips correctly.
#[test]
fn mor_tier_round_trip_with_post_clitic() {
    let main = MorWord::new(PosCategory::new("pron"), "it");
    let post = MorWord::new(PosCategory::new("aux"), "be")
        .with_feature(MorFeature::new("Fin"))
        .with_feature(MorFeature::new("Ind"))
        .with_feature(MorFeature::new("Pres"))
        .with_feature(MorFeature::new("S3"));

    let mor = Mor::new(main).with_post_clitic(post);
    let tier = MorTier::new_mor(
        vec![mor],
        talkbank_model::Terminator::Period {
            span: talkbank_model::Span::DUMMY,
        },
    );
    let output = tier.to_chat();
    assert_eq!(
        output, "%mor:\tpron|it~aux|be-Fin-Ind-Pres-S3",
        "MorTier post-clitic roundtrip failed: {}",
        output
    );
}

/// Verifies `%mor` words with multiple features round-trip correctly.
#[test]
fn mor_tier_round_trip_multiple_features() {
    let word = MorWord::new(PosCategory::new("pron"), "I")
        .with_feature(MorFeature::new("Prs"))
        .with_feature(MorFeature::new("Nom"))
        .with_feature(MorFeature::new("S1"));
    let tier = MorTier::new_mor(
        vec![Mor::new(word)],
        talkbank_model::Terminator::Period {
            span: talkbank_model::Span::DUMMY,
        },
    );
    let output = tier.to_chat();
    assert_eq!(
        output, "%mor:\tpron|I-Prs-Nom-S1",
        "MorTier features roundtrip failed: {}",
        output
    );
}
