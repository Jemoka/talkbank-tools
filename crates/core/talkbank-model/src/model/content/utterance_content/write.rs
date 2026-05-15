//! Serialization for `UtteranceContent` variants.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Bullets>
//!
//! Each variant maintains its own `write_chat` so nested structures can reuse
//! canonical serialization without duplicating bracket, pause, or overlap logic.
//! The module covers everything from words to long-feature markers so alignment
//! serializers can rely on this file for consistent textual output.

use crate::model::WriteChat;

use super::types::UtteranceContent;

impl UtteranceContent {
    /// Serializes one `UtteranceContent` item to CHAT surface text.
    ///
    /// Each enum variant delegates to its own serializer so marker ordering and
    /// escape conventions stay centralized with the owning type. This function
    /// is intentionally side-effect free and never performs normalization, so
    /// parsed surface distinctions are preserved through roundtrip writes.
    pub fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        match self {
            UtteranceContent::Word(word) => word.write_chat(w),
            UtteranceContent::AnnotatedWord(ann) => ann.write_chat(w),
            UtteranceContent::ReplacedWord(rw) => rw.write_chat(w),
            UtteranceContent::Event(event) => event.write_chat(w),
            UtteranceContent::AnnotatedEvent(ann) => ann.write_chat(w),
            UtteranceContent::Pause(pause) => pause.write_chat(w),
            UtteranceContent::Group(group) => group.write_chat(w),
            UtteranceContent::AnnotatedGroup(ann) => ann.write_chat(w),
            UtteranceContent::PhoGroup(pho) => pho.write_chat(w),
            UtteranceContent::SinGroup(sin) => sin.write_chat(w),
            UtteranceContent::Quotation(quot) => quot.write_chat(w),
            UtteranceContent::AnnotatedAction(ann) => ann.write_chat(w),
            UtteranceContent::Freecode(freecode) => freecode.write_chat(w),
            UtteranceContent::Separator(sep) => sep.write_chat(w),
            UtteranceContent::OverlapPoint(marker) => marker.write_chat(w),
            UtteranceContent::InternalBullet(bullet) => bullet.write_chat(w),
            UtteranceContent::LongFeatureBegin(marker) => marker.write_chat(w),
            UtteranceContent::LongFeatureEnd(marker) => marker.write_chat(w),
            UtteranceContent::UnderlineBegin(_) => {
                w.write_char('\u{0002}')?;
                w.write_char('\u{0001}')
            }
            UtteranceContent::UnderlineEnd(_) => {
                w.write_char('\u{0002}')?;
                w.write_char('\u{0002}')
            }
            UtteranceContent::NonvocalBegin(marker) => marker.write_chat(w),
            UtteranceContent::NonvocalEnd(marker) => marker.write_chat(w),
            UtteranceContent::NonvocalSimple(marker) => marker.write_chat(w),
            UtteranceContent::Retrace(retrace) => retrace.write_chat(w),
            UtteranceContent::OtherSpokenEvent(event) => event.write_chat(w),
        }
    }
}

impl std::fmt::Display for UtteranceContent {
    /// Formats this content item with CHAT serialization.
    ///
    /// Prefer this for diagnostics; hot serialization paths should call
    /// [`Self::write_chat`] with an existing buffer.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.write_chat(f)
    }
}
