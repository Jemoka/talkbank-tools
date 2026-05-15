//! Media timing bullets (`\u{0015}start_end\u{0015}`).
//!
//! Bullets anchor utterance content to media offsets.
//!
//! # CHAT Format References
//!
//! - [Bullets](https://talkbank.org/0info/manuals/CHAT.html#Bullets)

use super::WriteChat;
use crate::model::MediaTiming;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift};

/// Provenance of a media timing bullet.
///
/// Bullets are set in two distinct ways — by the UTR pre-pass (provisional)
/// and by human annotation or FA post-processing (authoritative). The source
/// field is not serialized and has no effect on CHAT output; it is used
/// exclusively by `batchalign-chat-ops` to decide how to update the utterance
/// bullet after FA word timings are injected.
///
/// ## Rules encoded by each variant
///
/// - `Utr` — `update_utterance_bullet` **overwrites** this bullet with the
///   FA word span. The UTR window was a provisional grouping hint; once FA
///   has produced word timings the hint is discarded.
///
/// - `Authoritative` — `update_utterance_bullet` **unions** this bullet with
///   the FA word span (never shrinks). Hand-linked annotations may cover
///   leading fillers, trailing gestures, or other non-alignable content whose
///   timing would be lost if the bullet were blindly overwritten with the
///   word span.
///
/// The default is `Authoritative`, which matches the behavior of all bullets
/// that arrive from a parsed CHAT file or were constructed programmatically
/// after FA (e.g., by `Bullet::new`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum BulletSource {
    /// Provisional hint from the UTR (Utterance Timing Recovery) pre-pass.
    ///
    /// FA word timings overwrite this bullet.
    Utr,
    /// Hand-linked annotation, previous FA run, or FA-derived timing.
    ///
    /// FA word timings union with (never shrink) this bullet.
    #[default]
    Authoritative,
}

/// Media timing bullet attached to an utterance or header-level comment payload.
///
/// Start/end offsets are stored in milliseconds. The `source` field records
/// whether the bullet is a provisional UTR hint or an authoritative annotation;
/// it is not serialized and does not appear in CHAT output.
///
/// # CHAT Format
///
/// Bullets appear at the end of utterances using the `\u{0015}` (NEGATIVE ACKNOWLEDGE)
/// character as a delimiter:
///
/// ```text
/// \u{0015}start_end\u{0015}
/// ```
///
/// # Examples
///
/// ```text
/// *CHI: I want cookie . \u{0015}0_1073\u{0015}
/// *MOT: here you go . \u{0015}1073_2456\u{0015}
/// ```
///
/// # References
///
/// - [Bullets](https://talkbank.org/0info/manuals/CHAT.html#Bullets)
/// - [Media Linking](https://talkbank.org/0info/manuals/CHAT.html#MediaLinking)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
pub struct Bullet {
    /// Start/end media offsets in milliseconds.
    #[serde(flatten)]
    pub timing: MediaTiming,

    /// Source location metadata for diagnostics (not serialized).
    #[serde(skip)]
    #[schemars(skip)]
    pub span: crate::Span,

    /// Provenance of this bullet (not serialized, not part of CHAT output).
    ///
    /// Controls whether `update_utterance_bullet` overwrites or unions when
    /// FA word timings are available. See [`BulletSource`] for invariants.
    #[serde(skip)]
    #[schemars(skip)]
    #[semantic_eq(skip)]
    #[span_shift(skip)]
    pub source: BulletSource,
}

impl Bullet {
    /// Build an authoritative bullet with dummy span metadata.
    ///
    /// Use this constructor for FA-derived timing, hand-linked annotations,
    /// and any bullet that should not be overwritten by a subsequent UTR or FA pass.
    pub fn new(start_ms: u64, end_ms: u64) -> Self {
        Self {
            timing: MediaTiming::new(start_ms, end_ms),
            span: crate::Span::DUMMY,
            source: BulletSource::Authoritative,
        }
    }

    /// Build a provisional UTR hint bullet.
    ///
    /// UTR sets this before FA runs to mark the audio window for grouping.
    /// `update_utterance_bullet` will overwrite it once FA produces word timings.
    pub fn utr_hint(start_ms: u64, end_ms: u64) -> Self {
        Self {
            timing: MediaTiming::new(start_ms, end_ms),
            span: crate::Span::DUMMY,
            source: BulletSource::Utr,
        }
    }

    /// Attach source span metadata used by diagnostics.
    ///
    /// Span metadata is not serialized and does not affect timing semantics.
    pub fn with_span(mut self, span: crate::Span) -> Self {
        self.span = span;
        self
    }
}

impl WriteChat for Bullet {
    /// Serializes canonical CHAT bullet syntax.
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        w.write_char('\u{0015}')?;
        write!(w, "{}_{}", self.timing.start_ms, self.timing.end_ms)?;
        w.write_char('\u{0015}')
    }
}

impl std::fmt::Display for Bullet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.write_chat(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bullet_roundtrip() {
        let bullet = Bullet::new(0, 1073);
        let output = bullet.to_string();
        assert_eq!(output, "\u{0015}0_1073\u{0015}");
    }

    #[test]
    fn bullet_zero_time_roundtrip() {
        let bullet = Bullet::new(0, 0);
        let output = bullet.to_string();
        assert_eq!(output, "\u{0015}0_0\u{0015}");
    }
}
