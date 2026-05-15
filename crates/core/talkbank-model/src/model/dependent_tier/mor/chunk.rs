//! `%mor` chunk sequence — canonical expansion of tier items for `%gra` alignment.
//!
//! A "chunk" is the unit that aligns 1:1 with a `%gra` relation. `%gra`
//! relation indices (`1|2|SUBJ`, `2|1|AUX`, …) are 1-indexed positions into
//! this sequence, **not** into [`MorTier::items`]. Concretely, each item
//! contributes its main word, then each post-clitic (in serialized order),
//! and the optional tier terminator becomes one final chunk with no lemma.
//!
//! For the line
//!
//! ```text
//! *CHI: it's cookies .
//! %mor: pron|it~aux|be noun|cookie .
//! ```
//!
//! the chunk sequence is `[Main(it), PostClitic(be), Main(cookie), Terminator(.)]`.
//! Downstream crates (LSP, CLI, CLAN) MUST route any "what's at `%gra`
//! position N?" question through [`MorTier::chunk_at`] or [`MorTier::chunks`];
//! reconstructing the walk in a consumer silently drops post-clitics when an
//! item contains them. See `crates/chatter-lsp/CLAUDE.md` for the rule.
//!
//! CHAT reference anchors:
//! - [Morphological tier](https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier)
//! - [Grammatical relations tier](https://talkbank.org/0info/manuals/CHAT.html#GrammaticalRelations_Tier)

use super::item::Mor;
use super::word::MorWord;

/// Discriminant for the three kinds of `%mor` chunk.
///
/// Diagnostic and rendering code should match on this enum rather than
/// inspect string content to classify a chunk — the mapping to human labels
/// (`"post-clitic"`, `"terminator"`) is a presentation concern, not a
/// classification concern.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MorChunkKind {
    /// Main word of a `%mor` item (the head of any post-clitic group).
    Main,
    /// Post-clitic expansion inside an item, marked in CHAT with `~`.
    PostClitic,
    /// Tier terminator (`.`, `?`, `!`, or CHAT-specific terminator). Has no lemma.
    Terminator,
}

/// One chunk in a `%mor` tier's chunk sequence.
///
/// Borrows into the owning [`MorTier`] / [`Mor`] and is therefore cheap to
/// produce. Callers that need owned strings should `.to_string()` the
/// lemma at the edge — never `Clone` the variant to escape the borrow, since
/// the underlying `Mor` / `MorWord` are the canonical storage.
#[derive(Clone, Copy, Debug)]
pub enum MorChunk<'a> {
    /// Main word of a `%mor` item. The borrow is of the whole item so the
    /// caller can still reach sibling post-clitics or features if needed.
    Main(&'a Mor),
    /// A post-clitic expansion of the item in the first field; the specific
    /// post-clitic word is the second field.
    PostClitic(&'a Mor, &'a MorWord),
    /// Tier terminator (e.g. `Terminator::Period`). Typed, not stringly:
    /// the chunk carries the full categorical distinction between
    /// canonical (`.` `?` `!` `+...` etc.) and CA-mode (`→ ↗ ↘ ⇗ ⇘ ≋ ≈`)
    /// terminators.
    Terminator(&'a crate::model::content::Terminator),
}

impl<'a> MorChunk<'a> {
    /// Classifies this chunk.
    pub fn kind(&self) -> MorChunkKind {
        match self {
            MorChunk::Main(_) => MorChunkKind::Main,
            MorChunk::PostClitic(_, _) => MorChunkKind::PostClitic,
            MorChunk::Terminator(_) => MorChunkKind::Terminator,
        }
    }

    /// The morphological word carried by this chunk, or `None` for a terminator.
    ///
    /// For [`MorChunk::Main`] this returns the item's main word; for
    /// [`MorChunk::PostClitic`] it returns the post-clitic word.
    pub fn word(&self) -> Option<&'a MorWord> {
        match self {
            MorChunk::Main(item) => Some(&item.main),
            MorChunk::PostClitic(_, clitic) => Some(clitic),
            MorChunk::Terminator(_) => None,
        }
    }

    /// The lemma (base form) of this chunk's word.
    ///
    /// `None` for terminators, which carry only punctuation text. Use
    /// [`Self::terminator_text`] when a terminator's literal text is needed
    /// (e.g. for hover display).
    pub fn lemma(&self) -> Option<&'a str> {
        self.word().map(|w| w.lemma.as_str())
    }

    /// The host `%mor` item this chunk belongs to.
    ///
    /// Returns the same `&Mor` for a `Main` chunk and its sibling
    /// `PostClitic` chunks — useful when projecting a chunk back through the
    /// main↔mor alignment (which is keyed by `%mor` *item* index). `None`
    /// for a terminator because terminators are tier-level, not item-level.
    pub fn host_item(&self) -> Option<&'a Mor> {
        match self {
            MorChunk::Main(item) | MorChunk::PostClitic(item, _) => Some(*item),
            MorChunk::Terminator(_) => None,
        }
    }

    /// The typed terminator, or `None` for a word chunk.
    ///
    /// Complements [`Self::lemma`] for the terminator case.
    pub fn terminator(&self) -> Option<&'a crate::model::content::Terminator> {
        match self {
            MorChunk::Terminator(t) => Some(*t),
            _ => None,
        }
    }
}
