//! Text-based dependent tiers for CHAT transcripts.
//!
//! Text tiers provide free-form textual annotations for various purposes:
//! comments, explanations, addressee information, speech acts, situational context, etc.
//!
//! # CHAT Format References
//!
//! - [Comment Tier](https://talkbank.org/0info/manuals/CHAT.html#Comment_Tier)
//! - [Explanation Tier](https://talkbank.org/0info/manuals/CHAT.html#Explanation_Tier)
//! - [Addressee Tier](https://talkbank.org/0info/manuals/CHAT.html#Addressee_Tier)
//! - [Dependent Tiers](https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers)

use super::{BulletContent, WriteChat};
use crate::Span;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift};

macro_rules! define_text_tier {
    ($(#[$meta:meta])* $name:ident, $prefix:expr) => {
        $(#[$meta])*
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
        pub struct $name {
            /// Tier content with optional inline bullets and picture references.
            pub content: BulletContent,

            /// Source span for error reporting (not serialized to JSON)
            #[serde(skip)]
            #[schemars(skip)]
            pub span: Span,
        }

        impl $name {
            /// Constructs a text-dependent tier from parsed bullet-aware content.
            ///
            /// This is the canonical constructor for parser output because it preserves
            /// inline bullet boundaries and media references exactly as parsed.
            pub fn new(content: BulletContent) -> Self {
                Self {
                    content,
                    span: Span::DUMMY,
                }
            }

            /// Convenience constructor for plain text payloads.
            ///
            /// Use [`Self::new`] when callers already have parsed [`BulletContent`].
            /// This helper is mainly for tests and hand-built model values.
            pub fn from_text(text: impl Into<smol_str::SmolStr>) -> Self {
                Self {
                    content: BulletContent::from_text(text),
                    span: Span::DUMMY,
                }
            }

            /// Returns `true` when no serializable payload is present.
            ///
            /// Emptiness follows [`BulletContent::is_empty`], so a single empty text
            /// segment is treated the same as a missing payload.
            pub fn is_empty(&self) -> bool {
                self.content.is_empty()
            }

            /// Sets source span metadata used in diagnostics.
            ///
            /// Builders and tests can leave this as `Span::DUMMY`, but parser paths
            /// should attach real offsets for accurate user-facing errors.
            pub fn with_span(mut self, span: Span) -> Self {
                self.span = span;
                self
            }

            /// Allocating helper that writes `%tag:\t...` into a `String`.
            ///
            /// Prefer [`WriteChat::write_chat`] in streaming contexts to avoid
            /// intermediate allocation.
            pub fn to_chat(&self) -> String {
                let mut s = String::new();
                let _ = self.write_chat(&mut s);
                s
            }
        }

        impl WriteChat for $name {
            /// Serializes one full dependent-tier line for this tier type.
            ///
            /// The `%tag:\t` prefix is fixed per tier type, and payload emission is
            /// delegated to [`BulletContent`] to preserve inline timing markers.
            fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
                w.write_str(concat!("%", $prefix, ":\t"))?;
                self.content.write_chat(w)
            }
        }
    };
}

define_text_tier!(
    /// Comment tier (%com).
    ///
    /// Free-form comments about the utterance, situation, or transcript.
    /// Used for annotations, clarifications, and contextual notes.
    ///
    /// # CHAT Format Example
    ///
    /// ```text
    /// *CHI: I want cookie .
    /// %com: child is pointing at cookie jar
    /// ```
    ///
    /// # References
    ///
    /// - [Comment Tier](https://talkbank.org/0info/manuals/CHAT.html#Comment_Tier)
    ComTier, "com"
);

define_text_tier!(
    /// Explanation/expansion tier (%exp).
    ///
    /// Provides explanations or expansions of the utterance content.
    /// Often used to clarify ambiguous or incomplete speech.
    ///
    /// # CHAT Format Example
    ///
    /// ```text
    /// *CHI: xxx .
    /// %exp: child said something unintelligible about the toy
    /// ```
    ///
    /// # References
    ///
    /// - [Explanation Tier](https://talkbank.org/0info/manuals/CHAT.html#Explanation_Tier)
    ExpTier, "exp"
);

define_text_tier!(
    /// Addressee tier (%add).
    ///
    /// Specifies who the utterance is directed to.
    /// Useful in multi-party conversations.
    ///
    /// # CHAT Format Example
    ///
    /// ```text
    /// *MOT: come here !
    /// %add: CHI
    /// ```
    ///
    /// # References
    ///
    /// - [Addressee Tier](https://talkbank.org/0info/manuals/CHAT.html#Addressee_Tier)
    AddTier, "add"
);

define_text_tier!(
    /// Speech act tier (%spa).
    ///
    /// Codes the communicative function or speech act of the utterance
    /// (e.g., request, assertion, question, directive).
    ///
    /// # CHAT Format Example
    ///
    /// ```text
    /// *CHI: cookie ?
    /// %spa: $REQ:OBJ
    /// ```
    ///
    /// # References
    ///
    /// - [Speech Act Tier](https://talkbank.org/0info/manuals/CHAT.html#SpeechAct_Tier)
    SpaTier, "spa"
);

define_text_tier!(
    /// Situation tier (%sit).
    ///
    /// Describes the situational context or setting of the utterance.
    ///
    /// # CHAT Format Example
    ///
    /// ```text
    /// *MOT: let's eat .
    /// %sit: family sitting at dinner table
    /// ```
    ///
    /// # References
    ///
    /// - [Situation Tier](https://talkbank.org/0info/manuals/CHAT.html#Situation_Tier)
    SitTier, "sit"
);

define_text_tier!(
    /// Gesture position extended tier (%gpx).
    ///
    /// Extended gesture position coding for detailed gesture analysis.
    ///
    /// # CHAT Format Example
    ///
    /// ```text
    /// *CHI: that one .
    /// %gpx: pointing with right index finger
    /// ```
    ///
    /// # References
    ///
    /// - [Gesture Position Tier](https://talkbank.org/0info/manuals/CHAT.html#GesturePosition_Tier)
    GpxTier, "gpx"
);

define_text_tier!(
    /// Intonation tier (%int).
    ///
    /// Codes intonational contours and prosodic patterns.
    ///
    /// # CHAT Format Example
    ///
    /// ```text
    /// *CHI: really ?
    /// %int: high rising intonation
    /// ```
    ///
    /// # References
    ///
    /// - [Intonational Tier](https://talkbank.org/0info/manuals/CHAT.html#Intonational_Tier)
    IntTier, "int"
);

#[cfg(test)]
mod tests {
    use super::*;

    /// `%com` tier round-trips plain text payloads.
    ///
    /// This guards the macro-generated prefix and payload serializer wiring.
    #[test]
    fn test_com_tier() {
        let tier = ComTier::from_text("This is a comment");
        assert_eq!(tier.to_chat(), "%com:\tThis is a comment");
        assert!(!tier.is_empty());
    }

    /// `%exp` tier round-trips plain text payloads.
    ///
    /// This ensures explanations are emitted verbatim with the `%exp` prefix.
    #[test]
    fn test_exp_tier() {
        let tier = ExpTier::from_text("Explanation text");
        assert_eq!(tier.to_chat(), "%exp:\tExplanation text");
        assert!(!tier.is_empty());
    }

    /// `%add` tier round-trips plain text payloads.
    ///
    /// The test protects the shared macro path for addressee-tier formatting.
    #[test]
    fn test_add_tier() {
        let tier = AddTier::from_text("MOT");
        assert_eq!(tier.to_chat(), "%add:\tMOT");
        assert!(!tier.is_empty());
    }

    /// `%spa` tier round-trips plain text payloads.
    ///
    /// This confirms speech-act codes are preserved as plain text content.
    #[test]
    fn test_spa_tier() {
        let tier = SpaTier::from_text("DECL");
        assert_eq!(tier.to_chat(), "%spa:\tDECL");
        assert!(!tier.is_empty());
    }

    /// `%sit` tier round-trips plain text payloads.
    ///
    /// It verifies that contextual descriptions serialize without transformation.
    #[test]
    fn test_sit_tier() {
        let tier = SitTier::from_text("at the table");
        assert_eq!(tier.to_chat(), "%sit:\tat the table");
        assert!(!tier.is_empty());
    }

    /// Empty text payloads are treated as empty tier content.
    ///
    /// All macro-generated text tiers share this behavior via `BulletContent`.
    #[test]
    fn test_empty_tiers() {
        assert!(ComTier::from_text("").is_empty());
        assert!(ExpTier::from_text("").is_empty());
        assert!(AddTier::from_text("").is_empty());
        assert!(SpaTier::from_text("").is_empty());
        assert!(SitTier::from_text("").is_empty());
        assert!(GpxTier::from_text("").is_empty());
        assert!(IntTier::from_text("").is_empty());
    }

    /// `%gpx` tier round-trips plain text payloads.
    ///
    /// This keeps gesture-description tier formatting aligned with other macro-generated tiers.
    #[test]
    fn test_gpx_tier() {
        let tier = GpxTier::from_text("looks at chicken");
        assert_eq!(tier.to_chat(), "%gpx:\tlooks at chicken");
        assert!(!tier.is_empty());
    }

    /// `%int` tier round-trips plain text payloads.
    ///
    /// The test ensures prosodic notes are emitted verbatim with the `%int` prefix.
    #[test]
    fn test_int_tier() {
        let tier = IntTier::from_text("draws out the letter h");
        assert_eq!(tier.to_chat(), "%int:\tdraws out the letter h");
        assert!(!tier.is_empty());
    }
}
