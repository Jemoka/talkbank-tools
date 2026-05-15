//! One dependency edge record from a `%gra` tier.
//!
//! Each entry stores the dependent index, head index, and relation label in the
//! canonical `index|head|relation` triple format.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#GrammaticalRelations_Tier>
//! - <https://universaldependencies.org/>

use super::super::WriteChat;
use super::relation_type::GrammaticalRelationType;
use crate::alignment::indices::{GraHeadRef, SemanticWordIndex1, SemanticWordIndexError};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift};

/// Single grammatical relation in Universal Dependencies format.
///
/// Represents one edge in the dependency tree, specifying how a word relates to
/// its syntactic parent (head). Each relation encodes the word's position, its
/// parent's position, and the type of dependency relationship.
///
/// # Format
///
/// The CHAT format is: `index|head|relation`
///
/// Where:
/// - **index**: Position of the dependent word (1-indexed)
/// - **head**: Position of the parent word (0 = ROOT)
/// - **relation**: Universal Dependencies relation type
///
/// # Index Numbering
///
/// Indices are **1-indexed** (not 0-indexed):
/// - First word = 1
/// - Second word = 2
/// - Third word = 3
/// - ROOT (sentence head) = 0
///
/// # CHAT Format Examples
///
/// Subject relation:
/// ```text
/// 1|2|SUBJ
/// ```
/// Word 1 is the subject of word 2.
///
/// Root relation (head=0, virtual root node):
/// ```text
/// 2|0|ROOT
/// ```
/// Word 2 is the root of the sentence. Head=0 means it depends on the virtual root.
///
/// Object relation:
/// ```text
/// 3|2|OBJ
/// ```
/// Word 3 is the object of word 2.
///
/// Complete example:
/// ```text
/// *CHI: the dog barks .
/// %mor: det:art|the n|dog v|bark-3S .
/// %gra: 1|2|DET 2|3|SUBJ 3|0|ROOT 4|3|PUNCT
/// ```
///
/// Dependency tree visualization:
/// ```text
///        barks (3, ROOT)
///       /    |    \
///      /     |     \
///   dog(2) SUBJ   .(4)
///    |            PUNCT
///  the(1)
///  DET
/// ```
///
/// # References
///
/// - [Universal Dependencies](https://universaldependencies.org/)
/// - [CHAT Manual: Grammatical Relations](https://talkbank.org/0info/manuals/CHAT.html#GrammaticalRelations_Tier)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
pub struct GrammaticalRelation {
    /// Index of this word in %mor tier chunks (1-indexed)
    ///
    /// Note: This is 1-indexed (1, 2, 3, ...), not 0-indexed!
    /// Index 0 is reserved for ROOT in the head field.
    pub index: usize,

    /// Head index (index of parent word, 0 = ROOT)
    ///
    /// - 0 means this word is the root of the sentence
    /// - Otherwise, points to the index of the parent word
    pub head: usize,

    /// Relation type (e.g., "SUBJ", "OBJ", "ROOT", "DET", "PUNCT")
    ///
    /// Common Universal Dependencies relations:
    /// - ROOT: root of sentence
    /// - SUBJ: subject
    /// - OBJ: object
    /// - DET: determiner
    /// - PUNCT: punctuation
    /// - MOD: modifier
    pub relation: GrammaticalRelationType,
}

impl GrammaticalRelation {
    /// Creates a new grammatical relation (index|head|relation).
    ///
    /// This constructor performs no structural validation on index sequences or
    /// tree shape. Those checks are handled by `%gra` tier validators where the
    /// full relation list is available.
    pub fn new(index: usize, head: usize, relation: impl Into<GrammaticalRelationType>) -> Self {
        Self {
            index,
            head,
            relation: relation.into(),
        }
    }

    /// Return whether this relation should be treated as sentence root.
    ///
    /// A relation is considered a root if ANY of these conditions hold:
    /// 1. **Self-referential**: head == index (structural root, any label)
    /// 2. **ROOT label**: Relation labeled "ROOT" (standard UD)
    /// 3. **INCROOT label**: Relation labeled "INCROOT" (incomplete utterances)
    ///
    /// ## Rationale
    ///
    /// Real corpus data shows diverse root conventions:
    /// - Standard UD: `2|0|ROOT` (head=0, labeled ROOT)
    /// - TalkBank variant: `2|2|ROOT` (head=self, labeled ROOT)
    /// - Structural root: `6|6|FLAT` (head=self, any label)
    ///
    /// We accept all patterns. Self-referential structure (head == index) is
    /// the most reliable indicator of root status.
    ///
    /// This intentionally favors compatibility with observed corpora over strict
    /// theoretical constraints at construction time.
    pub fn is_root(&self) -> bool {
        // Structural root: head points to self
        self.head == self.index
            // OR labeled as ROOT/INCROOT (even if head != self)
            || matches!(self.relation.as_str(), "ROOT" | "INCROOT")
    }

    /// Classify the `head` field as either ROOT (authored as `0`) or a
    /// reference to another word position.
    ///
    /// Prefer this accessor over raw `head == 0` comparisons: the enum
    /// return forces callers to handle both cases explicitly, which
    /// matches the "avoid boolean blindness" rule and makes hover /
    /// graph-edge code self-documenting.
    pub fn head_ref(&self) -> GraHeadRef {
        GraHeadRef::from_raw(self.head)
    }

    /// Return the dependent's position as a [`SemanticWordIndex1`].
    ///
    /// Returns an error if `index == 0`, which is not a valid dependent
    /// position (0 is reserved for the ROOT marker on `head`). Callers
    /// should only hit the error path when the parser accepted a
    /// malformed `%gra` triple such as `0|0|ROOT`.
    pub fn index_as_semantic(&self) -> Result<SemanticWordIndex1, SemanticWordIndexError> {
        SemanticWordIndex1::new(self.index)
    }
}

impl WriteChat for GrammaticalRelation {
    /// Write to CHAT format: `index|head|relation`
    ///
    /// Serialization is intentionally lossless and does not normalize labels or
    /// rebase indices.
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        write!(w, "{}|{}|{}", self.index, self.head, self.relation)
    }
}

impl std::fmt::Display for GrammaticalRelation {
    /// Formats this dependency triple as `index|head|relation`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.write_chat(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Constructor stores index, head, and relation as provided.
    ///
    /// This guards the basic data contract before tree-level validation.
    #[test]
    fn test_grammatical_relation_new() {
        let rel = GrammaticalRelation::new(1, 2, "SUBJ");
        assert_eq!(rel.index, 1);
        assert_eq!(rel.head, 2);
        assert_eq!(rel.relation, "SUBJ".into());
        assert!(!rel.is_root());
    }

    /// Canonical `ROOT` label is recognized as root status.
    ///
    /// The test keeps UD-style root behavior explicit.
    #[test]
    fn test_grammatical_relation_root() {
        let rel = GrammaticalRelation::new(2, 0, "ROOT");
        assert_eq!(rel.index, 2);
        assert_eq!(rel.head, 0);
        assert_eq!(rel.relation, "ROOT".into());
        assert!(rel.is_root());
    }

    /// `INCROOT` is treated as a root marker for incomplete utterances.
    ///
    /// This matches observed corpus conventions.
    #[test]
    fn test_grammatical_relation_incroot() {
        let rel = GrammaticalRelation::new(1, 0, "INCROOT");
        assert_eq!(rel.index, 1);
        assert_eq!(rel.head, 0);
        assert_eq!(rel.relation, "INCROOT".into());
        assert!(rel.is_root()); // INCROOT should be recognized as a valid root
    }

    /// Self-referential edges are treated as structural roots.
    ///
    /// This preserves compatibility with non-standard but common corpus data.
    #[test]
    fn test_grammatical_relation_structural_root() {
        // Self-referential relation with non-ROOT label (e.g., FLAT)
        let rel = GrammaticalRelation::new(6, 6, "FLAT");
        assert_eq!(rel.index, 6);
        assert_eq!(rel.head, 6);
        assert_eq!(rel.relation, "FLAT".into());
        assert!(rel.is_root()); // Self-referential = structural root
    }

    /// Non-self, non-root-labeled edges must not be treated as roots.
    ///
    /// This keeps dependent edges distinct from sentence roots.
    #[test]
    fn test_grammatical_relation_not_root() {
        let rel = GrammaticalRelation::new(1, 2, "SUBJ");
        assert!(!rel.is_root()); // Neither self-referential nor ROOT/INCROOT
    }

    /// `%gra` relation display uses `index|head|relation` form.
    ///
    /// The test covers the shared `WriteChat` path used by tier serialization.
    #[test]
    fn test_grammatical_relation_chat_format() {
        use crate::model::WriteChat;
        let rel = GrammaticalRelation::new(3, 2, "OBJ");
        assert_eq!(rel.to_chat_string(), "3|2|OBJ");
    }
}
