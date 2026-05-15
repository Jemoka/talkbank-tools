//! Main-line utterance content union (`*SPK:` body items between speaker and terminator).
//!
//! CHAT reference anchors:
//! - [Main line](https://talkbank.org/0info/manuals/CHAT.html#Main_Line)
//! - [Annotations](https://talkbank.org/0info/manuals/CHAT.html#Annotations)
//! - [Local Event](https://talkbank.org/0info/manuals/CHAT.html#Local_Event)
//! - [Overlap](https://talkbank.org/0info/manuals/CHAT.html#Overlap)
//! - [Media linking](https://talkbank.org/0info/manuals/CHAT.html#Media_Linking)

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift};

use crate::model::{
    Action, Annotated, Event, Freecode, Group, LongFeatureBegin, LongFeatureEnd, NonvocalBegin,
    NonvocalEnd, NonvocalSimple, OtherSpokenEvent, OverlapPoint, Pause, PhoGroup, Quotation,
    ReplacedWord, Retrace, Separator, SinGroup, UnderlineMarker, Word,
};

/// Content items that can appear on the main tier of a CHAT utterance.
///
/// This enum represents all possible content between the speaker code and terminator
/// on a main tier. Content items can be simple (words, events, pauses) or complex
/// (annotated items, groups with nested content).
///
/// # CHAT Format Examples
///
/// ```text
/// *CHI: I want cookie .                    Word variants
/// *CHI: I want [* m] cookie .              AnnotatedWord
/// *CHI: I want [: need] cookie .           ReplacedWord
/// *CHI: <I want> [/] I need it .           Group, AnnotatedGroup
/// *CHI: the dog &=barks loudly .           Event
/// *CHI: um (.) yeah (1.5) okay .           Pause
/// *CHI: ‹hello there› !                    PhoGroup
/// *CHI: 〔wave goodbye〕 .                  SinGroup
/// *CHI: "I said hello" .                   Quotation
/// *CHI: [^ note] yeah .                    Freecode
/// *CHI: well , you know .                  Separator (CA mode)
/// *CHI: ⌊yeah⌋ !                           OverlapPoint (CA mode)
/// ```
///
/// # Annotation Patterns
///
/// Several content types have both "bare" and "annotated" variants:
/// - `Word` / `AnnotatedWord` - word with scoped annotations like `[* m]`, `[= explanation]`
/// - `Event` / `AnnotatedEvent` - event with annotations
/// - `Group` / `AnnotatedGroup` - group with scoped annotations like `[/]`, `[//]`
///
/// The `Annotated<T>` wrapper adds scoped annotations to any annotatable type.
///
/// # References
///
/// - [Main line](https://talkbank.org/0info/manuals/CHAT.html#Main_Line)
/// - [Group scopes](https://talkbank.org/0info/manuals/CHAT.html#Group_Scopes)
/// - [Annotations](https://talkbank.org/0info/manuals/CHAT.html#Annotations)
/// - [Error Coding](https://talkbank.org/0info/manuals/CHAT.html#Error_Coding)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum UtteranceContent {
    /// Bare word with NO brackets
    /// Boxed to keep enum size bounded.
    Word(Box<Word>),
    /// Word WITH scoped annotations (e.g., "hello [* m]", "hello [= greeting]")
    /// `Annotated<T>` MUST have nonempty scoped_annotations
    /// Boxed to reduce enum size (216 bytes → 8 bytes)
    #[serde(rename = "annotated_word")]
    AnnotatedWord(Box<Annotated<Word>>),
    /// Replaced word - word with [: replacement] (e.g., "hello [: world]")
    /// This is a distinct semantic concept, not just an annotation
    /// Boxed to reduce enum size (248 bytes → 8 bytes)
    #[serde(rename = "replaced_word")]
    ReplacedWord(Box<ReplacedWord>),
    /// Bare event with NO brackets
    Event(Event),
    /// Event WITH scoped annotations
    #[serde(rename = "annotated_event")]
    AnnotatedEvent(Annotated<Event>),
    /// Pause (no annotations possible)
    Pause(Pause),
    /// Regular group <...> - bare (no annotations)
    Group(Group),
    /// Regular group <...> WITH scoped annotations (non-retrace only)
    #[serde(rename = "annotated_group")]
    AnnotatedGroup(Annotated<Group>),
    /// Retraced content — words the speaker said then corrected.
    ///
    /// First-class variant ensures every `match` handles retraces explicitly.
    /// Excluded from %mor alignment; included in %pho/%sin/%wor.
    ///
    /// CHAT: `<I want> [/] I need` or `word [/] word`
    Retrace(Box<Retrace>),
    /// Phonological group ‹...› (cannot have annotations)
    #[serde(rename = "pho_group")]
    PhoGroup(PhoGroup),
    /// Sign/gesture group 〔...〕 (cannot have annotations)
    #[serde(rename = "sin_group")]
    SinGroup(SinGroup),
    /// Quotation "..." (cannot have annotations)
    Quotation(Quotation),
    /// Action WITH scoped annotations (bare Action is an error - actions defined by brackets)
    #[serde(rename = "annotated_action")]
    AnnotatedAction(Annotated<Action>),
    /// Freecode - free-form inline annotation (e.g., "[^ comment]")
    Freecode(Freecode),
    /// CA separator (e.g., Comma, Semicolon, Colon, intonation markers)
    /// Can appear between content items or at end of utterance in CA mode
    /// Canonicalized output always has a space before the separator
    #[serde(rename = "separator")]
    Separator(Separator),
    /// Overlap point marker (e.g., "⌊", "⌋", "⌈", "⌉" with optional indices)
    /// Used in CA transcription to mark overlap boundaries
    #[serde(rename = "overlap_point")]
    OverlapPoint(OverlapPoint),

    /// Internal bullet/timestamp (media_url within content, not terminal)
    /// These mark timing for content segments, distinct from terminal bullets
    #[serde(rename = "internal_bullet")]
    InternalBullet(super::super::Bullet),

    /// Long feature begin marker (&{l=LABEL)
    #[serde(rename = "long_feature_begin")]
    LongFeatureBegin(LongFeatureBegin),

    /// Long feature end marker (&}l=LABEL)
    #[serde(rename = "long_feature_end")]
    LongFeatureEnd(LongFeatureEnd),

    /// Underline begin marker (\u0002\u0001) - can appear at main tier level
    #[serde(rename = "underline_begin")]
    UnderlineBegin(UnderlineMarker),

    /// Underline end marker (\u0002\u0002) - can appear at main tier level
    #[serde(rename = "underline_end")]
    UnderlineEnd(UnderlineMarker),

    /// Nonvocal scope begin marker (&{n=LABEL)
    #[serde(rename = "nonvocal_begin")]
    NonvocalBegin(NonvocalBegin),

    /// Nonvocal scope end marker (&}n=LABEL)
    #[serde(rename = "nonvocal_end")]
    NonvocalEnd(NonvocalEnd),

    /// Nonvocal simple marker (&{n=LABEL})
    #[serde(rename = "nonvocal_simple")]
    NonvocalSimple(NonvocalSimple),

    /// Other spoken event (&*SPEAKER:word) - someone else speaking in background
    #[serde(rename = "other_spoken_event")]
    OtherSpokenEvent(OtherSpokenEvent),
}

impl UtteranceContent {
    /// Returns `true` if this item is an opening overlap marker (`⌈` or `⌊`).
    ///
    /// Opening overlap markers should NOT have a space after them in serialization.
    /// Example: `⌈very⌉` not `⌈ very⌉`
    pub fn is_opening_overlap(&self) -> bool {
        matches!(self, UtteranceContent::OverlapPoint(marker) if marker.is_opening())
    }

    /// Returns `true` if this item is a closing overlap marker (`⌉` or `⌋`).
    ///
    /// Closing overlap markers should NOT have a space before them in serialization.
    /// Example: `⌈very⌉` not `⌈very ⌉`
    pub fn is_closing_overlap(&self) -> bool {
        matches!(self, UtteranceContent::OverlapPoint(marker) if marker.is_closing())
    }
}
