//! Item types used by `%sin` gesture/sign dependent tiers.
//!
//! CHAT reference anchors:
//! - [Gestures](https://talkbank.org/0info/manuals/CHAT.html#Gestures)
//! - [Sign language coding](https://talkbank.org/0info/manuals/CHAT.html#SignLanguage)

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt::Write as FmtWrite;
use std::ops::{Deref, DerefMut};
use talkbank_derive::{SemanticEq, SpanShift};

use super::super::WriteChat;
use crate::ErrorSink;
use crate::model::NonEmptyString;
use crate::validation::{Validate, ValidationContext};

/// Gesture/sign tier content item
///
/// Represents a single item in a %sin tier, which can be:
/// - A simple gesture token (e.g., `g:ball:dpoint`, `0`)
/// - A sin group containing multiple gestures (e.g., `〔g:x g:y〕`)
///
/// # Item Types
///
/// **Token**: Single gesture or no gesture
/// - Gesture code: `g:referent:type` (e.g., `g:ball:dpoint`)
/// - No gesture: `0` (word spoken without accompanying gesture)
///
/// **SinGroup**: Multiple gestures for one word using `〔...〕` brackets
/// - Captures multiple simultaneous or sequential gestures
/// - Example: `〔g:toy:hold g:toy:shake〕`
///
/// # Gesture Code Format
///
/// Gesture codes follow the pattern: `g:referent:gesture_type`
///
/// Where:
/// - **g**: Fixed gesture marker
/// - **referent**: What the gesture refers to (ball, mom, toy, cookie, etc.)
/// - **gesture_type**: Type of gesture (dpoint, hold, give, show, reach, etc.)
///
/// # Common Gesture Types
///
/// - **dpoint**: Deictic pointing (pointing to indicate referent)
/// - **hold**: Holding gesture
/// - **give**: Giving gesture
/// - **show**: Showing gesture
/// - **point**: General pointing
/// - **reach**: Reaching gesture
/// - **take**: Taking gesture
/// - **touch**: Touching gesture
///
/// # CHAT Format Examples
///
/// Simple gesture:
/// ```text
/// g:ball:dpoint
/// ```
///
/// No gesture:
/// ```text
/// 0
/// ```
///
/// Multiple gestures for one word:
/// ```text
/// 〔g:toy:hold g:toy:shake〕
/// ```
///
/// Complete tier example:
/// ```text
/// *CHI: I want ball .
/// %sin: 0 0 g:ball:dpoint .
/// ```
///
/// # References
///
/// - [Gesture Coding](https://talkbank.org/0info/manuals/CHAT.html#Gestures)
/// - [Sign Language](https://talkbank.org/0info/manuals/CHAT.html#SignLanguage)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
#[serde(tag = "type", content = "content", rename_all = "lowercase")]
pub enum SinItem {
    /// Simple gesture token
    Token(SinToken),

    /// Sin group `〔...〕` containing multiple gestures
    #[serde(rename = "sin_group")]
    SinGroup(SinGroupGestures),
}

/// A single gesture/sign token (e.g., `g:ball:dpoint` or `0` for no gesture).
///
/// References:
/// - <https://talkbank.org/0info/manuals/CHAT.html#Gestures>
/// - <https://talkbank.org/0info/manuals/CHAT.html#Sign_Tier>
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
#[serde(transparent)]
pub struct SinToken(pub NonEmptyString);

impl SinToken {
    /// Create a new token, returning `None` if the text is empty.
    ///
    /// This constructor is useful for user-input paths where empty values are
    /// expected and should be handled without panics.
    pub fn new(text: impl AsRef<str>) -> Option<Self> {
        NonEmptyString::new(text).map(Self)
    }

    /// Create a new token without checking for empty text.
    ///
    /// Use this only when callers already enforce non-empty invariants (for
    /// example, parser code after lexical validation).
    pub fn new_unchecked(text: impl AsRef<str>) -> Self {
        Self(NonEmptyString::new_unchecked(text))
    }
}

impl Deref for SinToken {
    type Target = str;

    /// Borrows the underlying token text.
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<str> for SinToken {
    /// Borrows the token as `&str` for APIs that accept borrowed text.
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl WriteChat for SinToken {
    /// Writes the token verbatim (for example `g:ball:dpoint` or `0`).
    fn write_chat<W: FmtWrite>(&self, w: &mut W) -> std::fmt::Result {
        w.write_str(self.as_ref())
    }
}

impl Validate for SinToken {
    /// Enforces non-empty token text.
    fn validate(&self, context: &ValidationContext, errors: &impl ErrorSink) {
        let ctx = context.clone().with_field_label("sin token");
        self.0.validate(&ctx, errors);
    }
}

/// A group of gestures within `〔...〕` brackets for a single word.
///
/// References:
/// - <https://talkbank.org/0info/manuals/CHAT.html#Sign_Group>
/// - <https://talkbank.org/0info/manuals/CHAT.html#Sign_Tier>
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
#[serde(transparent)]
#[schemars(transparent)]
pub struct SinGroupGestures(pub Vec<SinToken>);

impl SinGroupGestures {
    /// Create a new gesture group from a list of tokens.
    ///
    /// Order is preserved because grouped `%sin` serialization is positional and
    /// should not be normalized.
    pub fn new(gestures: Vec<SinToken>) -> Self {
        Self(gestures)
    }

    /// Returns `true` if the group contains no gestures.
    ///
    /// Empty groups are represented directly and can be handled by alignment-
    /// aware validation at the tier level.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Deref for SinGroupGestures {
    type Target = Vec<SinToken>;

    /// Borrows the underlying gesture-token list.
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for SinGroupGestures {
    /// Mutably borrows the underlying gesture-token list.
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<Vec<SinToken>> for SinGroupGestures {
    /// Wraps gesture tokens without allocating.
    ///
    /// This keeps parser output conversion lightweight when building large
    /// gesture corpora.
    fn from(gestures: Vec<SinToken>) -> Self {
        Self(gestures)
    }
}

impl Validate for SinGroupGestures {
    /// Group-level semantic checks are performed when validating `%sin` alignment.
    fn validate(&self, _context: &ValidationContext, _errors: &impl ErrorSink) {}
}

impl WriteChat for SinItem {
    /// Serializes one `%sin` item, including tortoise-shell brackets for grouped gestures.
    fn write_chat<W: FmtWrite>(&self, w: &mut W) -> std::fmt::Result {
        match self {
            SinItem::Token(token) => token.write_chat(w),
            SinItem::SinGroup(gestures) => {
                w.write_char('〔')?; // U+3014 LEFT TORTOISE SHELL BRACKET
                for (i, gesture) in gestures.iter().enumerate() {
                    if i > 0 {
                        w.write_char(' ')?;
                    }
                    gesture.write_chat(w)?;
                }
                w.write_char('〕') // U+3015 RIGHT TORTOISE SHELL BRACKET
            }
        }
    }
}
