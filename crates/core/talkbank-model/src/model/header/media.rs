//! Typed model for the `@Media` header.
//!
//! CHAT format:
//! `@Media:\tfilename, media_type[, status]`
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Media_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Working_with_Media>

use super::{MediaStatus, MediaType, WriteChat, codes::MediaFilename};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift};

/// Parsed payload of one `@Media` header line.
///
/// `filename` and `media_type` are required by CHAT. `status` is optional and
/// used by some corpora to mark missing or not-yet-linked assets.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
pub struct MediaHeader {
    /// Media basename without extension.
    pub filename: MediaFilename,

    /// Capture modality token (`audio` or `video`).
    pub media_type: MediaType,

    /// Optional availability status (`missing`, `unlinked`, `notrans`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<MediaStatus>,
}

impl MediaHeader {
    /// Builds an `@Media` payload with required fields.
    pub fn new(filename: impl Into<MediaFilename>, media_type: MediaType) -> Self {
        Self {
            filename: filename.into(),
            media_type,
            status: None,
        }
    }

    /// Sets optional media-link status metadata.
    pub fn with_status(mut self, status: MediaStatus) -> Self {
        self.status = Some(status);
        self
    }
}

impl WriteChat for MediaHeader {
    /// Serializes canonical `@Media` text, including optional status.
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        write!(
            w,
            "@Media:\t{}, {}",
            self.filename,
            self.media_type.as_str()
        )?;

        if let Some(ref status) = self.status {
            write!(w, ", {}", status.as_str())?;
        }

        Ok(())
    }
}

impl std::fmt::Display for MediaHeader {
    /// Formats the media header in canonical CHAT text form.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.write_chat(f)
    }
}
