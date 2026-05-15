//! Temporal validation for media bullets
//!
//! Implements CLAN CHECK command temporal constraints:
//! - E701 (Error 83): Per-speaker start-time monotonicity
//! - E704 (Error 133): Per-speaker overlap with 500ms tolerance
//! - E729 (Error 84): Cross-speaker overlap (opt-in only, CLAN `+c0`)
//!
//! E729 is NOT part of default validation. CLAN only fires error 84 when the
//! user passes `+c0` to CHECK, enabling strict timeline contiguity mode (which
//! also fires error 85 for gaps). In normal conversational transcripts,
//! cross-speaker overlap is ubiquitous and expected.
//!
//! Note: E702/E703 (strict timeline mode) are reserved for future use.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Bullets>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Working_with_Media>
//! - <https://talkbank.org/0info/manuals/CHAT.html#CA_Option>

use crate::model::{Bullet, ChatFile, Header, UtteranceContent, Word};
use crate::validation::ValidationState;
use crate::{ErrorContext, ErrorSink, ParseError, Severity, SourceLocation, Span};
use std::collections::HashMap;

// Import error codes
// E729 is retained for future strict-bullet mode (CLAN `+c0`) but not used
// in default validation. See `validate_cross_speaker_overlap()`.
#[allow(unused_imports)]
use crate::codes::temporal::E729;
use crate::codes::temporal::{E701, E704};

/// CLAN Error 133 tolerance for same-speaker overlap in milliseconds.
///
/// Small overlaps are common around annotation boundaries; this threshold mirrors
/// CHECK behavior before issuing `E704`.
const SPEAKER_OVERLAP_TOLERANCE_MS: u64 = 500;

/// Validates temporal constraints on utterance bullets.
///
/// This follows CLAN CHECK semantics for bullet timing:
/// 1. Per-speaker start-time monotonicity (`E701` / Error 83)
/// 2. Per-speaker self-overlap with tolerance (`E704` / Error 133)
///
/// The check is skipped when CA mode is enabled, where timing constraints are
/// intentionally relaxed for conversation-analysis workflows.
pub fn validate_temporal_constraints<S: ValidationState>(
    file: &ChatFile<S>,
    ca_mode: bool,
    errors: &impl ErrorSink,
) {
    if ca_mode {
        return;
    }

    // Collect all relevant bullets in document order
    let bullets = collect_bullets(file);

    // 1. Per-speaker start-time monotonicity (E701 - CLAN Error 83)
    validate_global_timeline(&bullets, errors);

    // 2. Per-speaker overlap (E704 - CLAN Error 133)
    validate_speaker_timelines(&bullets, errors);

    // E729 (CLAN Error 84) is NOT run here. CLAN only fires error 84 when
    // the user passes `+c0` to CHECK, enabling strict timeline contiguity.
    // Cross-speaker overlap is normal in conversational transcripts.
}

/// Returns whether the file headers enable CA mode (`@Options: CA`).
///
/// This helper is retained for clarity and parity with other option-mode
/// detectors, even though the main entrypoint receives `ca_mode` directly.
#[allow(dead_code)]
fn is_ca_mode<S: ValidationState>(file: &ChatFile<S>) -> bool {
    file.headers().any(|h| {
        if let Header::Options { options } = h {
            options.iter().any(|opt| opt.enables_ca_mode())
        } else {
            false
        }
    })
}

/// Captured bullet metadata used by temporal validation passes.
#[derive(Debug)]
struct BulletInfo<'a> {
    utterance_idx: usize,
    speaker: &'a str,
    bullet: &'a Bullet,
    has_timeable_content: bool,
}

/// Collects utterance bullets used by temporal validators.
///
/// Rules:
/// - Main speaker tiers only (ignore dependent tiers)
/// - Only check terminator bullets (the single bullet in TierContent)
///
/// The collected vector preserves utterance order so monotonicity and overlap
/// checks share a consistent traversal basis.
fn collect_bullets<S: ValidationState>(file: &ChatFile<S>) -> Vec<BulletInfo<'_>> {
    let mut bullets = Vec::new();

    for (idx, utt) in file.utterances().enumerate() {
        // Prefer explicit terminator bullet; recover from internal bullet token if needed.
        let bullet = utt.main.content.bullet.as_ref().or_else(|| {
            utt.main.content.content.iter().find_map(|item| match item {
                UtteranceContent::InternalBullet(b) => Some(b),
                _ => None,
            })
        });

        if let Some(bullet) = bullet {
            let has_timeable_content = has_transcribed_content(&utt.main.content.content);
            bullets.push(BulletInfo {
                utterance_idx: idx,
                speaker: utt.main.speaker.as_ref(),
                bullet,
                has_timeable_content,
            });
        }
    }

    bullets
}

/// Returns whether utterance content includes at least one transcribed word.
///
/// Returns false for turns containing only untranscribed material (xxx, yyy, www).
/// CLAN CHECK skips such turns for speaker self-overlap (E704) validation.
fn has_transcribed_content(content: &[UtteranceContent]) -> bool {
    /// Returns `true` for a lexical word token with usable transcription.
    fn word_is_transcribed(word: &Word) -> bool {
        word.untranscribed().is_none() && !word.cleaned_text().is_empty()
    }

    content.iter().any(|item| match item {
        UtteranceContent::Word(word) => word_is_transcribed(word),
        UtteranceContent::AnnotatedWord(annotated) => word_is_transcribed(&annotated.inner),
        UtteranceContent::ReplacedWord(replaced) => {
            word_is_transcribed(&replaced.word)
                || replaced.replacement.words.iter().any(word_is_transcribed)
        }
        UtteranceContent::Group(group) => has_transcribed_bracketed(&group.content),
        UtteranceContent::AnnotatedGroup(annotated) => {
            has_transcribed_bracketed(&annotated.inner.content)
        }
        UtteranceContent::PhoGroup(pho) => has_transcribed_bracketed(&pho.content),
        UtteranceContent::SinGroup(sin) => has_transcribed_bracketed(&sin.content),
        UtteranceContent::Quotation(quot) => has_transcribed_bracketed(&quot.content),
        _ => false,
    })
}

/// Returns whether bracketed content includes at least one transcribed word token.
fn has_transcribed_bracketed(content: &crate::model::BracketedContent) -> bool {
    use crate::model::BracketedItem;
    content.content.iter().any(|item| match item {
        BracketedItem::Word(word) => {
            word.untranscribed().is_none() && !word.cleaned_text().is_empty()
        }
        BracketedItem::AnnotatedWord(annotated) => {
            annotated.inner.untranscribed().is_none() && !annotated.inner.cleaned_text().is_empty()
        }
        _ => false,
    })
}

/// Validate per-speaker start-time monotonicity (`E701`, CLAN Error 83).
///
/// Rule: Each speaker's utterances must have non-decreasing start times.
/// Cross-speaker non-monotonicity is expected in multi-party conversations
/// (speakers naturally overlap), so only same-speaker violations are reported.
///
/// CLAN fires error 83 globally (cross-speaker), but its early-return
/// implementation accidentally suppresses many cross-speaker hits. We scope
/// to same-speaker intentionally — it matches the real intent of detecting
/// disordered timestamps without flagging normal conversational overlap.
fn validate_global_timeline(bullets: &[BulletInfo], errors: &impl ErrorSink) {
    let mut speaker_last_start: HashMap<&str, (usize, u64)> = HashMap::new();

    for bullet_info in bullets {
        if let Some(&(prev_idx, prev_start_ms)) = speaker_last_start.get(bullet_info.speaker)
            && bullet_info.bullet.timing.start_ms < prev_start_ms
        {
            errors.report(
                ParseError::new(
                    E701,
                    Severity::Error,
                    SourceLocation::new(bullet_info.bullet.span),
                    ErrorContext::new(
                        bullet_text(bullet_info.bullet),
                        Span::from_usize(0, bullet_text(bullet_info.bullet).len()),
                        bullet_text(bullet_info.bullet),
                    ),
                    format!(
                        "Same-speaker start time not monotonic: speaker '{}' utterance {} \
                         starts at {}ms but their utterance {} started at {}ms",
                        bullet_info.speaker,
                        bullet_info.utterance_idx + 1,
                        bullet_info.bullet.timing.start_ms,
                        prev_idx + 1,
                        prev_start_ms
                    ),
                )
                .with_suggestion(format!(
                    "Adjust bullet to start at or after {}ms",
                    prev_start_ms
                )),
            );
        }

        speaker_last_start.insert(
            bullet_info.speaker,
            (
                bullet_info.utterance_idx,
                bullet_info.bullet.timing.start_ms,
            ),
        );
    }
}

/// Validate per-speaker timelines (`E704`, CLAN Error 133).
///
/// Rule: Same speaker cannot overlap with themselves beyond 500ms tolerance
/// current.start_ms >= (previous.end_ms - 500)
/// The check ignores non-timeable utterances to match CLAN CHECK behavior.
fn validate_speaker_timelines(bullets: &[BulletInfo], errors: &impl ErrorSink) {
    let mut speaker_last_end: HashMap<&str, (usize, u64)> = HashMap::new();

    for bullet_info in bullets {
        // Match CHECK behavior: skip untranscribed-only/non-timeable tiers (e.g., "www").
        // These turns can carry broad segment bullets but do not represent timeable lexical
        // content for speaker-self overlap checks, so including them creates false E704 reports
        // compared with CLAN CHECK (e.g., long INV "www" scaffolding spans in some corpora).
        if !bullet_info.has_timeable_content {
            continue;
        }

        if let Some((prev_idx, prev_end_ms)) = speaker_last_end.get(bullet_info.speaker) {
            // Calculate overlap (0 if no overlap)
            let overlap = prev_end_ms.saturating_sub(bullet_info.bullet.timing.start_ms);

            if overlap > SPEAKER_OVERLAP_TOLERANCE_MS {
                errors.report(
                    ParseError::new(
                        E704,
                        Severity::Error,
                        SourceLocation::new(bullet_info.bullet.span),
                        ErrorContext::new(
                            bullet_text(bullet_info.bullet),
                            Span::from_usize(0, bullet_text(bullet_info.bullet).len()),
                            bullet_text(bullet_info.bullet),
                        ),
                        format!(
                            "Speaker '{}' overlaps with self: utterance {} ends at {}ms \
                             but utterance {} starts at {}ms ({}ms overlap exceeds {}ms tolerance)",
                            bullet_info.speaker,
                            prev_idx + 1,
                            prev_end_ms,
                            bullet_info.utterance_idx + 1,
                            bullet_info.bullet.timing.start_ms,
                            overlap,
                            SPEAKER_OVERLAP_TOLERANCE_MS
                        ),
                    )
                    .with_suggestion(format!(
                        "Adjust bullet to start at or after {}ms (tolerating {}ms overlap)",
                        prev_end_ms - SPEAKER_OVERLAP_TOLERANCE_MS,
                        SPEAKER_OVERLAP_TOLERANCE_MS
                    )),
                );
            }
        }

        // Update speaker's last end time
        speaker_last_end.insert(
            bullet_info.speaker,
            (bullet_info.utterance_idx, bullet_info.bullet.timing.end_ms),
        );
    }
}

/// Validate cross-speaker bullet overlap (`E729`, CLAN Error 84).
///
/// Rule: Current tier's BEG should not be less than the previous tier's END
/// when the speakers are different. Unlike self-overlap (E704), this has no
/// tolerance — any overlap is reported as a warning.
///
/// **Not called in default validation.** CLAN only fires error 84 when the
/// user passes `+c0` to CHECK, enabling strict timeline contiguity mode.
/// Cross-speaker overlap is normal and ubiquitous in conversational transcripts.
/// Retained for future strict-bullet mode.
#[allow(dead_code)]
fn validate_cross_speaker_overlap(bullets: &[BulletInfo], errors: &impl ErrorSink) {
    let mut prev_info: Option<&BulletInfo> = None;

    for bullet_info in bullets {
        if let Some(prev) = prev_info {
            // Only check cross-speaker overlap (same-speaker is handled by E704)
            if bullet_info.speaker != prev.speaker
                && bullet_info.bullet.timing.start_ms < prev.bullet.timing.end_ms
            {
                let overlap = prev.bullet.timing.end_ms - bullet_info.bullet.timing.start_ms;
                errors.report(
                    ParseError::new(
                        E729,
                        Severity::Error,
                        SourceLocation::new(bullet_info.bullet.span),
                        ErrorContext::new(
                            bullet_text(bullet_info.bullet),
                            Span::from_usize(0, bullet_text(bullet_info.bullet).len()),
                            bullet_text(bullet_info.bullet),
                        ),
                        format!(
                            "Bullet start {}ms is before previous tier end {}ms \
                             (speaker '{}' overlaps with '{}' by {}ms)",
                            bullet_info.bullet.timing.start_ms,
                            prev.bullet.timing.end_ms,
                            bullet_info.speaker,
                            prev.speaker,
                            overlap,
                        ),
                    )
                    .with_suggestion(
                        "Cross-speaker overlap may be intentional. \
                         Adjust timing or suppress E729 if deliberate.",
                    ),
                );
            }
        }
        prev_info = Some(bullet_info);
    }
}

/// Formats bullet timing as `start_end` for diagnostic context payloads.
fn bullet_text(bullet: &Bullet) -> String {
    format!("{}_{}", bullet.timing.start_ms, bullet.timing.end_ms)
}

#[cfg(test)]
mod tests {
    use super::has_transcribed_content;
    use crate::model::{UtteranceContent, Word, WordCategory};

    // Note: Full integration tests should go in talkbank-model/tests/

    /// Documents expected tolerance behavior around the `E704` boundary.
    ///
    /// This placeholder keeps the intended 499ms/501ms edge semantics visible
    /// until the full temporal fixture harness is added.
    #[test]
    fn test_speaker_overlap_tolerance() {
        // 499ms overlap should pass (within tolerance)
        // 501ms overlap should fail
        // This is a unit test - full integration tests elsewhere
    }

    #[test]
    fn fillers_count_as_timeable_content_for_utterance_bullets() {
        let content = vec![UtteranceContent::Word(Box::new(
            Word::new_unchecked("&-you_know", "you_know").with_category(WordCategory::Filler),
        ))];

        assert!(has_transcribed_content(&content));
    }

    #[test]
    fn untranscribed_only_content_is_not_timeable_for_utterance_bullets() {
        let content = vec![UtteranceContent::Word(Box::new(Word::new_unchecked(
            "xxx", "xxx",
        )))];

        assert!(!has_transcribed_content(&content));
    }
}
