//! `%pho`/`%mod` tier model and container types.
//!
//! CHAT reference anchors:
//! - [Phonology tier](https://talkbank.org/0info/manuals/CHAT.html#Phonology)
//! - [Model phonology](https://talkbank.org/0info/manuals/CHAT.html#Model_Phonology)

use super::{PhoItem, PhoTierType, PhoWord, WriteChat};
use crate::Span;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt::Write as FmtWrite;
use std::ops::{Deref, DerefMut};
use talkbank_derive::{SemanticEq, SpanShift};

/// Phonological transcription tier (%pho, %mod, or %upho).
///
/// Contains phonetic transcription of how words are pronounced, using notation
/// systems like IPA (International Phonetic Alphabet) or UNIBET. Tokens align
/// 1-to-1 with alignable main tier content.
///
/// # Alignment
///
/// Phonological tokens align with main tier words:
/// - One token per word (excluding retraces, pauses, events)
/// - Terminator gets its own token (typically `.`)
/// - Token order matches main tier word order
///
/// # Notation Systems
///
/// Common phonetic notation systems:
/// - **IPA**: International Phonetic Alphabet (Unicode)
/// - **UNIBET**: ASCII-based phonetic notation
/// - **X-SAMPA**: Extended SAMPA ASCII notation
/// - **Custom**: Researcher-defined systems for %upho
///
/// # %pho vs %mod
///
/// - **%pho**: What was actually said (observed pronunciation)
/// - **%mod**: What should have been said (target/standard pronunciation)
///
/// Use %mod when the speaker's pronunciation differs from the standard form,
/// such as in child language acquisition or speech disorders.
///
/// # CHAT Manual Reference
///
/// - [Phonology Tier](https://talkbank.org/0info/manuals/CHAT.html#Phonology)
/// - [Model Phonology](https://talkbank.org/0info/manuals/CHAT.html#Model_Phonology)
///
/// # Examples
///
/// ```
/// use talkbank_model::model::{PhoItem, PhoTier, PhoTierType, PhoWord};
///
/// // Child says "fwi" for "three"
/// let pho = PhoTier::new_pho(vec![PhoItem::Word(PhoWord::new("fwi"))]);
/// let mod_tier = PhoTier::new_mod(vec![PhoItem::Word(PhoWord::new("θri"))]);
///
/// // Should align 1-to-1
/// assert_eq!(pho.len(), mod_tier.len());
/// ```
///
/// **CHAT format:**
/// ```text
/// *CHI: three .
/// %pho: fwi .
/// %mod: θri .
/// ```
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
pub struct PhoTier {
    /// Type of phonological tier.
    ///
    /// Determines whether this is %pho, %mod, or %upho.
    pub tier_type: PhoTierType,

    /// Phonological content items aligned with main tier.
    ///
    /// Each item (word or group) aligns with alignable main tier content.
    /// Item order matches main tier word order.
    pub items: PhoItems,

    /// Source span for error reporting (not serialized to JSON)
    #[serde(skip)]
    #[schemars(skip)]
    pub span: Span,
}

impl PhoTier {
    /// Constructs a phonology tier from parsed items and explicit tier type.
    pub fn new(tier_type: PhoTierType, items: Vec<PhoItem>) -> Self {
        Self {
            tier_type,
            items: items.into(),
            span: Span::DUMMY,
        }
    }

    /// Sets source span metadata used in diagnostics.
    pub fn with_span(mut self, span: Span) -> Self {
        self.span = span;
        self
    }

    /// Legacy convenience constructor from plain token text.
    ///
    /// Each token becomes `PhoItem::Word`. Prefer [`Self::new`] when callers
    /// already parsed groups or other structured `%pho/%mod` forms.
    pub fn from_tokens(tier_type: PhoTierType, tokens: Vec<String>) -> Self {
        let items: Vec<PhoItem> = tokens
            .into_iter()
            .map(|text| PhoItem::Word(PhoWord::new(text)))
            .collect();
        Self {
            tier_type,
            items: items.into(),
            span: Span::DUMMY,
        }
    }

    /// Construct `%pho` tier (observed pronunciation).
    pub fn new_pho(items: Vec<PhoItem>) -> Self {
        Self::new(PhoTierType::Pho, items)
    }

    /// Construct `%mod` tier (target/model pronunciation).
    pub fn new_mod(items: Vec<PhoItem>) -> Self {
        Self::new(PhoTierType::Mod, items)
    }

    /// Returns `true` if this tier serializes as `%pho`.
    pub fn is_pho(&self) -> bool {
        self.tier_type == PhoTierType::Pho
    }

    /// Returns `true` if this tier serializes as `%mod`.
    pub fn is_mod(&self) -> bool {
        self.tier_type == PhoTierType::Mod
    }

    /// Number of alignment slots represented in this tier.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Returns `true` when the tier carries no items.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Write tier content only (items), without the tier prefix (%pho:\t).
    ///
    /// This is used for roundtrip testing against golden data that contains
    /// content-only, and for the TreeSitterParser API which expects content-only input.
    pub fn write_content<W: FmtWrite>(&self, w: &mut W) -> std::fmt::Result {
        for (i, item) in self.items.iter().enumerate() {
            if i > 0 {
                w.write_char(' ')?;
            }
            item.write_chat(w)?;
        }
        Ok(())
    }

    /// Allocating helper that writes the full tier line into a `String`.
    pub fn to_chat(&self) -> String {
        use super::WriteChat;
        let mut s = String::new();
        let _ = self.write_chat(&mut s);
        s
    }

    /// Allocating helper that writes content-only payload into a `String`.
    pub fn to_content(&self) -> String {
        let mut s = String::new();
        let _ = self.write_content(&mut s);
        s
    }
}

impl WriteChat for PhoTier {
    /// Serializes one full `%pho`, `%mod`, or `%upho` tier line.
    fn write_chat<W: FmtWrite>(&self, w: &mut W) -> std::fmt::Result {
        // Write tier type prefix using PhoTierType's WriteChat impl
        self.tier_type.write_chat(w)?;
        w.write_str(":\t")?;

        // Write space-separated items
        for (i, item) in self.items.iter().enumerate() {
            if i > 0 {
                w.write_char(' ')?;
            }
            item.write_chat(w)?;
        }
        Ok(())
    }
}

/// Ordered phonological items for one `%pho`/`%mod` tier line.
///
/// # Reference
///
/// - [Phonology tier](https://talkbank.org/0info/manuals/CHAT.html#Phonology)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
#[serde(transparent)]
#[schemars(transparent)]
pub struct PhoItems(pub Vec<PhoItem>);

impl PhoItems {
    /// Wraps ordered `%pho/%mod` items without reinterpreting alignment.
    pub fn new(items: Vec<PhoItem>) -> Self {
        Self(items)
    }

    /// Returns `true` when this item list is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Deref for PhoItems {
    type Target = Vec<PhoItem>;

    /// Exposes the underlying items for read-only collection operations.
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for PhoItems {
    /// Exposes the underlying items for in-place mutation.
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<Vec<PhoItem>> for PhoItems {
    /// Wraps a raw item vector as `PhoItems`.
    fn from(items: Vec<PhoItem>) -> Self {
        Self(items)
    }
}

impl crate::validation::Validate for PhoItems {
    /// Placeholder for future tier-level item constraints beyond parser guarantees.
    fn validate(
        &self,
        _context: &crate::validation::ValidationContext,
        _errors: &impl crate::ErrorSink,
    ) {
    }
}
