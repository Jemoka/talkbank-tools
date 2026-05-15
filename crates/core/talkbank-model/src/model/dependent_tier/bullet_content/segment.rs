//! Segment primitives for free-form tiers that embed media bullets.
//!
//! These segment types preserve exact inline ordering between plain text,
//! timing bullets, picture references, and continuation markers so roundtrip
//! serialization remains lossless.
//!
//! CHAT reference anchor:
//! - [Media bullets](https://talkbank.org/0info/manuals/CHAT.html#Bullets)

use crate::model::MediaTiming;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift};

/// A segment of content that can be text, a media bullet, or a picture reference.
///
/// Represents one piece of content within a tier that supports inline bullets.
/// Content is broken into segments to preserve the exact position and timing of
/// media bullets relative to the surrounding text.
///
/// # CHAT Format Examples
///
/// Plain text segment:
/// ```text
/// "this is text"
/// ```
///
/// Media timing bullet:
/// ```text
/// \u00152051689_2052652\u0015
/// ```
///
/// Picture reference:
/// ```text
/// \u0015%pic:\"photo.jpg\"\u0015
/// ```
///
/// # References
///
/// - [CHAT Manual: Media Bullets](https://talkbank.org/0info/manuals/CHAT.html#Bullets)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BulletContentSegment {
    /// Plain text content
    Text(BulletContentText),
    /// Media timing bullet: \u0015START_END\u0015
    Bullet(BulletContentBullet),
    /// Picture reference: \u0015%pic:\"filename\"\u0015
    /// Only valid in @Comment and %com
    Picture(BulletContentPicture),
    /// Continuation marker: newline + tab (preserves wrapping)
    Continuation,
}

impl BulletContentSegment {
    /// Create a plain text segment.
    ///
    /// Text segments preserve exact lexical content between bullets and are
    /// emitted verbatim during CHAT serialization.
    pub fn text(s: impl Into<smol_str::SmolStr>) -> Self {
        Self::Text(BulletContentText { text: s.into() })
    }

    /// Create a media timing bullet segment with the given start and end times.
    ///
    /// The times are stored in milliseconds and later serialized in the
    /// canonical `\u0015START_END\u0015` form.
    pub fn bullet(start_ms: u64, end_ms: u64) -> Self {
        Self::Bullet(BulletContentBullet::new(start_ms, end_ms))
    }

    /// Create a picture reference segment with the given filename.
    ///
    /// Picture segments are preserved as explicit units so callers do not need
    /// to parse `%pic:` syntax out of plain text.
    pub fn picture(filename: impl Into<smol_str::SmolStr>) -> Self {
        Self::Picture(BulletContentPicture {
            filename: filename.into(),
        })
    }

    /// Create a continuation marker segment (newline + tab).
    ///
    /// Continuation markers preserve wrapped dependent-tier formatting for
    /// roundtrip output fidelity.
    pub fn continuation() -> Self {
        Self::Continuation
    }
}

/// Plain text content within a bullet content segment.
///
/// This wrapper keeps text segments explicit in the AST so downstream code does
/// not have to infer whether a string came from plain text vs media syntax.
///
/// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
pub struct BulletContentText {
    /// The text content.
    pub text: smol_str::SmolStr,
}

/// A media timing bullet within a bullet-content segment.
///
/// This is an alias of [`MediaTiming`], reused here to keep timing semantics
/// consistent across headers, main tiers, and dependent-tier bullet payloads.
pub type BulletContentBullet = MediaTiming;

/// A picture reference within a bullet content segment.
///
/// References:
/// - <https://talkbank.org/0info/manuals/CHAT.html#Internal_Media>
/// - <https://talkbank.org/0info/manuals/CHAT.html#Working_with_Media>
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
pub struct BulletContentPicture {
    /// The picture filename (e.g., `"photo.jpg"`).
    pub filename: smol_str::SmolStr,
}
