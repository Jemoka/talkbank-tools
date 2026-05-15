//! Inline-bullet content container used by free-form dependent tiers.
//!
//! CHAT reference anchors:
//! - [Dependent tiers](https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers)
//! - [Media bullets](https://talkbank.org/0info/manuals/CHAT.html#Bullets)

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::ops::{Deref, DerefMut};
use talkbank_derive::{SemanticEq, SpanShift};

use super::BulletContentSegment;

/// Content that may contain inline media bullets interspersed with text.
///
/// Used by tiers like %act, %cod, %com, %gpx, etc. that can have media timing
/// markers embedded within free-form text. The content is stored as an ordered
/// sequence of segments (text, bullets, and picture references) that preserves
/// the exact interleaving and timing information.
///
/// # Alignment vs. Inline Bullets
///
/// - **%mor, %gra, %pho, %sin**: Use word-by-word alignment (one token per word)
/// - **%act, %cod, %com, etc.**: Use inline bullets within free-form text
///
/// # CHAT Format Examples
///
/// Plain text (no bullets):
/// ```text
/// %com:\tThis is a simple comment
/// ```
///
/// Text with single bullet:
/// ```text
/// %act:\tChild picks up toy 2051689_2052652 and examines it
/// ```
///
/// Multiple bullets:
/// ```text
/// %cod:\tfirst event 1000_2000 then pause 3000_4000 final action 5000_6000
/// ```
///
/// Comment with picture reference:
/// ```text
/// %com:\tSee photo %pic:\"scene01.jpg\" for context
/// ```
///
/// Mixed content:
/// ```text
/// %gpx:\tpoints at 1000_1500 picture %pic:\"toy.jpg\" then reaches 2000_2500
/// ```
///
/// # Use Cases
///
/// - **%act**: Time-stamped action descriptions during utterances
/// - **%cod**: Precise timing of behavioral codes
/// - **%com**: Comments with media references and timing
/// - **%exp**: Explanations linked to specific time points
/// - **%gpx**: Gesture timing within descriptions
///
/// # References
///
/// - [CHAT Manual: Dependent Tiers](https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers)
/// - [CHAT Manual: Media Bullets](https://talkbank.org/0info/manuals/CHAT.html#Bullets)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
pub struct BulletContent {
    /// Ordered sequence of text and bullet segments
    pub segments: BulletContentSegments,
}

impl BulletContent {
    /// Constructs bullet-aware content from pre-parsed segments.
    ///
    /// The segment order is preserved exactly so tier writers can reproduce
    /// original timing and media placement without re-tokenization.
    pub fn new(segments: Vec<BulletContentSegment>) -> Self {
        Self {
            segments: segments.into(),
        }
    }

    /// Convenience constructor for plain text with no explicit bullets.
    ///
    /// This is primarily useful for tests and simple programmatic construction.
    /// Parser outputs usually build [`BulletContentSegment`] sequences directly.
    pub fn from_text(text: impl Into<smol_str::SmolStr>) -> Self {
        Self {
            segments: vec![BulletContentSegment::text(text)].into(),
        }
    }

    /// Returns `true` when the payload carries no meaningful content.
    ///
    /// A single empty text segment is also treated as empty content.
    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
            || (self.segments.len() == 1
                && matches!(&self.segments[0], BulletContentSegment::Text(text) if text.text.is_empty()))
    }
}

/// Ordered sequence of inline-bullet-capable segments.
///
/// References:
/// - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
/// - <https://talkbank.org/0info/manuals/CHAT.html#Bullets>
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
#[serde(transparent)]
#[schemars(transparent)]
pub struct BulletContentSegments(pub Vec<BulletContentSegment>);

impl BulletContentSegments {
    /// Wraps segments while preserving transcript order.
    pub fn new(segments: Vec<BulletContentSegment>) -> Self {
        Self(segments)
    }

    /// Returns `true` when this segment list is empty.
    ///
    /// Callers that need semantic emptiness should use [`BulletContent::is_empty`],
    /// which also treats a single empty text segment as empty payload.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Deref for BulletContentSegments {
    type Target = Vec<BulletContentSegment>;

    /// Borrows the underlying segment vector.
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for BulletContentSegments {
    /// Mutably borrows the underlying segment vector.
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<Vec<BulletContentSegment>> for BulletContentSegments {
    /// Wraps an owned vector without copying.
    fn from(segments: Vec<BulletContentSegment>) -> Self {
        Self(segments)
    }
}

impl<'a> IntoIterator for &'a BulletContentSegments {
    type Item = &'a BulletContentSegment;
    type IntoIter = std::slice::Iter<'a, BulletContentSegment>;

    /// Iterates immutably over segments.
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl<'a> IntoIterator for &'a mut BulletContentSegments {
    type Item = &'a mut BulletContentSegment;
    type IntoIter = std::slice::IterMut<'a, BulletContentSegment>;

    /// Iterates mutably over segments.
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter_mut()
    }
}

impl IntoIterator for BulletContentSegments {
    type Item = BulletContentSegment;
    type IntoIter = std::vec::IntoIter<BulletContentSegment>;

    /// Consumes the wrapper and yields owned segments.
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl crate::validation::Validate for BulletContentSegments {
    /// Segment-level checks run in tier-specific validators with field context.
    fn validate(
        &self,
        _context: &crate::validation::ValidationContext,
        _errors: &impl crate::ErrorSink,
    ) {
    }
}
