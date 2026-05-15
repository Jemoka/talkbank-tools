//! Time representation for media synchronization.
//!
//! CHAT reference anchors:
//! - [Media linking](https://talkbank.org/0info/manuals/CHAT.html#Media_Linking)
//! - [Word tier](https://talkbank.org/0info/manuals/CHAT.html#Word_Tier)

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift};

/// A time interval in milliseconds for media synchronization.
///
/// Represents a start and end time, typically for linking transcript segments
/// to audio/video files.
///
/// References:
/// - <https://talkbank.org/0info/manuals/CHAT.html#Media_Linking>
/// - <https://talkbank.org/0info/manuals/CHAT.html#Working_with_Media>
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    JsonSchema,
    SemanticEq,
    SpanShift,
    Default,
)]
pub struct MediaTiming {
    /// Start time in milliseconds.
    pub start_ms: u64,
    /// End time in milliseconds.
    pub end_ms: u64,
}

impl MediaTiming {
    /// Creates a media time interval using millisecond offsets.
    pub fn new(start_ms: u64, end_ms: u64) -> Self {
        Self { start_ms, end_ms }
    }
}
