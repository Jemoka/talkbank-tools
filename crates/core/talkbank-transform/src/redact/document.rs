//! Top-level orchestration.

use smol_str::SmolStr;
use talkbank_model::alignment::helpers::{ContentItemMut, walk_content_mut};
use talkbank_model::validation::NotValidated;
use talkbank_model::{ChatFile, EventType, Line, WriteChat};

use super::REDACTED_TEXT;
use super::dependent_tier::{keep_dependent_tier, sanitize_dependent_tier};
use super::error::RedactError;
use super::header::sanitize_header;
use super::placeholder::PlaceholderState;
use super::policy::SanitizationPolicy;
use super::word::sanitize_word;

/// The result of sanitizing a parsed `ChatFile`.
///
/// Spans become stale after sanitization; `WriteChat` doesn't depend on
/// them, but downstream JSON consumers should treat sanitized output as
/// a fresh document with no offsets back into the original.
pub struct SanitizedDocument {
    inner: ChatFile<NotValidated>,
}

impl SanitizedDocument {
    /// Writes the sanitized document in CHAT surface form to `w`.
    pub fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        self.inner.write_chat(w)
    }

    /// Serializes the sanitized document to a fresh `String`.
    pub fn to_chat_string(&self) -> String {
        let mut out = String::new();
        let _ = self.write_chat(&mut out);
        out
    }
}

/// Sanitizes a parsed `ChatFile` according to `policy`.
pub fn sanitize(
    mut input: ChatFile<NotValidated>,
    _policy: &SanitizationPolicy,
) -> Result<SanitizedDocument, RedactError> {
    let mut state = PlaceholderState::new();

    for line in input.lines.0.iter_mut() {
        match line {
            Line::Header { header, .. } => {
                sanitize_header(header);
            }
            Line::Utterance(utt) => {
                for header in utt.preceding_headers.iter_mut() {
                    sanitize_header(header);
                }

                let main_content = utt.main.content.content.0.as_mut_slice();
                walk_content_mut(main_content, None, &mut |item| {
                    sanitize_content_item(item, &mut state);
                });

                utt.dependent_tiers.retain(|tier| keep_dependent_tier(tier));
                for tier in utt.dependent_tiers.iter_mut() {
                    sanitize_dependent_tier(tier, &mut state);
                }
            }
        }
    }

    Ok(SanitizedDocument { inner: input })
}

/// Single-pass redaction of every leaf item visited by `walk_content_mut`.
fn sanitize_content_item(item: ContentItemMut<'_>, state: &mut PlaceholderState) {
    match item {
        ContentItemMut::Word(word) => sanitize_word(word, state),
        ContentItemMut::ReplacedWord(replaced) => {
            sanitize_word(&mut replaced.word, state);
            for word in replaced.replacement.words.0.iter_mut() {
                sanitize_word(word, state);
            }
        }
        ContentItemMut::Event(event) => {
            event.event_type = EventType::new(REDACTED_TEXT);
        }
        ContentItemMut::Freecode(fc) => {
            fc.text = SmolStr::new(REDACTED_TEXT);
        }
        ContentItemMut::OtherSpokenEvent(ose) => {
            ose.text = SmolStr::new(REDACTED_TEXT);
        }
        ContentItemMut::Separator(_)
        | ContentItemMut::Pause(_)
        | ContentItemMut::Action(_)
        | ContentItemMut::OverlapPoint(_)
        | ContentItemMut::InternalBullet(_)
        | ContentItemMut::LongFeatureBegin(_)
        | ContentItemMut::LongFeatureEnd(_)
        | ContentItemMut::UnderlineBegin(_)
        | ContentItemMut::UnderlineEnd(_)
        | ContentItemMut::NonvocalBegin(_)
        | ContentItemMut::NonvocalEnd(_)
        | ContentItemMut::NonvocalSimple(_) => {}
    }
}
