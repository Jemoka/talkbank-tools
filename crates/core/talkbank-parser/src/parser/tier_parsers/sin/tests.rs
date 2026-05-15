//! Tests for this subsystem.
//!

use talkbank_model::model::{SinTier, WriteChat};

// Note: These tests are for the old string-based API
// New CST-based tests should be in the integration tests

/// Tests sin tier construction.
#[test]
fn test_sin_tier_construction() {
    use talkbank_model::model::{SinItem, SinToken};
    let items = vec![
        SinItem::Token(SinToken::new_unchecked("g:toy:dpoint")),
        SinItem::Token(SinToken::new_unchecked("0")),
    ];
    let tier = SinTier::new(items);
    assert_eq!(tier.items.len(), 2);

    let output = tier.to_chat_string();
    assert_eq!(output, "%sin:\tg:toy:dpoint 0");
}

/// Tests empty tier.
#[test]
fn test_empty_tier() {
    let tier = SinTier::new(vec![]);
    assert_eq!(tier.items.len(), 0);
    assert!(tier.is_empty());
}

/// Tests single token.
#[test]
fn test_single_token() -> Result<(), String> {
    use talkbank_model::model::{SinItem, SinToken};
    let items = vec![SinItem::Token(SinToken::new_unchecked("g:toy:dpoint"))];
    let tier = SinTier::new(items);
    assert_eq!(tier.len(), 1);
    match &tier.items[0] {
        SinItem::Token(text) => assert_eq!(text.as_ref(), "g:toy:dpoint"),
        _ => return Err("Expected Token".to_string()),
    }
    Ok(())
}

/// Tests all zeros.
#[test]
fn test_all_zeros() -> Result<(), String> {
    let tier = SinTier::from_tokens(vec!["0".to_string(), "0".to_string(), "0".to_string()]);
    assert_eq!(tier.len(), 3);
    match &tier.items[0] {
        talkbank_model::model::SinItem::Token(text) => assert_eq!(text.as_ref(), "0"),
        _ => return Err("Expected Token".to_string()),
    }
    Ok(())
}
