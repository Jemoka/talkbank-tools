//! Shared bracketed-content model used by groups, quotations, and related tiers.
//!
//! CHAT reference anchors:
//! - [Group](https://talkbank.org/0info/manuals/CHAT.html#Group)
//! - [Quotation](https://talkbank.org/0info/manuals/CHAT.html#Quotation)
//! - [Phonological coding](https://talkbank.org/0info/manuals/CHAT.html#Phonological_Coding)
//! - [Sign language coding](https://talkbank.org/0info/manuals/CHAT.html#SignLanguage)

use super::{
    Action, Annotated, Event, LongFeatureBegin, LongFeatureEnd, NonvocalBegin, NonvocalEnd,
    NonvocalSimple, OtherSpokenEvent, OverlapPoint, Pause, ReplacedWord, Separator,
    UnderlineMarker, Word, WriteChat,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::ops::{Deref, DerefMut};
use talkbank_derive::{SemanticEq, SpanShift};

/// Token variants allowed inside bracketed constructs.
///
/// Shared across regular groups, phonological/sign groups, and quotations so
/// parsing and serialization logic can treat nested payloads consistently.
///
/// # CHAT Format Examples
///
/// **Regular group with retracing:**
/// ```text
/// *CHI: <I want> [/] I need cookie .
/// ```
///
/// **Phonological group:**
/// ```text
/// *CHI: hello ‹hɛloʊ› !
/// ```
///
/// **Sign group:**
/// ```text
/// *CHI: 〔wave goodbye〕 .
/// ```
///
/// **Quotation:**
/// ```text
/// *CHI: "I said hello" .
/// ```
///
/// **Nested structures:**
/// ```text
/// *CHI: <I <really> [/] really want it> [/] I need it .
/// ```
///
/// # Allowed Content
///
/// Unlike main tier content, bracketed content has some restrictions:
/// - Groups inside brackets MUST have scoped annotations (e.g., `[/]`, `[//]`)
/// - Bare actions (without annotations) are rare but allowed
/// - All CA markers (overlaps, separators) are allowed
///
/// # References
///
/// - [Group](https://talkbank.org/0info/manuals/CHAT.html#Group)
/// - [Phonological Coding](https://talkbank.org/0info/manuals/CHAT.html#Phonological_Coding)
/// - [Sign Language Coding](https://talkbank.org/0info/manuals/CHAT.html#Sign_Language)
/// - [Quotation](https://talkbank.org/0info/manuals/CHAT.html#Quotation)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum BracketedItem {
    /// Bare word with NO brackets
    /// Boxed to keep enum size bounded.
    Word(Box<Word>),
    /// Word WITH scoped annotations (e.g., "hello [* m]", "hello [= greeting]")
    /// Boxed to reduce enum size (216 bytes → 8 bytes)
    #[serde(rename = "annotated_word")]
    AnnotatedWord(Box<Annotated<Word>>),
    /// Replaced word - word with [: replacement] (e.g., "hello [: world]")
    /// Boxed to reduce enum size (248 bytes → 8 bytes)
    #[serde(rename = "replaced_word")]
    ReplacedWord(Box<ReplacedWord>),
    /// Event (e.g., &=laughs)
    Event(Event),
    /// Event WITH scoped annotations
    #[serde(rename = "annotated_event")]
    AnnotatedEvent(Annotated<Event>),
    /// Pause (e.g., (.), (1.5))
    Pause(Pause),
    /// Action (e.g., 0) - bare action without annotations (rare)
    Action(Action),
    /// Action WITH scoped annotations (e.g., <0 [= ! whining]>)
    #[serde(rename = "annotated_action")]
    AnnotatedAction(Annotated<Action>),
    /// Nested regular group `<...>` WITH scoped annotations (non-retrace only)
    #[serde(rename = "annotated_group")]
    AnnotatedGroup(Annotated<super::Group>),
    /// Nested retraced content — words the speaker said then corrected.
    Retrace(Box<super::Retrace>),
    /// Nested phonological group ‹...›
    #[serde(rename = "pho_group")]
    PhoGroup(super::PhoGroup),
    /// Nested sign group 〔...〕
    #[serde(rename = "sin_group")]
    SinGroup(super::SinGroup),
    /// Nested quotation "..."
    Quotation(super::Quotation),
    /// Overlap point marker (CA transcription: ⌊, ⌋, ⌈, ⌉)
    #[serde(rename = "overlap_point")]
    OverlapPoint(OverlapPoint),
    /// Separator (commas, etc. used in CA transcription)
    /// Canonicalized output always has a space before the separator
    Separator(Separator),
    /// Internal bullet/timestamp within group
    #[serde(rename = "internal_bullet")]
    InternalBullet(super::Bullet),
    /// Freecode - free-form inline annotation (e.g., "[^ comment]")
    Freecode(super::Freecode),
    /// Long feature begin marker (&{l=LABEL)
    #[serde(rename = "long_feature_begin")]
    LongFeatureBegin(LongFeatureBegin),
    /// Long feature end marker (&}l=LABEL)
    #[serde(rename = "long_feature_end")]
    LongFeatureEnd(LongFeatureEnd),
    /// Underline begin marker (\u0002\u0001)
    #[serde(rename = "underline_begin")]
    UnderlineBegin(UnderlineMarker),
    /// Underline end marker (\u0002\u0002)
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

impl WriteChat for BracketedItem {
    /// Serializes one bracket-internal item in its canonical CHAT surface form.
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        match self {
            BracketedItem::Word(word) => word.write_chat(w),
            BracketedItem::AnnotatedWord(ann) => ann.write_chat(w),
            BracketedItem::ReplacedWord(rw) => rw.write_chat(w),
            BracketedItem::Event(event) => event.write_chat(w),
            BracketedItem::AnnotatedEvent(ann) => ann.write_chat(w),
            BracketedItem::Pause(pause) => pause.write_chat(w),
            BracketedItem::Action(action) => action.write_chat(w),
            BracketedItem::AnnotatedAction(ann) => ann.write_chat(w),
            BracketedItem::AnnotatedGroup(ann) => ann.write_chat(w),
            BracketedItem::PhoGroup(pho) => pho.write_chat(w),
            BracketedItem::SinGroup(sin) => sin.write_chat(w),
            BracketedItem::Quotation(quot) => quot.write_chat(w),
            BracketedItem::OverlapPoint(marker) => marker.write_chat(w),
            BracketedItem::Separator(sep) => sep.write_chat(w),
            BracketedItem::InternalBullet(bullet) => {
                w.write_char(' ')?;
                bullet.write_chat(w)
            }
            BracketedItem::Freecode(freecode) => freecode.write_chat(w),
            BracketedItem::LongFeatureBegin(marker) => marker.write_chat(w),
            BracketedItem::LongFeatureEnd(marker) => marker.write_chat(w),
            BracketedItem::UnderlineBegin(_) => {
                w.write_char('\u{0002}')?;
                w.write_char('\u{0001}')
            }
            BracketedItem::UnderlineEnd(_) => {
                w.write_char('\u{0002}')?;
                w.write_char('\u{0002}')
            }
            BracketedItem::NonvocalBegin(marker) => marker.write_chat(w),
            BracketedItem::NonvocalEnd(marker) => marker.write_chat(w),
            BracketedItem::NonvocalSimple(marker) => marker.write_chat(w),
            BracketedItem::Retrace(retrace) => retrace.write_chat(w),
            BracketedItem::OtherSpokenEvent(event) => event.write_chat(w),
        }
    }
}

/// Shared payload container for bracketed constructs.
///
/// Keeps nested item ordering exactly as parsed so roundtrip serialization
/// preserves original grouping layout.
///
/// # CHAT Format Examples
///
/// **Regular group content:**
/// ```text
/// <I want>         → BracketedContent { content: [Word("I"), Word("want")] }
/// ```
///
/// **Phonological group content:**
/// ```text
/// ‹hɛloʊ›          → BracketedContent { content: [Word("hɛloʊ")] }
/// ```
///
/// **Quotation content:**
/// ```text
/// "hello there"    → BracketedContent { content: [Word("hello"), Word("there")] }
/// ```
///
/// **Complex nested content:**
/// ```text
/// <I <really> want it>  → BracketedContent with nested AnnotatedGroup
/// ```
///
/// # Usage
///
/// This type is used internally by `Group`, `PhoGroup`, `SinGroup`, and `Quotation`
/// to store their content. It provides methods for checking emptiness and length,
/// and implements `WriteChat` for CHAT format serialization.
///
/// # References
///
/// - [Group](https://talkbank.org/0info/manuals/CHAT.html#Group)
/// - [Phonological Coding](https://talkbank.org/0info/manuals/CHAT.html#Phonological_Coding)
/// - [Sign Language Coding](https://talkbank.org/0info/manuals/CHAT.html#Sign_Language)
/// - [Quotation](https://talkbank.org/0info/manuals/CHAT.html#Quotation)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
pub struct BracketedContent {
    /// Ordered bracket-internal items.
    pub content: BracketedItems,
}

impl BracketedContent {
    /// Wraps bracket-internal items as bracketed payload content.
    pub fn new(content: Vec<BracketedItem>) -> Self {
        Self {
            content: content.into(),
        }
    }

    /// Returns `true` when the payload has no items.
    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }

    /// Returns number of bracket-internal items.
    pub fn len(&self) -> usize {
        self.content.len()
    }
}

impl WriteChat for BracketedContent {
    /// Serializes bracket-internal content only (without surrounding bracket chars).
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        for (i, item) in self.content.iter().enumerate() {
            // Add space before items, except:
            // - First item (i == 0)
            // - InternalBullet items (they have their own NAK delimiters: word ␕timing␕ next)
            if i > 0 && !matches!(item, BracketedItem::InternalBullet(_)) {
                w.write_char(' ')?;
            }
            item.write_chat(w)?;
        }
        Ok(())
    }
}

/// Collection of items inside a bracketed construct.
///
/// References:
/// - <https://talkbank.org/0info/manuals/CHAT.html#Group_Scopes>
/// - <https://talkbank.org/0info/manuals/CHAT.html#QuotationFollows_Linker>
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
#[serde(transparent)]
#[schemars(transparent)]
pub struct BracketedItems(pub Vec<BracketedItem>);

impl BracketedItems {
    /// Wraps an owned bracket-item vector.
    pub fn new(items: Vec<BracketedItem>) -> Self {
        Self(items)
    }

    /// Returns `true` when no bracket items are present.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Deref for BracketedItems {
    type Target = Vec<BracketedItem>;

    /// Borrows the underlying bracketed-item vector.
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for BracketedItems {
    /// Mutably borrows the underlying bracketed-item vector.
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<Vec<BracketedItem>> for BracketedItems {
    /// Wraps bracketed items without copying.
    fn from(items: Vec<BracketedItem>) -> Self {
        Self(items)
    }
}

impl<'a> IntoIterator for &'a BracketedItems {
    type Item = &'a BracketedItem;
    type IntoIter = std::slice::Iter<'a, BracketedItem>;

    /// Iterates immutably over bracketed items.
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl<'a> IntoIterator for &'a mut BracketedItems {
    type Item = &'a mut BracketedItem;
    type IntoIter = std::slice::IterMut<'a, BracketedItem>;

    /// Iterates mutably over bracketed items.
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter_mut()
    }
}

impl IntoIterator for BracketedItems {
    type Item = BracketedItem;
    type IntoIter = std::vec::IntoIter<BracketedItem>;

    /// Consumes the wrapper and yields owned bracketed items.
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl crate::validation::Validate for BracketedItems {
    /// Bracket-structure validation runs at enclosing group/utterance boundaries.
    fn validate(
        &self,
        _context: &crate::validation::ValidationContext,
        _errors: &impl crate::ErrorSink,
    ) {
    }
}
