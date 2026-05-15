//! REPEAT -- mark utterances containing revisions with `[+ rep]` postcodes.
//!
//! Reimplements CLAN's `repeat` command, which adds a `[+ rep]` postcode to
//! utterances from a target speaker that contain revision markers. Only
//! utterances that do not already have `[+ rep]` are modified.
//!
//! # Revision markers detected
//!
//! - `[//]` -- retracing (exact repetition with correction)
//! - `[///]` -- multiple retracing
//! - `[/-]` -- reformulation (false start)
//! - `[/?]` -- uncertain retracing
//!
//! Note: Simple repetitions (`[/]`) do **not** trigger the `[+ rep]` marker.
//! Only revisions and reformulations do.
//!
//! # Differences from CLAN
//!
//! - Operates on the typed AST rather than raw text line scanning.
//! - Uses the framework transform pipeline (parse → transform → serialize).
//! - Detects revision markers by matching typed `ContentAnnotation` variants
//!   instead of scanning for bracket text patterns like `[//]` in raw lines.
//! - Speaker filtering uses typed `SpeakerCode` comparison rather than
//!   string-matching the `*SPK:` prefix.

use talkbank_model::{
    BracketedItem, ChatFile, Line, Postcode, RetraceKind, SpeakerCode, UtteranceContent,
};

use crate::framework::{TransformCommand, TransformError};

/// REPEAT transform: mark utterances with revisions using `[+ rep]` postcodes.
pub struct RepeatCommand {
    /// Target speaker to process. Only utterances from this speaker are checked.
    pub speaker: SpeakerCode,
}

impl RepeatCommand {
    /// Create a new `RepeatCommand` targeting the given speaker.
    pub fn new(speaker: SpeakerCode) -> Self {
        Self { speaker }
    }
}

impl TransformCommand for RepeatCommand {
    type Config = SpeakerCode;

    /// Append `[+ rep]` to target-speaker utterances containing revision markers.
    fn transform(&self, file: &mut ChatFile) -> Result<(), TransformError> {
        for line in file.lines.iter_mut() {
            if let Line::Utterance(utterance) = line {
                // Only process utterances from the target speaker
                if utterance.main.speaker != self.speaker {
                    continue;
                }

                // Check if utterance contains revision markers
                if has_revision_markers(&utterance.main.content.content) {
                    // Add [+ rep] postcode if not already present
                    let already_has_rep = utterance
                        .main
                        .content
                        .postcodes
                        .iter()
                        .any(|p| p.text == "rep");

                    if !already_has_rep {
                        utterance.main.content.postcodes.push(Postcode::new("rep"));
                    }
                }
            }
        }

        Ok(())
    }
}

/// Returns whether a retrace kind is a revision (not simple repetition).
///
/// Simple repetitions (`[/]`) do NOT count as revisions.
fn is_revision_kind(kind: RetraceKind) -> bool {
    matches!(
        kind,
        RetraceKind::Full           // [//]
            | RetraceKind::Multiple  // [///]
            | RetraceKind::Reformulation // [/-]
    )
}

/// Check if utterance content contains any revision markers.
fn has_revision_markers(content: &[UtteranceContent]) -> bool {
    for item in content {
        match item {
            UtteranceContent::Retrace(retrace) if is_revision_kind(retrace.kind) => {
                return true;
            }
            UtteranceContent::Group(group)
                if has_revision_markers_in_brackets(&group.content.content) =>
            {
                return true;
            }
            UtteranceContent::AnnotatedGroup(annotated)
                if has_revision_markers_in_brackets(&annotated.inner.content.content) =>
            {
                return true;
            }
            _ => {}
        }
    }
    false
}

/// Check if bracketed content contains revision markers.
fn has_revision_markers_in_brackets(items: &[BracketedItem]) -> bool {
    for item in items {
        match item {
            BracketedItem::Retrace(retrace) if is_revision_kind(retrace.kind) => {
                return true;
            }
            BracketedItem::AnnotatedGroup(annotated)
                if has_revision_markers_in_brackets(&annotated.inner.content.content) =>
            {
                return true;
            }
            _ => {}
        }
    }
    false
}
