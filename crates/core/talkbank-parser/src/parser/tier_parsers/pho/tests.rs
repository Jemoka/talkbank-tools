//! Tests for this subsystem.
//!

use talkbank_model::model::{PhoTier, PhoTierType, WriteChat};

// Note: These tests are for the old string-based API
// New CST-based tests should be in the integration tests

/// Tests pho tier construction.
#[test]
fn test_pho_tier_construction() {
    use talkbank_model::model::{PhoItem, PhoWord};
    let items = vec![
        PhoItem::Word(PhoWord("wʌn".into())),
        PhoItem::Word(PhoWord("tu".into())),
        PhoItem::Word(PhoWord("θɹi".into())),
    ];
    let tier = PhoTier::new(PhoTierType::Pho, items);
    assert_eq!(tier.items.len(), 3);
    assert!(tier.is_pho());

    let output = tier.to_chat_string();
    assert_eq!(output, "%pho:\twʌn tu θɹi");
}

/// Tests mod tier type.
#[test]
fn test_mod_tier_type() {
    let tier = PhoTier::new(PhoTierType::Mod, vec![]);
    assert!(tier.is_mod());
    assert_eq!(tier.to_chat_string(), "%mod:\t");
}

/// Tests empty tier.
#[test]
fn test_empty_tier() {
    let tier = PhoTier::new(PhoTierType::Pho, vec![]);
    assert_eq!(tier.items.len(), 0);
    assert!(tier.is_empty());
}

/// Tests single token.
#[test]
fn test_single_token() -> Result<(), String> {
    use talkbank_model::model::{PhoItem, PhoWord};
    let items = vec![PhoItem::Word(PhoWord("hɛloʊ".into()))];
    let tier = PhoTier::new(PhoTierType::Pho, items);
    assert_eq!(tier.len(), 1);
    match &tier.items[0] {
        PhoItem::Word(word) => assert_eq!(word.0, "hɛloʊ"),
        _ => return Err("Expected Word item".to_string()),
    }
    Ok(())
}

/// Tests unicode tokens.
#[test]
fn test_unicode_tokens() -> Result<(), String> {
    use talkbank_model::model::{PhoItem, PhoWord};
    let items = vec![
        PhoItem::Word(PhoWord("ʃə".into())),
        PhoItem::Word(PhoWord("ɪz".into())),
        PhoItem::Word(PhoWord("naɪs".into())),
    ];
    let tier = PhoTier::new(PhoTierType::Pho, items);
    assert_eq!(tier.len(), 3);
    match &tier.items[0] {
        PhoItem::Word(word) => assert_eq!(word.0, "ʃə"),
        _ => return Err("Expected Word item".to_string()),
    }
    Ok(())
}
