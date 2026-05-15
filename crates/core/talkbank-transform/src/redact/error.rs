//! Domain error type for sanitization failures.

use thiserror::Error;

/// Errors that can arise while sanitizing a [`ChatFile`](talkbank_model::ChatFile).
///
/// Sanitization is mostly infallible — the structure of a parsed
/// `ChatFile` always supports walking and replacing content. The only
/// error condition reachable today is empty word content, which the
/// upstream parser usually rejects but the sanitizer guards against
/// rather than panicking.
#[derive(Debug, Error)]
pub enum RedactError {
    /// A `WordContents` SmallVec was empty when the placeholder
    /// generator expected at least one segment to replace.
    #[error("empty word content at utterance {utterance_index}, word {word_index}")]
    EmptyWordContent {
        /// Zero-based utterance index in document order.
        utterance_index: usize,
        /// Zero-based word index within the utterance (post-walk-words).
        word_index: usize,
    },
}
