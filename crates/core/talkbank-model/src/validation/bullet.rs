//! Bullet (media timestamp) validation functions
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Utterance_Media>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Media_Linking>

use crate::model::{Bullet, WriteChat};
use crate::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};

/// Validate one media bullet's internal timing invariants.
///
/// Currently enforces `start_ms < end_ms` (`E515`) and reports diagnostics using
/// reconstructed bullet text for user-facing context.
pub(crate) fn check_bullet(bullet: &Bullet, errors: &impl ErrorSink) {
    // E515: Check start < end
    if bullet.timing.start_ms >= bullet.timing.end_ms {
        // Use bullet's serialized form for error context
        let bullet_text = bullet.to_chat_string();

        errors.report(
            ParseError::new(
                ErrorCode::TimestampBackwards,
                Severity::Error,
                SourceLocation::new(bullet.span),
                ErrorContext::from_reconstructed(&bullet_text, bullet.span),
                format!(
                    "Media bullet start time ({}ms) must be less than end time ({}ms)",
                    bullet.timing.start_ms, bullet.timing.end_ms
                ),
            )
            .with_suggestion("Ensure start time is before end time in media bullets"),
        );
    }
}

/// Validate chronological ordering across a sequence of bullets.
///
/// Emits `E701` when a bullet starts before the previous bullet's start time.
pub fn check_bullet_monotonicity(bullets: &[&Bullet], errors: &impl ErrorSink) {
    for i in 1..bullets.len() {
        // E701: Check monotonicity - each start time must be >= previous start time
        if bullets[i].timing.start_ms < bullets[i - 1].timing.start_ms {
            let bullet_text = bullets[i].to_chat_string();

            errors.report(
                ParseError::new(
                    ErrorCode::TimestampBackwards,
                    Severity::Error,
                    SourceLocation::new(bullets[i].span),
                    ErrorContext::from_reconstructed(&bullet_text, bullets[i].span),
                    format!(
                        "Media bullet timestamp {}ms comes before previous timestamp {}ms (timestamps must increase monotonically)",
                        bullets[i].timing.start_ms, bullets[i-1].timing.start_ms
                    ),
                )
                .with_suggestion("Ensure media bullets are in chronological order within the same media file"),
            );
        }
    }
}

// Cross-speaker bullet overlap is implemented in validation::temporal::validate_cross_speaker_overlap().
