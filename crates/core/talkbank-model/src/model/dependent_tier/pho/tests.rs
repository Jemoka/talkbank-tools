//! Regression tests for `%pho`/`%mod` model primitives.
//!
//! These tests focus on serialization contracts and basic alignment assumptions
//! used by higher-level tier validators.
//! They intentionally avoid parser internals and operate on direct model
//! construction so failures isolate model-level regressions quickly.

use super::*;

/// `PhoWord` preserves IPA text through construction and accessors.
///
/// This is a baseline guard for lossless phonology token handling.
#[test]
fn test_pho_word_from_str() {
    let word = PhoWord::from("həˈloʊ");
    assert_eq!(word.0, "həˈloʊ");
    assert_eq!(word.as_str(), "həˈloʊ");
}

/// Tier-type enum values serialize to canonical `%pho`/`%mod` prefixes.
///
/// Prefix correctness is required for full-tier `WriteChat` roundtrips.
#[test]
fn test_pho_tier_type_write_chat() {
    let pho = PhoTierType::Pho;
    let mod_type = PhoTierType::Mod;

    assert_eq!(pho.to_chat_string(), "%pho");
    assert_eq!(mod_type.to_chat_string(), "%mod");
}

/// Plain phonology words serialize without wrapper punctuation.
///
/// This protects the `PhoItem::Word` formatting branch.
#[test]
fn test_pho_item_word_write_chat() {
    let item = PhoItem::Word(PhoWord::new("həˈloʊ"));
    assert_eq!(item.to_chat_string(), "həˈloʊ");
}

/// Grouped phonology words serialize with `‹...›` delimiters.
///
/// This ensures grouped token boundaries are preserved in output.
#[test]
fn test_pho_item_group_write_chat() {
    let item = PhoItem::Group(PhoGroupWords::new(vec![
        PhoWord::new("wɑ"),
        PhoWord::new("nə"),
    ]));
    assert_eq!(item.to_chat_string(), "‹wɑ nə›");
}

/// `new_pho` assigns tier type and preserves item payloads.
///
/// The test covers constructor wiring used by parser output.
#[test]
fn test_pho_tier_new() {
    let tier = PhoTier::new_pho(vec![PhoItem::Word(PhoWord::new("həˈloʊ"))]);

    assert_eq!(tier.tier_type, PhoTierType::Pho);
    assert_eq!(tier.items.len(), 1);
    assert!(tier.is_pho());
    assert!(!tier.is_mod());
}

/// Full `%pho` tier serialization includes prefix and token spacing.
///
/// This guards the common writer path used in roundtrip tests.
#[test]
fn test_pho_tier_write_chat() {
    let tier = PhoTier::new_pho(vec![
        PhoItem::Word(PhoWord::new("həˈloʊ")),
        PhoItem::Word(PhoWord::new("wɜrld")),
    ]);

    assert_eq!(tier.to_chat_string(), "%pho:\thəˈloʊ wɜrld");
}

/// Typical `%pho`/`%mod` pairings keep matching alignment length.
///
/// The pronunciations may differ, but slot count must stay synchronized.
#[test]
fn test_pho_mod_alignment_example() -> Result<(), String> {
    // Example showing typical %pho/%mod alignment
    // Child says "fwee" for "three"
    let pho = PhoTier::new_pho(vec![PhoItem::Word(PhoWord::new("fwi"))]);
    let mod_tier = PhoTier::new_mod(vec![PhoItem::Word(PhoWord::new("θri"))]);

    assert_eq!(pho.len(), mod_tier.len()); // Should align 1-1

    // Check different pronunciations
    match (&pho.items[0], &mod_tier.items[0]) {
        (PhoItem::Word(pho_word), PhoItem::Word(mod_word)) => {
            assert_ne!(pho_word, mod_word); // Different pronunciations
        }
        _ => return Err("Expected Word items".to_string()),
    }
    Ok(())
}
