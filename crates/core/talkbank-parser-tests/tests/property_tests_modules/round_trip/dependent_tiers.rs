//! Test module for dependent tiers in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

use talkbank_model::model::{
    ActTier, AddTier, BulletContent, ComTier, ExpTier, GpxTier, GraTier, GrammaticalRelation,
    PhoItem, PhoTier, PhoTierType, PhoWord, WriteChat,
};

/// Verifies a simple `%gra` tier round-trips.
#[test]
fn gra_tier_round_trip_simple() {
    let relations = vec![
        GrammaticalRelation::new(1, 2, "SUBJ"),
        GrammaticalRelation::new(2, 0, "ROOT"),
    ];

    let tier = GraTier::new_gra(relations);
    let output = tier.to_chat();
    assert_eq!(
        output, "%gra:\t1|2|SUBJ 2|0|ROOT",
        "GraTier roundtrip failed: {}",
        output
    );
}

/// Verifies `%pho` tier round-trip serialization.
#[test]
fn pho_tier_round_trip_pho() {
    let tier = PhoTier::new(
        PhoTierType::Pho,
        vec![
            PhoItem::Word(PhoWord("wʌn".into())),
            PhoItem::Word(PhoWord("tu".into())),
            PhoItem::Word(PhoWord("θɹi".into())),
        ],
    );
    let output = tier.to_chat_string();
    assert_eq!(
        output, "%pho:\twʌn tu θɹi",
        "PhoTier (%pho) roundtrip failed: {}",
        output
    );
}

/// Verifies `%mod` tier round-trip serialization.
#[test]
fn pho_tier_round_trip_mod() {
    let tier = PhoTier::new(
        PhoTierType::Mod,
        vec![
            PhoItem::Word(PhoWord("wʌn".into())),
            PhoItem::Word(PhoWord("tu".into())),
            PhoItem::Word(PhoWord("θri".into())),
        ],
    );
    let output = tier.to_chat_string();
    assert_eq!(
        output, "%mod:\twʌn tu θri",
        "PhoTier (%mod) roundtrip failed: {}",
        output
    );
}

/// Verifies `%act` tier round-trip serialization.
#[test]
fn act_tier_round_trip() {
    let tier = ActTier::new(BulletContent::from_text("<1w-2w> holds object out to Amy"));
    let output = tier.to_chat();
    assert_eq!(
        output, "%act:\t<1w-2w> holds object out to Amy",
        "ActTier roundtrip failed: {}",
        output
    );
}

/// Verifies `%com` tier round-trip serialization.
#[test]
fn com_tier_round_trip() {
    let tier = ComTier::new(BulletContent::from_text("spoken softly"));
    let output = tier.to_chat();
    assert_eq!(
        output, "%com:\tspoken softly",
        "ComTier roundtrip failed: {}",
        output
    );
}

/// Verifies `%exp` tier round-trip serialization.
#[test]
fn exp_tier_round_trip() {
    let tier = ExpTier::new(BulletContent::from_text("dancing noises"));
    let output = tier.to_chat();
    assert_eq!(
        output, "%exp:\tdancing noises",
        "ExpTier roundtrip failed: {}",
        output
    );
}

/// Adds tier round trip.
#[test]
fn add_tier_round_trip() {
    let tier = AddTier::new(BulletContent::from_text("MOT"));
    let output = tier.to_chat();
    assert_eq!(output, "%add:\tMOT", "AddTier roundtrip failed: {}", output);
}

/// Verifies `%gpx` tier round-trip serialization.
#[test]
fn gpx_tier_round_trip() {
    let tier = GpxTier::new(BulletContent::from_text("points at toy"));
    let output = tier.to_chat();
    assert_eq!(
        output, "%gpx:\tpoints at toy",
        "GpxTier roundtrip failed: {}",
        output
    );
}
