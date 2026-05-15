//! `%sin` token/group container tests.
//!
//! These cases cover token/group construction and canonical `%sin` formatting
//! so gesture-tier model behavior stays stable during parser changes.

use super::{SinGroupGestures, SinItem, SinTier, SinToken};

/// Builds a `%sin` tier from explicit `SinItem` values.
#[test]
fn test_sin_tier_new() {
    let items = vec![
        SinItem::Token(SinToken::new_unchecked("g:toy:dpoint")),
        SinItem::Token(SinToken::new_unchecked("0")),
    ];
    let tier = SinTier::new(items);
    assert_eq!(tier.items.len(), 2);
    assert_eq!(tier.len(), 2);
    assert!(!tier.is_empty());
}

/// Handles an empty `%sin` tier.
#[test]
fn test_sin_tier_empty() {
    let tier = SinTier::new(vec![]);
    assert_eq!(tier.len(), 0);
    assert!(tier.is_empty());
}

/// Preserves literal `0` gesture placeholders.
#[test]
fn test_sin_tier_with_zeros() -> Result<(), String> {
    let tier = SinTier::from_tokens(vec!["0".to_string(), "0".to_string(), "0".to_string()]);
    assert_eq!(tier.len(), 3);
    match &tier.items[0] {
        SinItem::Token(text) => assert_eq!(text.as_ref(), "0"),
        _ => return Err("Expected Token".to_string()),
    }
    Ok(())
}

/// Preserves explicit gesture tokens.
#[test]
fn test_sin_tier_with_gesture_codes() -> Result<(), String> {
    let tier = SinTier::from_tokens(vec![
        "g:toy:dpoint".to_string(),
        "gg:toyy:dpointt".to_string(),
    ]);
    assert_eq!(tier.len(), 2);
    match &tier.items[0] {
        SinItem::Token(text) => assert_eq!(text.as_ref(), "g:toy:dpoint"),
        _ => return Err("Expected Token".to_string()),
    }
    Ok(())
}

/// Preserves grouped `%sin` gestures.
#[test]
fn test_sin_tier_with_groups() -> Result<(), String> {
    let items = vec![
        SinItem::Token(SinToken::new_unchecked("b")),
        SinItem::SinGroup(SinGroupGestures::new(vec![
            SinToken::new_unchecked("c"),
            SinToken::new_unchecked("d"),
        ])),
        SinItem::Token(SinToken::new_unchecked("e")),
    ];
    let tier = SinTier::new(items);
    assert_eq!(tier.len(), 3);
    match &tier.items[1] {
        SinItem::SinGroup(gestures) => assert_eq!(gestures.len(), 2),
        _ => return Err("Expected SinGroup".to_string()),
    }
    Ok(())
}

/// Serializes `%sin` tiers to canonical CHAT text.
#[test]
fn test_sin_tier_chat_format() {
    use crate::model::WriteChat;
    let tier = SinTier::from_tokens(vec![
        "g:toy:dpoint".to_string(),
        "0".to_string(),
        "g:book:hold".to_string(),
    ]);
    let output = tier.to_chat_string();
    assert_eq!(output, "%sin:\tg:toy:dpoint 0 g:book:hold");
}
