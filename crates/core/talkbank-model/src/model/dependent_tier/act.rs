//! Action description tier (%act) for CHAT transcripts.
//!
//! The %act tier documents non-verbal actions, gestures, and physical activities
//! that occur during or around an utterance. Actions can be timed relative to
//! specific words or positioned relative to the utterance.
//!
//! # Timing Markers
//!
//! Actions can include timing markers indicating when they occur:
//! - **`<1w>`**: During word 1
//! - **`<1w-2w>`**: Spanning words 1-2
//! - **`<aft>`**: After the utterance
//! - **`<bef>`**: Before the utterance
//! - Timestamps: `2061689_2062652` (milliseconds)
//!
//! # Format
//!
//! ```text
//! %act:\taction description [timing markers]
//! ```
//!
//! # CHAT Manual Reference
//!
//! - [Actions Tier](https://talkbank.org/0info/manuals/CHAT.html#Actions)
//!
//! # Examples
//!
//! ```text
//! *CHI: I want ball .
//! %act: <1w-2w> reaches toward shelf
//! %act: <3w> points at red ball
//!
//! *MOT: here you go .
//! %act: <aft> hands ball to child
//! ```

use super::{BulletContent, WriteChat};
use crate::Span;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift};

/// Action description tier (%act).
///
/// Describes physical actions, gestures, and non-verbal behaviors that occur
/// during or around speech. Actions can be timed to specific words using
/// timing markers, and may contain inline media bullets.
///
/// # Timing Conventions
///
/// - **`<1w>`**: Action occurs during word 1
/// - **`<1w-3w>`**: Action spans from word 1 to word 3
/// - **`<aft>`**: Action occurs after the utterance
/// - **`<bef>`**: Action occurs before the utterance
/// - **Inline bullets**: `\u0015START_END\u0015` for precise media timing
///
/// # Common Uses
///
/// - Documenting manipulative actions (picking up, holding, giving objects)
/// - Recording body movements and positioning
/// - Noting facial expressions and eye gaze
/// - Timing actions relative to speech
/// - Multimodal interaction analysis
///
/// # CHAT Manual Reference
///
/// - [Actions Coding](https://talkbank.org/0info/manuals/CHAT.html#Actions)
///
/// # Examples
///
/// ```
/// use talkbank_model::model::{ActTier, BulletContent};
///
/// let act = ActTier::new(BulletContent::from_text("<1w-2w> reaches toward shelf"));
/// assert_eq!(act.to_chat(), "%act:\t<1w-2w> reaches toward shelf");
/// ```
///
/// **CHAT format:**
/// ```text
/// *CHI: I want that .
/// %act: <3w> points at cookie jar
/// ```
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
pub struct ActTier {
    /// Action description with optional timing markers and inline bullets.
    pub content: BulletContent,

    /// Source span for error reporting (not serialized to JSON)
    #[serde(skip)]
    #[schemars(skip)]
    pub span: Span,
}

impl ActTier {
    /// Constructs an `%act` tier from parsed bullet-aware content.
    pub fn new(content: BulletContent) -> Self {
        Self {
            content,
            span: Span::DUMMY,
        }
    }

    /// Sets source span metadata used in diagnostics.
    pub fn with_span(mut self, span: Span) -> Self {
        self.span = span;
        self
    }

    /// Convenience constructor for plain action text.
    ///
    /// Use [`Self::new`] when callers already have parsed [`BulletContent`].
    pub fn from_text(text: impl Into<smol_str::SmolStr>) -> Self {
        Self {
            content: BulletContent::from_text(text),
            span: Span::DUMMY,
        }
    }

    /// Returns `true` when the tier has no serializable payload.
    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }

    /// Allocating helper that writes `%act:\t...` into a `String`.
    pub fn to_chat(&self) -> String {
        let mut s = String::new();
        let _ = self.write_chat(&mut s);
        s
    }
}

impl WriteChat for ActTier {
    /// Serializes one full `%act` line.
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        w.write_str("%act:\t")?;
        self.content.write_chat(w)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ensures plain action text round-trips to `%act` form.
    #[test]
    fn test_act_tier_simple() {
        let tier = ActTier::from_text("picks up toy");
        assert_eq!(tier.to_chat(), "%act:\tpicks up toy");
        assert!(!tier.is_empty());
    }

    /// Ensures word-index timing markers are preserved.
    #[test]
    fn test_act_tier_with_timing() {
        let tier = ActTier::from_text("<1w-2w> holds object out to Amy");
        assert_eq!(tier.to_chat(), "%act:\t<1w-2w> holds object out to Amy");
        assert!(!tier.is_empty());
    }

    /// Empty text produces an empty payload according to `is_empty`.
    #[test]
    fn test_act_tier_empty() {
        let tier = ActTier::from_text("");
        assert!(tier.is_empty());
    }
}
