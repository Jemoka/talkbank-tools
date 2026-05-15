//! Size-introspection test for `UtteranceContent` enum layout.
//!
//! This test is intentionally informational: it helps contributors reason about
//! enum-size pressure when deciding whether to box additional variants.

use super::UtteranceContent;
use crate::model::{
    Action, Annotated, Event, Freecode, Group, LongFeatureBegin, LongFeatureEnd, NonvocalBegin,
    NonvocalEnd, NonvocalSimple, OtherSpokenEvent, OverlapPoint, Pause, PhoGroup, Quotation,
    ReplacedWord, Separator, SinGroup, Word,
};

/// Prints variant sizes to guide enum boxing decisions.
///
/// The assertions are log-based rather than threshold-based because the goal is
/// to surface tradeoffs during refactors, not enforce one fixed target.
#[test]
fn test_utterance_content_sizes() {
    use std::mem::size_of;

    println!("\n=== UtteranceContent Enum Size Analysis ===");
    println!(
        "UtteranceContent enum size: {} bytes",
        size_of::<UtteranceContent>()
    );
    println!("\nVariant content sizes:");
    println!("  Word: {} bytes", size_of::<Word>());
    println!("  Annotated<Word>: {} bytes", size_of::<Annotated<Word>>());
    println!("  ReplacedWord: {} bytes", size_of::<ReplacedWord>());
    println!("  Event: {} bytes", size_of::<Event>());
    println!(
        "  Annotated<Event>: {} bytes",
        size_of::<Annotated<Event>>()
    );
    println!("  Pause: {} bytes", size_of::<Pause>());
    println!("  Group: {} bytes", size_of::<Group>());
    println!(
        "  Annotated<Group>: {} bytes",
        size_of::<Annotated<Group>>()
    );
    println!("  PhoGroup: {} bytes", size_of::<PhoGroup>());
    println!("  SinGroup: {} bytes", size_of::<SinGroup>());
    println!("  Quotation: {} bytes", size_of::<Quotation>());
    println!(
        "  Annotated<Action>: {} bytes",
        size_of::<Annotated<Action>>()
    );
    println!("  Freecode: {} bytes", size_of::<Freecode>());
    println!("  Separator: {} bytes", size_of::<Separator>());
    println!("  OverlapPoint: {} bytes", size_of::<OverlapPoint>());
    println!(
        "  Bullet: {} bytes",
        size_of::<super::super::super::Bullet>()
    );
    println!(
        "  LongFeatureBegin: {} bytes",
        size_of::<LongFeatureBegin>()
    );
    println!("  LongFeatureEnd: {} bytes", size_of::<LongFeatureEnd>());
    println!("  NonvocalBegin: {} bytes", size_of::<NonvocalBegin>());
    println!("  NonvocalEnd: {} bytes", size_of::<NonvocalEnd>());
    println!("  NonvocalSimple: {} bytes", size_of::<NonvocalSimple>());
    println!(
        "  OtherSpokenEvent: {} bytes",
        size_of::<OtherSpokenEvent>()
    );
    println!(
        "
Recommendation: If enum > 64 bytes, consider boxing largest variants"
    );
}
