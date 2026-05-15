//! Tests for this subsystem.
//!

use talkbank_model::model::{
    AddTier, ComTier, ExpTier, GpxTier, IntTier, SitTier, SpaTier, WriteChat,
};

/// Tests com tier construction.
#[test]
fn test_com_tier_construction() {
    let tier = ComTier::from_text("This is a comment");
    assert!(!tier.content.is_empty());
    assert_eq!(tier.to_chat_string(), "%com:\tThis is a comment");
}

/// Tests exp tier construction.
#[test]
fn test_exp_tier_construction() {
    let tier = ExpTier::from_text("Explanation text");
    assert!(!tier.content.is_empty());
    assert_eq!(tier.to_chat_string(), "%exp:\tExplanation text");
}

/// Tests add tier construction.
#[test]
fn test_add_tier_construction() {
    let tier = AddTier::from_text("MOT");
    assert!(!tier.content.is_empty());
    assert_eq!(tier.to_chat_string(), "%add:\tMOT");
}

/// Tests spa tier construction.
#[test]
fn test_spa_tier_construction() {
    let tier = SpaTier::from_text("$DIS");
    assert!(!tier.content.is_empty());
    assert_eq!(tier.to_chat_string(), "%spa:\t$DIS");
}

/// Tests sit tier construction.
#[test]
fn test_sit_tier_construction() {
    let tier = SitTier::from_text("at home");
    assert!(!tier.content.is_empty());
    assert_eq!(tier.to_chat_string(), "%sit:\tat home");
}

/// Tests gpx tier construction.
#[test]
fn test_gpx_tier_construction() {
    let tier = GpxTier::from_text("1.5");
    assert!(!tier.content.is_empty());
    assert_eq!(tier.to_chat_string(), "%gpx:\t1.5");
}

/// Tests int tier construction.
#[test]
fn test_int_tier_construction() {
    let tier = IntTier::from_text("rising");
    assert!(!tier.content.is_empty());
    assert_eq!(tier.to_chat_string(), "%int:\trising");
}
