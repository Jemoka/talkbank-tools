//! Auto-generated CST traversal with typed `NodeSlot` fields.
//!
//! Every child position is a `NodeSlot<TypedNode>` — the faithful
//! representation of Present / Missing / Error / Unexpected / Absent.
//!
//! **Payload structs** contain only the semantically meaningful
//! children — structural delimiters are stripped. Use these for
//! semantic processing.
//!
//! Implement `GrammarTraversal` and override extraction methods
//! where you need custom recovery.
//!
//! This file is standalone — it has no dependencies beyond `tree_sitter`.
#![allow(
    clippy::cast_possible_truncation,
    clippy::collapsible_if,
    clippy::module_name_repetitions,
    clippy::needless_pass_by_value,
    clippy::similar_names,
    clippy::too_many_lines,
    clippy::unused_self,
    dead_code,
    unused_assignments,
    unused_imports
)]
/// What exists at a child position in the CST.
///
/// `T` is a typed node wrapper (e.g., `SpeakerNode`) that guarantees the
/// node kind matches the grammar expectation. `Present` and `Missing` both
/// carry a `T` because MISSING nodes have the expected `kind()` — the
/// type is correct, only the content is absent.
#[derive(Debug, Clone)]
pub enum NodeSlot<'tree, T> {
    /// A valid node with the expected kind and content.
    Present(T),
    /// A tree-sitter MISSING placeholder (kind correct, zero-length span).
    Missing(T),
    /// A tree-sitter ERROR node (unparseable region).
    Error(tree_sitter::Node<'tree>),
    /// A node with an unexpected kind at this position.
    Unexpected(tree_sitter::Node<'tree>),
    /// No node exists at this position (child list too short).
    Absent,
}
impl<T> NodeSlot<'_, T> {
    /// Get the typed node if present (not MISSING, not error).
    #[must_use]
    pub const fn ok(&self) -> Option<&T> {
        match self {
            Self::Present(n) => Some(n),
            _ => None,
        }
    }
    /// Get the typed node if present, consuming the slot.
    #[must_use]
    pub fn into_ok(self) -> Option<T> {
        match self {
            Self::Present(n) => Some(n),
            _ => None,
        }
    }
    /// Get the typed node if present OR missing (kind is correct either way).
    #[must_use]
    pub const fn typed(&self) -> Option<&T> {
        match self {
            Self::Present(n) | Self::Missing(n) => Some(n),
            _ => None,
        }
    }
    /// Whether a valid, non-MISSING node is present.
    #[must_use]
    pub const fn is_present(&self) -> bool {
        matches!(self, Self::Present(_))
    }
    /// Whether this is a MISSING placeholder.
    #[must_use]
    pub const fn is_missing(&self) -> bool {
        matches!(self, Self::Missing(_))
    }
    /// Whether this is an ERROR node.
    #[must_use]
    pub const fn is_error(&self) -> bool {
        matches!(self, Self::Error(_))
    }
    /// Whether this is an unexpected node kind.
    #[must_use]
    pub const fn is_unexpected(&self) -> bool {
        matches!(self, Self::Unexpected(_))
    }
    /// Whether no node exists at this position.
    #[must_use]
    pub const fn is_absent(&self) -> bool {
        matches!(self, Self::Absent)
    }
    /// Whether this slot has any problem (not `Present`).
    #[must_use]
    pub const fn has_error(&self) -> bool {
        !self.is_present()
    }
}
impl<'tree, T: AsRawNode<'tree>> NodeSlot<'tree, T> {
    /// Get the raw tree-sitter node, regardless of slot state.
    /// Returns `None` only for `Absent`.
    #[must_use]
    pub fn raw_node(&self) -> Option<tree_sitter::Node<'tree>> {
        match self {
            Self::Present(n) | Self::Missing(n) => Some(n.raw_node()),
            Self::Error(n) | Self::Unexpected(n) => Some(*n),
            Self::Absent => None,
        }
    }
}
/// Trait for typed node wrappers to expose the underlying `tree_sitter::Node`.
pub trait AsRawNode<'tree> {
    /// Get the underlying tree-sitter node.
    fn raw_node(&self) -> tree_sitter::Node<'tree>;
}
/// Classify a tree-sitter node at a position where `expected_kind` was expected.
#[must_use]
fn classify_child<'tree, T, F>(
    node: tree_sitter::Node<'tree>,
    expected_kind: &str,
    wrap: F,
) -> NodeSlot<'tree, T>
where
    F: FnOnce(tree_sitter::Node<'tree>) -> T,
{
    if node.is_error() {
        NodeSlot::Error(node)
    } else if node.is_missing() {
        NodeSlot::Missing(wrap(node))
    } else if node.kind() == expected_kind {
        NodeSlot::Present(wrap(node))
    } else {
        NodeSlot::Unexpected(node)
    }
}
///Typed wrapper for `_id_demographic_fields` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct IdDemographicFieldsNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for IdDemographicFieldsNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl IdDemographicFieldsNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `_id_identity_fields` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct IdIdentityFieldsNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for IdIdentityFieldsNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl IdIdentityFieldsNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `_id_role_fields` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct IdRoleFieldsNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for IdRoleFieldsNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl IdRoleFieldsNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `act_tier_prefix` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct ActTierPrefixNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for ActTierPrefixNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl ActTierPrefixNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `activities_prefix` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct ActivitiesPrefixNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for ActivitiesPrefixNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl ActivitiesPrefixNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `add_tier_prefix` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct AddTierPrefixNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for AddTierPrefixNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl AddTierPrefixNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `alt_tier_prefix` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct AltTierPrefixNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for AltTierPrefixNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl AltTierPrefixNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `ampersand` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct AmpersandNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for AmpersandNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl AmpersandNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `annotation_content` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct AnnotationContentNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for AnnotationContentNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl AnnotationContentNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `base_annotations` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct BaseAnnotationsNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for BaseAnnotationsNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl BaseAnnotationsNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `bck_prefix` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct BckPrefixNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for BckPrefixNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl BckPrefixNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `begin_header` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct BeginHeaderNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for BeginHeaderNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl BeginHeaderNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `bg_prefix` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct BgPrefixNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for BgPrefixNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl BgPrefixNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `birth_of_prefix` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct BirthOfPrefixNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for BirthOfPrefixNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl BirthOfPrefixNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `birthplace_of_prefix` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct BirthplaceOfPrefixNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for BirthplaceOfPrefixNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl BirthplaceOfPrefixNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `blank_prefix` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct BlankPrefixNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for BlankPrefixNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl BlankPrefixNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `bullet_end` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct BulletEndNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for BulletEndNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl BulletEndNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `bullet_start` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct BulletStartNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for BulletStartNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl BulletStartNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `bullet_timestamp` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct BulletTimestampNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for BulletTimestampNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl BulletTimestampNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `cod_tier_prefix` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct CodTierPrefixNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for CodTierPrefixNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl CodTierPrefixNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `coh_tier_prefix` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct CohTierPrefixNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for CohTierPrefixNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl CohTierPrefixNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `colon` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct ColonNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for ColonNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl ColonNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `color_words_prefix` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct ColorWordsPrefixNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for ColorWordsPrefixNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl ColorWordsPrefixNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `com_tier_prefix` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct ComTierPrefixNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for ComTierPrefixNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl ComTierPrefixNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `comma` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct CommaNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for CommaNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl CommaNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `comment_prefix` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct CommentPrefixNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for CommentPrefixNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl CommentPrefixNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `contents` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct ContentsNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for ContentsNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl ContentsNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `date_contents` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct DateContentsNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for DateContentsNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl DateContentsNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `date_prefix` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct DatePrefixNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for DatePrefixNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl DatePrefixNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `def_tier_prefix` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct DefTierPrefixNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for DefTierPrefixNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl DefTierPrefixNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `eg_prefix` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct EgPrefixNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for EgPrefixNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl EgPrefixNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `end_header` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct EndHeaderNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for EndHeaderNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl EndHeaderNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `eng_tier_prefix` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct EngTierPrefixNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for EngTierPrefixNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl EngTierPrefixNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `err_tier_prefix` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct ErrTierPrefixNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for ErrTierPrefixNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl ErrTierPrefixNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `event_marker` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct EventMarkerNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for EventMarkerNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl EventMarkerNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `event_segment` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct EventSegmentNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for EventSegmentNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl EventSegmentNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `exp_tier_prefix` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct ExpTierPrefixNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for ExpTierPrefixNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl ExpTierPrefixNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `fac_tier_prefix` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct FacTierPrefixNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for FacTierPrefixNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl FacTierPrefixNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `final_codes` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct FinalCodesNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for FinalCodesNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl FinalCodesNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `flo_tier_prefix` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct FloTierPrefixNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for FloTierPrefixNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl FloTierPrefixNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `font_prefix` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct FontPrefixNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for FontPrefixNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl FontPrefixNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `form_marker` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct FormMarkerNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for FormMarkerNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl FormMarkerNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `free_text` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct FreeTextNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for FreeTextNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl FreeTextNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `g_prefix` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct GPrefixNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for GPrefixNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl GPrefixNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `gls_tier_prefix` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct GlsTierPrefixNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for GlsTierPrefixNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl GlsTierPrefixNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `gpx_tier_prefix` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct GpxTierPrefixNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for GpxTierPrefixNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl GpxTierPrefixNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `gra_contents` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct GraContentsNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for GraContentsNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl GraContentsNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `gra_head` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct GraHeadNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for GraHeadNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl GraHeadNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `gra_index` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct GraIndexNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for GraIndexNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl GraIndexNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `gra_relation` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct GraRelationNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for GraRelationNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl GraRelationNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `gra_relation_name` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct GraRelationNameNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for GraRelationNameNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl GraRelationNameNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `gra_tier_prefix` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct GraTierPrefixNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for GraTierPrefixNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl GraTierPrefixNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `greater_than` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct GreaterThanNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for GreaterThanNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl GreaterThanNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `header_gap` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct HeaderGapNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for HeaderGapNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl HeaderGapNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `header_sep` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct HeaderSepNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for HeaderSepNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl HeaderSepNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `hyphen` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct HyphenNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for HyphenNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl HyphenNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `id_age` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct IdAgeNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for IdAgeNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl IdAgeNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `id_contents` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct IdContentsNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for IdContentsNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl IdContentsNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `id_corpus` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct IdCorpusNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for IdCorpusNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl IdCorpusNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `id_custom_field` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct IdCustomFieldNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for IdCustomFieldNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl IdCustomFieldNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `id_education` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct IdEducationNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for IdEducationNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl IdEducationNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `id_group` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct IdGroupNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for IdGroupNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl IdGroupNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `id_languages` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct IdLanguagesNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for IdLanguagesNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl IdLanguagesNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `id_prefix` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct IdPrefixNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for IdPrefixNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl IdPrefixNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `id_role` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct IdRoleNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for IdRoleNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl IdRoleNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `id_ses` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct IdSesNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for IdSesNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl IdSesNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `id_sex` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct IdSexNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for IdSexNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl IdSexNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `id_speaker` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct IdSpeakerNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for IdSpeakerNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl IdSpeakerNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `int_tier_prefix` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct IntTierPrefixNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for IntTierPrefixNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl IntTierPrefixNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `l1_of_prefix` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct L1OfPrefixNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for L1OfPrefixNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl L1OfPrefixNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `language_code` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct LanguageCodeNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for LanguageCodeNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl LanguageCodeNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `languages_contents` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct LanguagesContentsNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for LanguagesContentsNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl LanguagesContentsNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `languages_prefix` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct LanguagesPrefixNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for LanguagesPrefixNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl LanguagesPrefixNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `left_bracket` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct LeftBracketNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for LeftBracketNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl LeftBracketNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `left_double_quote` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct LeftDoubleQuoteNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for LeftDoubleQuoteNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl LeftDoubleQuoteNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `less_than` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct LessThanNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for LessThanNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl LessThanNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `linkers` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct LinkersNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for LinkersNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl LinkersNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `location_prefix` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct LocationPrefixNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for LocationPrefixNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl LocationPrefixNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `long_feature_begin_marker` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct LongFeatureBeginMarkerNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for LongFeatureBeginMarkerNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl LongFeatureBeginMarkerNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `long_feature_end_marker` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct LongFeatureEndMarkerNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for LongFeatureEndMarkerNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl LongFeatureEndMarkerNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `long_feature_label` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct LongFeatureLabelNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for LongFeatureLabelNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl LongFeatureLabelNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `main_tier` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct MainTierNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for MainTierNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl MainTierNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `media_contents` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct MediaContentsNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for MediaContentsNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl MediaContentsNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `media_filename` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct MediaFilenameNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for MediaFilenameNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl MediaFilenameNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `media_prefix` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct MediaPrefixNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for MediaPrefixNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl MediaPrefixNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `media_type` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct MediaTypeNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for MediaTypeNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl MediaTypeNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `mod_tier_prefix` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct ModTierPrefixNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for ModTierPrefixNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl ModTierPrefixNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `modsyl_tier_prefix` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct ModsylTierPrefixNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for ModsylTierPrefixNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl ModsylTierPrefixNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `mor_contents` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct MorContentsNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for MorContentsNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl MorContentsNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `mor_feature_value` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct MorFeatureValueNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for MorFeatureValueNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl MorFeatureValueNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `mor_lemma` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct MorLemmaNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for MorLemmaNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl MorLemmaNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `mor_pos` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct MorPosNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for MorPosNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl MorPosNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `mor_tier_prefix` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct MorTierPrefixNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for MorTierPrefixNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl MorTierPrefixNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `mor_word` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct MorWordNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for MorWordNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl MorWordNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `new_episode_prefix` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct NewEpisodePrefixNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for NewEpisodePrefixNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl NewEpisodePrefixNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `newline` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct NewlineNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for NewlineNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl NewlineNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `nonvocal_begin_marker` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct NonvocalBeginMarkerNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for NonvocalBeginMarkerNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl NonvocalBeginMarkerNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `nonvocal_end_marker` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct NonvocalEndMarkerNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for NonvocalEndMarkerNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl NonvocalEndMarkerNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `nonword` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct NonwordNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for NonwordNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl NonwordNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `number_option` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct NumberOptionNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for NumberOptionNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl NumberOptionNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `number_prefix` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct NumberPrefixNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for NumberPrefixNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl NumberPrefixNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `option_name` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct OptionNameNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for OptionNameNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl OptionNameNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `options_contents` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct OptionsContentsNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for OptionsContentsNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl OptionsContentsNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `options_prefix` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct OptionsPrefixNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for OptionsPrefixNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl OptionsPrefixNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `ort_tier_prefix` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct OrtTierPrefixNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for OrtTierPrefixNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl OrtTierPrefixNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `page_number` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct PageNumberNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for PageNumberNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl PageNumberNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `page_prefix` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct PagePrefixNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for PagePrefixNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl PagePrefixNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `par_tier_prefix` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct ParTierPrefixNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for ParTierPrefixNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl ParTierPrefixNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `participant` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct ParticipantNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for ParticipantNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl ParticipantNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `participants_contents` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct ParticipantsContentsNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for ParticipantsContentsNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl ParticipantsContentsNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `participants_prefix` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct ParticipantsPrefixNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for ParticipantsPrefixNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl ParticipantsPrefixNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `pho_begin_group` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct PhoBeginGroupNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for PhoBeginGroupNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl PhoBeginGroupNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `pho_end_group` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct PhoEndGroupNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for PhoEndGroupNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl PhoEndGroupNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `pho_group` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct PhoGroupNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for PhoGroupNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl PhoGroupNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `pho_groups` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct PhoGroupsNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for PhoGroupsNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl PhoGroupsNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `pho_tier_prefix` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct PhoTierPrefixNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for PhoTierPrefixNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl PhoTierPrefixNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `pho_word` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct PhoWordNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for PhoWordNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl PhoWordNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `pho_words` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct PhoWordsNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for PhoWordsNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl PhoWordsNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `phoaln_tier_prefix` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct PhoalnTierPrefixNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for PhoalnTierPrefixNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl PhoalnTierPrefixNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `phosyl_tier_prefix` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct PhosylTierPrefixNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for PhosylTierPrefixNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl PhosylTierPrefixNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `pid_prefix` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct PidPrefixNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for PidPrefixNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl PidPrefixNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `pipe` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct PipeNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for PipeNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl PipeNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `pos_tag` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct PosTagNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for PosTagNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl PosTagNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `recording_quality_option` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct RecordingQualityOptionNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for RecordingQualityOptionNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl RecordingQualityOptionNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `recording_quality_prefix` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct RecordingQualityPrefixNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for RecordingQualityPrefixNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl RecordingQualityPrefixNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `rest_of_line` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct RestOfLineNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for RestOfLineNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl RestOfLineNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `right_brace` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct RightBraceNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for RightBraceNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl RightBraceNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `right_bracket` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct RightBracketNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for RightBracketNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl RightBracketNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `right_double_quote` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct RightDoubleQuoteNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for RightDoubleQuoteNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl RightDoubleQuoteNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `room_layout_prefix` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct RoomLayoutPrefixNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for RoomLayoutPrefixNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl RoomLayoutPrefixNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `sin_begin_group` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct SinBeginGroupNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for SinBeginGroupNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl SinBeginGroupNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `sin_end_group` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct SinEndGroupNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for SinEndGroupNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl SinEndGroupNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `sin_group` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct SinGroupNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for SinGroupNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl SinGroupNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `sin_groups` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct SinGroupsNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for SinGroupsNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl SinGroupsNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `sin_tier_prefix` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct SinTierPrefixNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for SinTierPrefixNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl SinTierPrefixNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `sin_word` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct SinWordNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for SinWordNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl SinWordNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `sit_tier_prefix` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct SitTierPrefixNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for SitTierPrefixNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl SitTierPrefixNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `situation_prefix` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct SituationPrefixNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for SituationPrefixNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl SituationPrefixNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `spa_tier_prefix` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct SpaTierPrefixNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for SpaTierPrefixNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl SpaTierPrefixNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `space` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct SpaceNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for SpaceNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl SpaceNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `speaker` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct SpeakerNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for SpeakerNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl SpeakerNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `standalone_word` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct StandaloneWordNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for StandaloneWordNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl StandaloneWordNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `star` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct StarNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for StarNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl StarNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `t_prefix` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct TPrefixNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for TPrefixNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl TPrefixNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `tab` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct TabNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for TabNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl TabNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `tape_location_prefix` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct TapeLocationPrefixNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for TapeLocationPrefixNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl TapeLocationPrefixNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `terminator` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct TerminatorNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for TerminatorNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl TerminatorNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `text_with_bullets` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct TextWithBulletsNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for TextWithBulletsNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl TextWithBulletsNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `text_with_bullets_and_pics` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct TextWithBulletsAndPicsNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for TextWithBulletsAndPicsNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl TextWithBulletsAndPicsNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `thumbnail_prefix` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct ThumbnailPrefixNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for ThumbnailPrefixNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl ThumbnailPrefixNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `tier_body` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct TierBodyNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for TierBodyNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl TierBodyNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `tier_sep` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct TierSepNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for TierSepNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl TierSepNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `tilde` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct TildeNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for TildeNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl TildeNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `tim_tier_prefix` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct TimTierPrefixNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for TimTierPrefixNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl TimTierPrefixNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `time_duration_contents` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct TimeDurationContentsNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for TimeDurationContentsNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl TimeDurationContentsNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `time_duration_prefix` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct TimeDurationPrefixNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for TimeDurationPrefixNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl TimeDurationPrefixNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `time_start_prefix` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct TimeStartPrefixNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for TimeStartPrefixNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl TimeStartPrefixNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `transcriber_prefix` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct TranscriberPrefixNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for TranscriberPrefixNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl TranscriberPrefixNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `transcription_option` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct TranscriptionOptionNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for TranscriptionOptionNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl TranscriptionOptionNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `transcription_prefix` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct TranscriptionPrefixNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for TranscriptionPrefixNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl TranscriptionPrefixNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `types_activity` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct TypesActivityNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for TypesActivityNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl TypesActivityNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `types_design` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct TypesDesignNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for TypesDesignNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl TypesDesignNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `types_group` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct TypesGroupNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for TypesGroupNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl TypesGroupNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `types_prefix` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct TypesPrefixNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for TypesPrefixNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl TypesPrefixNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `utf8_header` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct Utf8HeaderNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for Utf8HeaderNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl Utf8HeaderNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `utterance_end` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct UtteranceEndNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for UtteranceEndNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl UtteranceEndNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `videos_prefix` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct VideosPrefixNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for VideosPrefixNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl VideosPrefixNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `warning_prefix` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct WarningPrefixNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for WarningPrefixNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl WarningPrefixNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `whitespaces` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct WhitespacesNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for WhitespacesNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl WhitespacesNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `window_prefix` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct WindowPrefixNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for WindowPrefixNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl WindowPrefixNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `wor_tier_body` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct WorTierBodyNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for WorTierBodyNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl WorTierBodyNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `wor_tier_prefix` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct WorTierPrefixNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for WorTierPrefixNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl WorTierPrefixNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `word_body` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct WordBodyNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for WordBodyNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl WordBodyNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `word_lang_suffix` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct WordLangSuffixNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for WordLangSuffixNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl WordLangSuffixNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Typed wrapper for `word_segment` nodes. Kind is verified at construction.
#[derive(Debug, Clone, Copy)]
pub struct WordSegmentNode<'tree>(pub tree_sitter::Node<'tree>);
impl<'tree> AsRawNode<'tree> for WordSegmentNode<'tree> {
    fn raw_node(&self) -> tree_sitter::Node<'tree> {
        self.0
    }
}
impl WordSegmentNode<'_> {
    /// The source text of this node.
    #[must_use]
    pub fn text<'s>(&self, source: &'s str) -> &'s str {
        self.0.utf8_text(source.as_bytes()).unwrap_or("")
    }
    /// The byte range of this node in the source.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.0.start_byte()..self.0.end_byte()
    }
    /// Whether this is a MISSING placeholder node.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        self.0.is_missing()
    }
}
///Extracted children for `_id_demographic_fields` nodes.
#[derive(Debug)]
pub struct IdDemographicFieldsChildren<'tree> {
    ///`whitespaces` — optional
    pub child_0: Option<WhitespacesNode<'tree>>,
    ///`id_age` — optional
    pub child_1: Option<IdAgeNode<'tree>>,
    ///`whitespaces` — optional
    pub child_2: Option<WhitespacesNode<'tree>>,
    ///`pipe` — required
    pub child_3: NodeSlot<'tree, PipeNode<'tree>>,
    ///`whitespaces` — optional
    pub child_4: Option<WhitespacesNode<'tree>>,
    ///`id_sex` — optional
    pub child_5: Option<IdSexNode<'tree>>,
    ///`whitespaces` — optional
    pub child_6: Option<WhitespacesNode<'tree>>,
    ///`pipe` — required
    pub child_7: NodeSlot<'tree, PipeNode<'tree>>,
    ///`whitespaces` — optional
    pub child_8: Option<WhitespacesNode<'tree>>,
    ///`id_group` — optional
    pub child_9: Option<IdGroupNode<'tree>>,
    ///`whitespaces` — optional
    pub child_10: Option<WhitespacesNode<'tree>>,
    ///`pipe` — required
    pub child_11: NodeSlot<'tree, PipeNode<'tree>>,
    ///`whitespaces` — optional
    pub child_12: Option<WhitespacesNode<'tree>>,
    ///`id_ses` — optional
    pub child_13: Option<IdSesNode<'tree>>,
    ///`whitespaces` — optional
    pub child_14: Option<WhitespacesNode<'tree>>,
    ///`pipe` — required
    pub child_15: NodeSlot<'tree, PipeNode<'tree>>,
}
///Extracted children for `_id_identity_fields` nodes.
#[derive(Debug)]
pub struct IdIdentityFieldsChildren<'tree> {
    ///`id_languages` — required
    pub child_0: NodeSlot<'tree, IdLanguagesNode<'tree>>,
    ///`pipe` — required
    pub child_1: NodeSlot<'tree, PipeNode<'tree>>,
    ///`whitespaces` — optional
    pub child_2: Option<WhitespacesNode<'tree>>,
    ///`id_corpus` — optional
    pub child_3: Option<IdCorpusNode<'tree>>,
    ///`whitespaces` — optional
    pub child_4: Option<WhitespacesNode<'tree>>,
    ///`pipe` — required
    pub child_5: NodeSlot<'tree, PipeNode<'tree>>,
    ///`id_speaker` — required
    pub child_6: NodeSlot<'tree, IdSpeakerNode<'tree>>,
    ///`pipe` — required
    pub child_7: NodeSlot<'tree, PipeNode<'tree>>,
}
///Extracted children for `_id_role_fields` nodes.
#[derive(Debug)]
pub struct IdRoleFieldsChildren<'tree> {
    ///`id_role` — required
    pub child_0: NodeSlot<'tree, IdRoleNode<'tree>>,
    ///`pipe` — required
    pub child_1: NodeSlot<'tree, PipeNode<'tree>>,
    ///`whitespaces` — optional
    pub child_2: Option<WhitespacesNode<'tree>>,
    ///`id_education` — optional
    pub child_3: Option<IdEducationNode<'tree>>,
    ///`whitespaces` — optional
    pub child_4: Option<WhitespacesNode<'tree>>,
    ///`pipe` — required
    pub child_5: NodeSlot<'tree, PipeNode<'tree>>,
    ///`whitespaces` — optional
    pub child_6: Option<WhitespacesNode<'tree>>,
    ///`id_custom_field` — optional
    pub child_7: Option<IdCustomFieldNode<'tree>>,
    ///`whitespaces` — optional
    pub child_8: Option<WhitespacesNode<'tree>>,
    ///`pipe` — required
    pub child_9: NodeSlot<'tree, PipeNode<'tree>>,
}
///Extracted children for `act_dependent_tier` nodes.
#[derive(Debug)]
pub struct ActDependentTierChildren<'tree> {
    ///`act_tier_prefix` — required
    pub child_0: NodeSlot<'tree, ActTierPrefixNode<'tree>>,
    ///`tier_sep` — required
    pub child_1: NodeSlot<'tree, TierSepNode<'tree>>,
    ///`text_with_bullets` — required
    pub child_2: NodeSlot<'tree, TextWithBulletsNode<'tree>>,
    ///`newline` — required
    pub child_3: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `activities_header` nodes.
#[derive(Debug)]
pub struct ActivitiesHeaderChildren<'tree> {
    ///`activities_prefix` — required
    pub child_0: NodeSlot<'tree, ActivitiesPrefixNode<'tree>>,
    ///`header_sep` — required
    pub child_1: NodeSlot<'tree, HeaderSepNode<'tree>>,
    ///`free_text` — required
    pub child_2: NodeSlot<'tree, FreeTextNode<'tree>>,
    ///`newline` — required
    pub child_3: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `add_dependent_tier` nodes.
#[derive(Debug)]
pub struct AddDependentTierChildren<'tree> {
    ///`add_tier_prefix` — required
    pub child_0: NodeSlot<'tree, AddTierPrefixNode<'tree>>,
    ///`tier_sep` — required
    pub child_1: NodeSlot<'tree, TierSepNode<'tree>>,
    ///`text_with_bullets` — required
    pub child_2: NodeSlot<'tree, TextWithBulletsNode<'tree>>,
    ///`newline` — required
    pub child_3: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `alt_annotation` nodes.
#[derive(Debug)]
pub struct AltAnnotationChildren<'tree> {
    ///`space` — required
    pub child_1: NodeSlot<'tree, SpaceNode<'tree>>,
    ///`annotation_content` — required (field: `text`)
    pub text: NodeSlot<'tree, AnnotationContentNode<'tree>>,
    ///`right_bracket` — required
    pub child_3: NodeSlot<'tree, RightBracketNode<'tree>>,
}
///Extracted children for `alt_dependent_tier` nodes.
#[derive(Debug)]
pub struct AltDependentTierChildren<'tree> {
    ///`alt_tier_prefix` — required
    pub child_0: NodeSlot<'tree, AltTierPrefixNode<'tree>>,
    ///`tier_sep` — required
    pub child_1: NodeSlot<'tree, TierSepNode<'tree>>,
    ///`text_with_bullets` — required
    pub child_2: NodeSlot<'tree, TextWithBulletsNode<'tree>>,
    ///`newline` — required
    pub child_3: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `bck_header` nodes.
#[derive(Debug)]
pub struct BckHeaderChildren<'tree> {
    ///`bck_prefix` — required
    pub child_0: NodeSlot<'tree, BckPrefixNode<'tree>>,
    ///`header_sep` — required
    pub child_1: NodeSlot<'tree, HeaderSepNode<'tree>>,
    ///`free_text` — required
    pub child_2: NodeSlot<'tree, FreeTextNode<'tree>>,
    ///`newline` — required
    pub child_3: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `begin_header` nodes.
#[derive(Debug)]
pub struct BeginHeaderChildren<'tree> {
    ///`newline` — required
    pub child_1: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `bg_header` nodes.
#[derive(Debug)]
pub struct BgHeaderChildren<'tree> {
    ///`bg_prefix` — required
    pub child_0: NodeSlot<'tree, BgPrefixNode<'tree>>,
    ///`newline` — required
    pub child_2: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `birth_of_header` nodes.
#[derive(Debug)]
pub struct BirthOfHeaderChildren<'tree> {
    ///`birth_of_prefix` — required
    pub child_0: NodeSlot<'tree, BirthOfPrefixNode<'tree>>,
    ///`header_gap` — optional
    pub child_1: Option<HeaderGapNode<'tree>>,
    ///`speaker` — required
    pub child_2: NodeSlot<'tree, SpeakerNode<'tree>>,
    ///`header_sep` — required
    pub child_3: NodeSlot<'tree, HeaderSepNode<'tree>>,
    ///`date_contents` — required
    pub child_4: NodeSlot<'tree, DateContentsNode<'tree>>,
    ///`newline` — required
    pub child_5: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `birthplace_of_header` nodes.
#[derive(Debug)]
pub struct BirthplaceOfHeaderChildren<'tree> {
    ///`birthplace_of_prefix` — required
    pub child_0: NodeSlot<'tree, BirthplaceOfPrefixNode<'tree>>,
    ///`header_gap` — optional
    pub child_1: Option<HeaderGapNode<'tree>>,
    ///`speaker` — required
    pub child_2: NodeSlot<'tree, SpeakerNode<'tree>>,
    ///`header_sep` — required
    pub child_3: NodeSlot<'tree, HeaderSepNode<'tree>>,
    ///`free_text` — required
    pub child_4: NodeSlot<'tree, FreeTextNode<'tree>>,
    ///`newline` — required
    pub child_5: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `blank_header` nodes.
#[derive(Debug)]
pub struct BlankHeaderChildren<'tree> {
    ///`blank_prefix` — required
    pub child_0: NodeSlot<'tree, BlankPrefixNode<'tree>>,
    ///`newline` — required
    pub child_1: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `bullet` nodes.
#[derive(Debug)]
pub struct BulletChildren<'tree> {
    ///`bullet_start` — required
    pub child_0: NodeSlot<'tree, BulletStartNode<'tree>>,
    ///`bullet_timestamp` — required (field: `start_time`)
    pub start_time: NodeSlot<'tree, BulletTimestampNode<'tree>>,
    ///`bullet_timestamp` — required (field: `end_time`)
    pub end_time: NodeSlot<'tree, BulletTimestampNode<'tree>>,
    ///`bullet_end` — required
    pub child_4: NodeSlot<'tree, BulletEndNode<'tree>>,
}
///Extracted children for `cod_dependent_tier` nodes.
#[derive(Debug)]
pub struct CodDependentTierChildren<'tree> {
    ///`cod_tier_prefix` — required
    pub child_0: NodeSlot<'tree, CodTierPrefixNode<'tree>>,
    ///`tier_sep` — required
    pub child_1: NodeSlot<'tree, TierSepNode<'tree>>,
    ///`text_with_bullets` — required
    pub child_2: NodeSlot<'tree, TextWithBulletsNode<'tree>>,
    ///`newline` — required
    pub child_3: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `coh_dependent_tier` nodes.
#[derive(Debug)]
pub struct CohDependentTierChildren<'tree> {
    ///`coh_tier_prefix` — required
    pub child_0: NodeSlot<'tree, CohTierPrefixNode<'tree>>,
    ///`tier_sep` — required
    pub child_1: NodeSlot<'tree, TierSepNode<'tree>>,
    ///`text_with_bullets` — required
    pub child_2: NodeSlot<'tree, TextWithBulletsNode<'tree>>,
    ///`newline` — required
    pub child_3: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `color_words_header` nodes.
#[derive(Debug)]
pub struct ColorWordsHeaderChildren<'tree> {
    ///`color_words_prefix` — required
    pub child_0: NodeSlot<'tree, ColorWordsPrefixNode<'tree>>,
    ///`header_sep` — required
    pub child_1: NodeSlot<'tree, HeaderSepNode<'tree>>,
    ///`free_text` — required
    pub child_2: NodeSlot<'tree, FreeTextNode<'tree>>,
    ///`newline` — required
    pub child_3: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `com_dependent_tier` nodes.
#[derive(Debug)]
pub struct ComDependentTierChildren<'tree> {
    ///`com_tier_prefix` — required
    pub child_0: NodeSlot<'tree, ComTierPrefixNode<'tree>>,
    ///`tier_sep` — required
    pub child_1: NodeSlot<'tree, TierSepNode<'tree>>,
    ///`text_with_bullets_and_pics` — required
    pub child_2: NodeSlot<'tree, TextWithBulletsAndPicsNode<'tree>>,
    ///`newline` — required
    pub child_3: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `comment_header` nodes.
#[derive(Debug)]
pub struct CommentHeaderChildren<'tree> {
    ///`comment_prefix` — required
    pub child_0: NodeSlot<'tree, CommentPrefixNode<'tree>>,
    ///`header_sep` — required
    pub child_1: NodeSlot<'tree, HeaderSepNode<'tree>>,
    ///`text_with_bullets_and_pics` — required
    pub child_2: NodeSlot<'tree, TextWithBulletsAndPicsNode<'tree>>,
    ///`newline` — required
    pub child_3: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `date_header` nodes.
#[derive(Debug)]
pub struct DateHeaderChildren<'tree> {
    ///`date_prefix` — required
    pub child_0: NodeSlot<'tree, DatePrefixNode<'tree>>,
    ///`header_sep` — required
    pub child_1: NodeSlot<'tree, HeaderSepNode<'tree>>,
    ///`date_contents` — required
    pub child_2: NodeSlot<'tree, DateContentsNode<'tree>>,
    ///`newline` — required
    pub child_3: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `def_dependent_tier` nodes.
#[derive(Debug)]
pub struct DefDependentTierChildren<'tree> {
    ///`def_tier_prefix` — required
    pub child_0: NodeSlot<'tree, DefTierPrefixNode<'tree>>,
    ///`tier_sep` — required
    pub child_1: NodeSlot<'tree, TierSepNode<'tree>>,
    ///`text_with_bullets` — required
    pub child_2: NodeSlot<'tree, TextWithBulletsNode<'tree>>,
    ///`newline` — required
    pub child_3: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `eg_header` nodes.
#[derive(Debug)]
pub struct EgHeaderChildren<'tree> {
    ///`eg_prefix` — required
    pub child_0: NodeSlot<'tree, EgPrefixNode<'tree>>,
    ///`newline` — required
    pub child_2: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `end_header` nodes.
#[derive(Debug)]
pub struct EndHeaderChildren<'tree> {
    ///`newline` — required
    pub child_1: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `eng_dependent_tier` nodes.
#[derive(Debug)]
pub struct EngDependentTierChildren<'tree> {
    ///`eng_tier_prefix` — required
    pub child_0: NodeSlot<'tree, EngTierPrefixNode<'tree>>,
    ///`tier_sep` — required
    pub child_1: NodeSlot<'tree, TierSepNode<'tree>>,
    ///`text_with_bullets` — required
    pub child_2: NodeSlot<'tree, TextWithBulletsNode<'tree>>,
    ///`newline` — required
    pub child_3: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `err_dependent_tier` nodes.
#[derive(Debug)]
pub struct ErrDependentTierChildren<'tree> {
    ///`err_tier_prefix` — required
    pub child_0: NodeSlot<'tree, ErrTierPrefixNode<'tree>>,
    ///`tier_sep` — required
    pub child_1: NodeSlot<'tree, TierSepNode<'tree>>,
    ///`text_with_bullets` — required
    pub child_2: NodeSlot<'tree, TextWithBulletsNode<'tree>>,
    ///`newline` — required
    pub child_3: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `event` nodes.
#[derive(Debug)]
pub struct EventChildren<'tree> {
    ///`event_marker` — required
    pub child_0: NodeSlot<'tree, EventMarkerNode<'tree>>,
    ///`event_segment` — required (field: `description`)
    pub description: NodeSlot<'tree, EventSegmentNode<'tree>>,
}
///Extracted children for `exp_dependent_tier` nodes.
#[derive(Debug)]
pub struct ExpDependentTierChildren<'tree> {
    ///`exp_tier_prefix` — required
    pub child_0: NodeSlot<'tree, ExpTierPrefixNode<'tree>>,
    ///`tier_sep` — required
    pub child_1: NodeSlot<'tree, TierSepNode<'tree>>,
    ///`text_with_bullets` — required
    pub child_2: NodeSlot<'tree, TextWithBulletsNode<'tree>>,
    ///`newline` — required
    pub child_3: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `explanation_annotation` nodes.
#[derive(Debug)]
pub struct ExplanationAnnotationChildren<'tree> {
    ///`space` — required
    pub child_1: NodeSlot<'tree, SpaceNode<'tree>>,
    ///`annotation_content` — required (field: `text`)
    pub text: NodeSlot<'tree, AnnotationContentNode<'tree>>,
    ///`right_bracket` — required
    pub child_3: NodeSlot<'tree, RightBracketNode<'tree>>,
}
///Extracted children for `fac_dependent_tier` nodes.
#[derive(Debug)]
pub struct FacDependentTierChildren<'tree> {
    ///`fac_tier_prefix` — required
    pub child_0: NodeSlot<'tree, FacTierPrefixNode<'tree>>,
    ///`tier_sep` — required
    pub child_1: NodeSlot<'tree, TierSepNode<'tree>>,
    ///`text_with_bullets` — required
    pub child_2: NodeSlot<'tree, TextWithBulletsNode<'tree>>,
    ///`newline` — required
    pub child_3: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `flo_dependent_tier` nodes.
#[derive(Debug)]
pub struct FloDependentTierChildren<'tree> {
    ///`flo_tier_prefix` — required
    pub child_0: NodeSlot<'tree, FloTierPrefixNode<'tree>>,
    ///`tier_sep` — required
    pub child_1: NodeSlot<'tree, TierSepNode<'tree>>,
    ///`text_with_bullets` — required
    pub child_2: NodeSlot<'tree, TextWithBulletsNode<'tree>>,
    ///`newline` — required
    pub child_3: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `font_header` nodes.
#[derive(Debug)]
pub struct FontHeaderChildren<'tree> {
    ///`font_prefix` — required
    pub child_0: NodeSlot<'tree, FontPrefixNode<'tree>>,
    ///`header_sep` — required
    pub child_1: NodeSlot<'tree, HeaderSepNode<'tree>>,
    ///`free_text` — required
    pub child_2: NodeSlot<'tree, FreeTextNode<'tree>>,
    ///`newline` — required
    pub child_3: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `full_document` nodes.
#[derive(Debug)]
pub struct FullDocumentChildren<'tree> {
    ///`utf8_header` — required
    pub child_0: NodeSlot<'tree, Utf8HeaderNode<'tree>>,
    ///`begin_header` — required
    pub child_2: NodeSlot<'tree, BeginHeaderNode<'tree>>,
    ///`end_header` — required
    pub child_4: NodeSlot<'tree, EndHeaderNode<'tree>>,
}
///Extracted children for `g_header` nodes.
#[derive(Debug)]
pub struct GHeaderChildren<'tree> {
    ///`g_prefix` — required
    pub child_0: NodeSlot<'tree, GPrefixNode<'tree>>,
    ///`header_sep` — required
    pub child_1: NodeSlot<'tree, HeaderSepNode<'tree>>,
    ///`free_text` — required
    pub child_2: NodeSlot<'tree, FreeTextNode<'tree>>,
    ///`newline` — required
    pub child_3: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `gls_dependent_tier` nodes.
#[derive(Debug)]
pub struct GlsDependentTierChildren<'tree> {
    ///`gls_tier_prefix` — required
    pub child_0: NodeSlot<'tree, GlsTierPrefixNode<'tree>>,
    ///`tier_sep` — required
    pub child_1: NodeSlot<'tree, TierSepNode<'tree>>,
    ///`text_with_bullets` — required
    pub child_2: NodeSlot<'tree, TextWithBulletsNode<'tree>>,
    ///`newline` — required
    pub child_3: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `gpx_dependent_tier` nodes.
#[derive(Debug)]
pub struct GpxDependentTierChildren<'tree> {
    ///`gpx_tier_prefix` — required
    pub child_0: NodeSlot<'tree, GpxTierPrefixNode<'tree>>,
    ///`tier_sep` — required
    pub child_1: NodeSlot<'tree, TierSepNode<'tree>>,
    ///`text_with_bullets` — required
    pub child_2: NodeSlot<'tree, TextWithBulletsNode<'tree>>,
    ///`newline` — required
    pub child_3: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `gra_contents` nodes.
#[derive(Debug)]
pub struct GraContentsChildren<'tree> {
    ///`gra_relation` — required
    pub child_0: NodeSlot<'tree, GraRelationNode<'tree>>,
}
///Extracted children for `gra_dependent_tier` nodes.
#[derive(Debug)]
pub struct GraDependentTierChildren<'tree> {
    ///`gra_tier_prefix` — required
    pub child_0: NodeSlot<'tree, GraTierPrefixNode<'tree>>,
    ///`tier_sep` — required
    pub child_1: NodeSlot<'tree, TierSepNode<'tree>>,
    ///`gra_contents` — required
    pub child_2: NodeSlot<'tree, GraContentsNode<'tree>>,
    ///`newline` — required
    pub child_3: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `gra_relation` nodes.
#[derive(Debug)]
pub struct GraRelationChildren<'tree> {
    ///`gra_index` — required (field: `index`)
    pub index: NodeSlot<'tree, GraIndexNode<'tree>>,
    ///`pipe` — required
    pub child_1: NodeSlot<'tree, PipeNode<'tree>>,
    ///`gra_head` — required (field: `head`)
    pub head: NodeSlot<'tree, GraHeadNode<'tree>>,
    ///`pipe` — required
    pub child_3: NodeSlot<'tree, PipeNode<'tree>>,
    ///`gra_relation_name` — required (field: `relation`)
    pub relation: NodeSlot<'tree, GraRelationNameNode<'tree>>,
}
///Extracted children for `group_with_annotations` nodes.
#[derive(Debug)]
pub struct GroupWithAnnotationsChildren<'tree> {
    ///`less_than` — required
    pub child_0: NodeSlot<'tree, LessThanNode<'tree>>,
    ///`contents` — required (field: `content`)
    pub content: NodeSlot<'tree, ContentsNode<'tree>>,
    ///`greater_than` — required
    pub child_2: NodeSlot<'tree, GreaterThanNode<'tree>>,
    ///`base_annotations` — required (field: `annotations`)
    pub annotations: NodeSlot<'tree, BaseAnnotationsNode<'tree>>,
}
///Extracted children for `header_sep` nodes.
#[derive(Debug)]
pub struct HeaderSepChildren<'tree> {
    ///`colon` — required
    pub child_0: NodeSlot<'tree, ColonNode<'tree>>,
    ///`tab` — required
    pub child_1: NodeSlot<'tree, TabNode<'tree>>,
}
///Extracted children for `id_contents` nodes.
#[derive(Debug)]
pub struct IdContentsChildren<'tree> {
    ///`_id_identity_fields` — required
    pub child_0: NodeSlot<'tree, IdIdentityFieldsNode<'tree>>,
    ///`_id_demographic_fields` — required
    pub child_1: NodeSlot<'tree, IdDemographicFieldsNode<'tree>>,
    ///`_id_role_fields` — required
    pub child_2: NodeSlot<'tree, IdRoleFieldsNode<'tree>>,
}
///Extracted children for `id_header` nodes.
#[derive(Debug)]
pub struct IdHeaderChildren<'tree> {
    ///`id_prefix` — required
    pub child_0: NodeSlot<'tree, IdPrefixNode<'tree>>,
    ///`header_sep` — required
    pub child_1: NodeSlot<'tree, HeaderSepNode<'tree>>,
    ///`id_contents` — required
    pub child_2: NodeSlot<'tree, IdContentsNode<'tree>>,
    ///`newline` — required
    pub child_3: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `int_dependent_tier` nodes.
#[derive(Debug)]
pub struct IntDependentTierChildren<'tree> {
    ///`int_tier_prefix` — required
    pub child_0: NodeSlot<'tree, IntTierPrefixNode<'tree>>,
    ///`tier_sep` — required
    pub child_1: NodeSlot<'tree, TierSepNode<'tree>>,
    ///`text_with_bullets` — required
    pub child_2: NodeSlot<'tree, TextWithBulletsNode<'tree>>,
    ///`newline` — required
    pub child_3: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `l1_of_header` nodes.
#[derive(Debug)]
pub struct L1OfHeaderChildren<'tree> {
    ///`l1_of_prefix` — required
    pub child_0: NodeSlot<'tree, L1OfPrefixNode<'tree>>,
    ///`header_gap` — optional
    pub child_1: Option<HeaderGapNode<'tree>>,
    ///`speaker` — required
    pub child_2: NodeSlot<'tree, SpeakerNode<'tree>>,
    ///`header_sep` — required
    pub child_3: NodeSlot<'tree, HeaderSepNode<'tree>>,
    ///`language_code` — required
    pub child_4: NodeSlot<'tree, LanguageCodeNode<'tree>>,
    ///`newline` — required
    pub child_5: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `langcode` nodes.
#[derive(Debug)]
pub struct LangcodeChildren<'tree> {
    ///`space` — required
    pub child_1: NodeSlot<'tree, SpaceNode<'tree>>,
    ///`language_code` — required (field: `code`)
    pub code: NodeSlot<'tree, LanguageCodeNode<'tree>>,
    ///`right_bracket` — required
    pub child_3: NodeSlot<'tree, RightBracketNode<'tree>>,
}
///Extracted children for `languages_contents` nodes.
#[derive(Debug)]
pub struct LanguagesContentsChildren<'tree> {
    ///`language_code` — required
    pub child_0: NodeSlot<'tree, LanguageCodeNode<'tree>>,
}
///Extracted children for `languages_header` nodes.
#[derive(Debug)]
pub struct LanguagesHeaderChildren<'tree> {
    ///`languages_prefix` — required
    pub child_0: NodeSlot<'tree, LanguagesPrefixNode<'tree>>,
    ///`header_sep` — required
    pub child_1: NodeSlot<'tree, HeaderSepNode<'tree>>,
    ///`languages_contents` — required
    pub child_2: NodeSlot<'tree, LanguagesContentsNode<'tree>>,
    ///`newline` — required
    pub child_3: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `location_header` nodes.
#[derive(Debug)]
pub struct LocationHeaderChildren<'tree> {
    ///`location_prefix` — required
    pub child_0: NodeSlot<'tree, LocationPrefixNode<'tree>>,
    ///`header_sep` — required
    pub child_1: NodeSlot<'tree, HeaderSepNode<'tree>>,
    ///`free_text` — required
    pub child_2: NodeSlot<'tree, FreeTextNode<'tree>>,
    ///`newline` — required
    pub child_3: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `long_feature_begin` nodes.
#[derive(Debug)]
pub struct LongFeatureBeginChildren<'tree> {
    ///`ampersand` — required
    pub child_0: NodeSlot<'tree, AmpersandNode<'tree>>,
    ///`long_feature_begin_marker` — required
    pub child_1: NodeSlot<'tree, LongFeatureBeginMarkerNode<'tree>>,
    ///`long_feature_label` — required
    pub child_2: NodeSlot<'tree, LongFeatureLabelNode<'tree>>,
}
///Extracted children for `long_feature_end` nodes.
#[derive(Debug)]
pub struct LongFeatureEndChildren<'tree> {
    ///`ampersand` — required
    pub child_0: NodeSlot<'tree, AmpersandNode<'tree>>,
    ///`long_feature_end_marker` — required
    pub child_1: NodeSlot<'tree, LongFeatureEndMarkerNode<'tree>>,
    ///`long_feature_label` — required
    pub child_2: NodeSlot<'tree, LongFeatureLabelNode<'tree>>,
}
///Extracted children for `main_pho_group` nodes.
#[derive(Debug)]
pub struct MainPhoGroupChildren<'tree> {
    ///`pho_begin_group` — required
    pub child_0: NodeSlot<'tree, PhoBeginGroupNode<'tree>>,
    ///`contents` — required
    pub child_1: NodeSlot<'tree, ContentsNode<'tree>>,
    ///`pho_end_group` — required
    pub child_2: NodeSlot<'tree, PhoEndGroupNode<'tree>>,
}
///Extracted children for `main_sin_group` nodes.
#[derive(Debug)]
pub struct MainSinGroupChildren<'tree> {
    ///`sin_begin_group` — required
    pub child_0: NodeSlot<'tree, SinBeginGroupNode<'tree>>,
    ///`contents` — required
    pub child_1: NodeSlot<'tree, ContentsNode<'tree>>,
    ///`sin_end_group` — required
    pub child_2: NodeSlot<'tree, SinEndGroupNode<'tree>>,
}
///Extracted children for `main_tier` nodes.
#[derive(Debug)]
pub struct MainTierChildren<'tree> {
    ///`star` — required
    pub child_0: NodeSlot<'tree, StarNode<'tree>>,
    ///`speaker` — required (field: `speaker`)
    pub speaker: NodeSlot<'tree, SpeakerNode<'tree>>,
    ///`colon` — required
    pub child_2: NodeSlot<'tree, ColonNode<'tree>>,
    ///`tab` — required
    pub child_3: NodeSlot<'tree, TabNode<'tree>>,
    ///`tier_body` — required
    pub child_4: NodeSlot<'tree, TierBodyNode<'tree>>,
}
///Extracted children for `media_contents` nodes.
#[derive(Debug)]
pub struct MediaContentsChildren<'tree> {
    ///`media_filename` — required
    pub child_0: NodeSlot<'tree, MediaFilenameNode<'tree>>,
    ///`comma` — required
    pub child_1: NodeSlot<'tree, CommaNode<'tree>>,
    ///`whitespaces` — required
    pub child_2: NodeSlot<'tree, WhitespacesNode<'tree>>,
    ///`media_type` — required
    pub child_3: NodeSlot<'tree, MediaTypeNode<'tree>>,
}
///Extracted children for `media_header` nodes.
#[derive(Debug)]
pub struct MediaHeaderChildren<'tree> {
    ///`media_prefix` — required
    pub child_0: NodeSlot<'tree, MediaPrefixNode<'tree>>,
    ///`header_sep` — required
    pub child_1: NodeSlot<'tree, HeaderSepNode<'tree>>,
    ///`media_contents` — required
    pub child_2: NodeSlot<'tree, MediaContentsNode<'tree>>,
    ///`newline` — required
    pub child_3: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `mod_dependent_tier` nodes.
#[derive(Debug)]
pub struct ModDependentTierChildren<'tree> {
    ///`mod_tier_prefix` — required
    pub child_0: NodeSlot<'tree, ModTierPrefixNode<'tree>>,
    ///`tier_sep` — required
    pub child_1: NodeSlot<'tree, TierSepNode<'tree>>,
    ///`pho_groups` — required
    pub child_2: NodeSlot<'tree, PhoGroupsNode<'tree>>,
    ///`newline` — required
    pub child_3: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `modsyl_dependent_tier` nodes.
#[derive(Debug)]
pub struct ModsylDependentTierChildren<'tree> {
    ///`modsyl_tier_prefix` — required
    pub child_0: NodeSlot<'tree, ModsylTierPrefixNode<'tree>>,
    ///`tier_sep` — required
    pub child_1: NodeSlot<'tree, TierSepNode<'tree>>,
    ///`text_with_bullets` — required
    pub child_2: NodeSlot<'tree, TextWithBulletsNode<'tree>>,
    ///`newline` — required
    pub child_3: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `mor_content` nodes.
#[derive(Debug)]
pub struct MorContentChildren<'tree> {
    ///`mor_word` — required (field: `main`)
    pub main: NodeSlot<'tree, MorWordNode<'tree>>,
}
///Extracted children for `mor_contents` nodes.
#[derive(Debug)]
pub struct MorContentsChildren<'tree> {
    ///`whitespaces` — optional
    pub child_1: Option<WhitespacesNode<'tree>>,
}
///Extracted children for `mor_dependent_tier` nodes.
#[derive(Debug)]
pub struct MorDependentTierChildren<'tree> {
    ///`mor_tier_prefix` — required
    pub child_0: NodeSlot<'tree, MorTierPrefixNode<'tree>>,
    ///`tier_sep` — required
    pub child_1: NodeSlot<'tree, TierSepNode<'tree>>,
    ///`mor_contents` — required
    pub child_2: NodeSlot<'tree, MorContentsNode<'tree>>,
    ///`newline` — required
    pub child_3: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `mor_feature` nodes.
#[derive(Debug)]
pub struct MorFeatureChildren<'tree> {
    ///`hyphen` — required
    pub child_0: NodeSlot<'tree, HyphenNode<'tree>>,
    ///`mor_feature_value` — required
    pub child_1: NodeSlot<'tree, MorFeatureValueNode<'tree>>,
}
///Extracted children for `mor_post_clitic` nodes.
#[derive(Debug)]
pub struct MorPostCliticChildren<'tree> {
    ///`tilde` — required
    pub child_0: NodeSlot<'tree, TildeNode<'tree>>,
    ///`mor_word` — required
    pub child_1: NodeSlot<'tree, MorWordNode<'tree>>,
}
///Extracted children for `mor_word` nodes.
#[derive(Debug)]
pub struct MorWordChildren<'tree> {
    ///`mor_pos` — required
    pub child_0: NodeSlot<'tree, MorPosNode<'tree>>,
    ///`pipe` — required
    pub child_1: NodeSlot<'tree, PipeNode<'tree>>,
    ///`mor_lemma` — required
    pub child_2: NodeSlot<'tree, MorLemmaNode<'tree>>,
}
///Extracted children for `new_episode_header` nodes.
#[derive(Debug)]
pub struct NewEpisodeHeaderChildren<'tree> {
    ///`new_episode_prefix` — required
    pub child_0: NodeSlot<'tree, NewEpisodePrefixNode<'tree>>,
    ///`newline` — required
    pub child_1: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `nonvocal_begin` nodes.
#[derive(Debug)]
pub struct NonvocalBeginChildren<'tree> {
    ///`ampersand` — required
    pub child_0: NodeSlot<'tree, AmpersandNode<'tree>>,
    ///`nonvocal_begin_marker` — required
    pub child_1: NodeSlot<'tree, NonvocalBeginMarkerNode<'tree>>,
    ///`long_feature_label` — required
    pub child_2: NodeSlot<'tree, LongFeatureLabelNode<'tree>>,
}
///Extracted children for `nonvocal_end` nodes.
#[derive(Debug)]
pub struct NonvocalEndChildren<'tree> {
    ///`ampersand` — required
    pub child_0: NodeSlot<'tree, AmpersandNode<'tree>>,
    ///`nonvocal_end_marker` — required
    pub child_1: NodeSlot<'tree, NonvocalEndMarkerNode<'tree>>,
    ///`long_feature_label` — required
    pub child_2: NodeSlot<'tree, LongFeatureLabelNode<'tree>>,
}
///Extracted children for `nonvocal_simple` nodes.
#[derive(Debug)]
pub struct NonvocalSimpleChildren<'tree> {
    ///`ampersand` — required
    pub child_0: NodeSlot<'tree, AmpersandNode<'tree>>,
    ///`nonvocal_begin_marker` — required
    pub child_1: NodeSlot<'tree, NonvocalBeginMarkerNode<'tree>>,
    ///`long_feature_label` — required
    pub child_2: NodeSlot<'tree, LongFeatureLabelNode<'tree>>,
    ///`right_brace` — required
    pub child_3: NodeSlot<'tree, RightBraceNode<'tree>>,
}
///Extracted children for `nonword_with_optional_annotations` nodes.
#[derive(Debug)]
pub struct NonwordWithOptionalAnnotationsChildren<'tree> {
    ///`nonword` — required (field: `nonword`)
    pub nonword: NodeSlot<'tree, NonwordNode<'tree>>,
    ///`base_annotations` — optional (field: `annotations`)
    pub annotations: Option<BaseAnnotationsNode<'tree>>,
}
///Extracted children for `number_header` nodes.
#[derive(Debug)]
pub struct NumberHeaderChildren<'tree> {
    ///`number_prefix` — required
    pub child_0: NodeSlot<'tree, NumberPrefixNode<'tree>>,
    ///`header_sep` — required
    pub child_1: NodeSlot<'tree, HeaderSepNode<'tree>>,
    ///`number_option` — required
    pub child_2: NodeSlot<'tree, NumberOptionNode<'tree>>,
    ///`newline` — required
    pub child_3: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `options_contents` nodes.
#[derive(Debug)]
pub struct OptionsContentsChildren<'tree> {
    ///`option_name` — required
    pub child_0: NodeSlot<'tree, OptionNameNode<'tree>>,
}
///Extracted children for `options_header` nodes.
#[derive(Debug)]
pub struct OptionsHeaderChildren<'tree> {
    ///`options_prefix` — required
    pub child_0: NodeSlot<'tree, OptionsPrefixNode<'tree>>,
    ///`header_sep` — required
    pub child_1: NodeSlot<'tree, HeaderSepNode<'tree>>,
    ///`options_contents` — required
    pub child_2: NodeSlot<'tree, OptionsContentsNode<'tree>>,
    ///`newline` — required
    pub child_3: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `ort_dependent_tier` nodes.
#[derive(Debug)]
pub struct OrtDependentTierChildren<'tree> {
    ///`ort_tier_prefix` — required
    pub child_0: NodeSlot<'tree, OrtTierPrefixNode<'tree>>,
    ///`tier_sep` — required
    pub child_1: NodeSlot<'tree, TierSepNode<'tree>>,
    ///`text_with_bullets` — required
    pub child_2: NodeSlot<'tree, TextWithBulletsNode<'tree>>,
    ///`newline` — required
    pub child_3: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `other_spoken_event` nodes.
#[derive(Debug)]
pub struct OtherSpokenEventChildren<'tree> {
    ///`ampersand` — required
    pub child_0: NodeSlot<'tree, AmpersandNode<'tree>>,
    ///`star` — required
    pub child_1: NodeSlot<'tree, StarNode<'tree>>,
    ///`speaker` — required
    pub child_2: NodeSlot<'tree, SpeakerNode<'tree>>,
    ///`colon` — required
    pub child_3: NodeSlot<'tree, ColonNode<'tree>>,
    ///`standalone_word` — required
    pub child_4: NodeSlot<'tree, StandaloneWordNode<'tree>>,
}
///Extracted children for `page_header` nodes.
#[derive(Debug)]
pub struct PageHeaderChildren<'tree> {
    ///`page_prefix` — required
    pub child_0: NodeSlot<'tree, PagePrefixNode<'tree>>,
    ///`header_sep` — required
    pub child_1: NodeSlot<'tree, HeaderSepNode<'tree>>,
    ///`page_number` — required
    pub child_2: NodeSlot<'tree, PageNumberNode<'tree>>,
    ///`newline` — required
    pub child_3: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `par_dependent_tier` nodes.
#[derive(Debug)]
pub struct ParDependentTierChildren<'tree> {
    ///`par_tier_prefix` — required
    pub child_0: NodeSlot<'tree, ParTierPrefixNode<'tree>>,
    ///`tier_sep` — required
    pub child_1: NodeSlot<'tree, TierSepNode<'tree>>,
    ///`text_with_bullets` — required
    pub child_2: NodeSlot<'tree, TextWithBulletsNode<'tree>>,
    ///`newline` — required
    pub child_3: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `para_annotation` nodes.
#[derive(Debug)]
pub struct ParaAnnotationChildren<'tree> {
    ///`space` — required
    pub child_1: NodeSlot<'tree, SpaceNode<'tree>>,
    ///`annotation_content` — required (field: `text`)
    pub text: NodeSlot<'tree, AnnotationContentNode<'tree>>,
    ///`right_bracket` — required
    pub child_3: NodeSlot<'tree, RightBracketNode<'tree>>,
}
///Extracted children for `participant` nodes.
#[derive(Debug)]
pub struct ParticipantChildren<'tree> {
    ///`speaker` — required (field: `code`)
    pub code: NodeSlot<'tree, SpeakerNode<'tree>>,
    ///`whitespaces` — optional
    pub child_2: Option<WhitespacesNode<'tree>>,
}
///Extracted children for `participants_contents` nodes.
#[derive(Debug)]
pub struct ParticipantsContentsChildren<'tree> {
    ///`participant` — required
    pub child_0: NodeSlot<'tree, ParticipantNode<'tree>>,
}
///Extracted children for `participants_header` nodes.
#[derive(Debug)]
pub struct ParticipantsHeaderChildren<'tree> {
    ///`participants_prefix` — required
    pub child_0: NodeSlot<'tree, ParticipantsPrefixNode<'tree>>,
    ///`header_sep` — required
    pub child_1: NodeSlot<'tree, HeaderSepNode<'tree>>,
    ///`participants_contents` — required
    pub child_2: NodeSlot<'tree, ParticipantsContentsNode<'tree>>,
    ///`newline` — required
    pub child_3: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `percent_annotation` nodes.
#[derive(Debug)]
pub struct PercentAnnotationChildren<'tree> {
    ///`space` — required
    pub child_1: NodeSlot<'tree, SpaceNode<'tree>>,
    ///`annotation_content` — required (field: `text`)
    pub text: NodeSlot<'tree, AnnotationContentNode<'tree>>,
    ///`right_bracket` — required
    pub child_3: NodeSlot<'tree, RightBracketNode<'tree>>,
}
///Extracted children for `pho_dependent_tier` nodes.
#[derive(Debug)]
pub struct PhoDependentTierChildren<'tree> {
    ///`pho_tier_prefix` — required
    pub child_0: NodeSlot<'tree, PhoTierPrefixNode<'tree>>,
    ///`tier_sep` — required
    pub child_1: NodeSlot<'tree, TierSepNode<'tree>>,
    ///`pho_groups` — required
    pub child_2: NodeSlot<'tree, PhoGroupsNode<'tree>>,
    ///`newline` — required
    pub child_3: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `pho_grouped_content` nodes.
#[derive(Debug)]
pub struct PhoGroupedContentChildren<'tree> {
    ///`pho_words` — required
    pub child_0: NodeSlot<'tree, PhoWordsNode<'tree>>,
}
///Extracted children for `pho_groups` nodes.
#[derive(Debug)]
pub struct PhoGroupsChildren<'tree> {
    ///`pho_group` — required
    pub child_0: NodeSlot<'tree, PhoGroupNode<'tree>>,
}
///Extracted children for `pho_words` nodes.
#[derive(Debug)]
pub struct PhoWordsChildren<'tree> {
    ///`pho_word` — required
    pub child_0: NodeSlot<'tree, PhoWordNode<'tree>>,
}
///Extracted children for `phoaln_dependent_tier` nodes.
#[derive(Debug)]
pub struct PhoalnDependentTierChildren<'tree> {
    ///`phoaln_tier_prefix` — required
    pub child_0: NodeSlot<'tree, PhoalnTierPrefixNode<'tree>>,
    ///`tier_sep` — required
    pub child_1: NodeSlot<'tree, TierSepNode<'tree>>,
    ///`text_with_bullets` — required
    pub child_2: NodeSlot<'tree, TextWithBulletsNode<'tree>>,
    ///`newline` — required
    pub child_3: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `phosyl_dependent_tier` nodes.
#[derive(Debug)]
pub struct PhosylDependentTierChildren<'tree> {
    ///`phosyl_tier_prefix` — required
    pub child_0: NodeSlot<'tree, PhosylTierPrefixNode<'tree>>,
    ///`tier_sep` — required
    pub child_1: NodeSlot<'tree, TierSepNode<'tree>>,
    ///`text_with_bullets` — required
    pub child_2: NodeSlot<'tree, TextWithBulletsNode<'tree>>,
    ///`newline` — required
    pub child_3: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `pid_header` nodes.
#[derive(Debug)]
pub struct PidHeaderChildren<'tree> {
    ///`pid_prefix` — required
    pub child_0: NodeSlot<'tree, PidPrefixNode<'tree>>,
    ///`header_sep` — required
    pub child_1: NodeSlot<'tree, HeaderSepNode<'tree>>,
    ///`free_text` — required
    pub child_2: NodeSlot<'tree, FreeTextNode<'tree>>,
    ///`newline` — required
    pub child_3: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `pos_tag` nodes.
#[derive(Debug)]
pub struct PosTagChildren<'tree> {
    _phantom: std::marker::PhantomData<&'tree ()>,
}
///Extracted children for `postcode` nodes.
#[derive(Debug)]
pub struct PostcodeChildren<'tree> {
    ///`space` — required
    pub child_1: NodeSlot<'tree, SpaceNode<'tree>>,
    ///`annotation_content` — required (field: `code`)
    pub code: NodeSlot<'tree, AnnotationContentNode<'tree>>,
    ///`right_bracket` — required
    pub child_3: NodeSlot<'tree, RightBracketNode<'tree>>,
}
///Extracted children for `quotation` nodes.
#[derive(Debug)]
pub struct QuotationChildren<'tree> {
    ///`left_double_quote` — required
    pub child_0: NodeSlot<'tree, LeftDoubleQuoteNode<'tree>>,
    ///`contents` — required
    pub child_1: NodeSlot<'tree, ContentsNode<'tree>>,
    ///`right_double_quote` — required
    pub child_2: NodeSlot<'tree, RightDoubleQuoteNode<'tree>>,
}
///Extracted children for `recording_quality_header` nodes.
#[derive(Debug)]
pub struct RecordingQualityHeaderChildren<'tree> {
    ///`recording_quality_prefix` — required
    pub child_0: NodeSlot<'tree, RecordingQualityPrefixNode<'tree>>,
    ///`header_sep` — required
    pub child_1: NodeSlot<'tree, HeaderSepNode<'tree>>,
    ///`recording_quality_option` — required
    pub child_2: NodeSlot<'tree, RecordingQualityOptionNode<'tree>>,
    ///`newline` — required
    pub child_3: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `replacement` nodes.
#[derive(Debug)]
pub struct ReplacementChildren<'tree> {
    ///`left_bracket` — required
    pub child_0: NodeSlot<'tree, LeftBracketNode<'tree>>,
    ///`colon` — required
    pub child_1: NodeSlot<'tree, ColonNode<'tree>>,
    ///`right_bracket` — required
    pub child_3: NodeSlot<'tree, RightBracketNode<'tree>>,
}
///Extracted children for `room_layout_header` nodes.
#[derive(Debug)]
pub struct RoomLayoutHeaderChildren<'tree> {
    ///`room_layout_prefix` — required
    pub child_0: NodeSlot<'tree, RoomLayoutPrefixNode<'tree>>,
    ///`header_sep` — required
    pub child_1: NodeSlot<'tree, HeaderSepNode<'tree>>,
    ///`free_text` — required
    pub child_2: NodeSlot<'tree, FreeTextNode<'tree>>,
    ///`newline` — required
    pub child_3: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `shortening` nodes.
#[derive(Debug)]
pub struct ShorteningChildren<'tree> {
    ///`word_segment` — required
    pub child_1: NodeSlot<'tree, WordSegmentNode<'tree>>,
}
///Extracted children for `sin_dependent_tier` nodes.
#[derive(Debug)]
pub struct SinDependentTierChildren<'tree> {
    ///`sin_tier_prefix` — required
    pub child_0: NodeSlot<'tree, SinTierPrefixNode<'tree>>,
    ///`tier_sep` — required
    pub child_1: NodeSlot<'tree, TierSepNode<'tree>>,
    ///`sin_groups` — required
    pub child_2: NodeSlot<'tree, SinGroupsNode<'tree>>,
    ///`newline` — required
    pub child_3: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `sin_grouped_content` nodes.
#[derive(Debug)]
pub struct SinGroupedContentChildren<'tree> {
    ///`sin_word` — required
    pub child_0: NodeSlot<'tree, SinWordNode<'tree>>,
}
///Extracted children for `sin_groups` nodes.
#[derive(Debug)]
pub struct SinGroupsChildren<'tree> {
    ///`sin_group` — required
    pub child_0: NodeSlot<'tree, SinGroupNode<'tree>>,
}
///Extracted children for `sit_dependent_tier` nodes.
#[derive(Debug)]
pub struct SitDependentTierChildren<'tree> {
    ///`sit_tier_prefix` — required
    pub child_0: NodeSlot<'tree, SitTierPrefixNode<'tree>>,
    ///`tier_sep` — required
    pub child_1: NodeSlot<'tree, TierSepNode<'tree>>,
    ///`text_with_bullets` — required
    pub child_2: NodeSlot<'tree, TextWithBulletsNode<'tree>>,
    ///`newline` — required
    pub child_3: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `situation_header` nodes.
#[derive(Debug)]
pub struct SituationHeaderChildren<'tree> {
    ///`situation_prefix` — required
    pub child_0: NodeSlot<'tree, SituationPrefixNode<'tree>>,
    ///`header_sep` — required
    pub child_1: NodeSlot<'tree, HeaderSepNode<'tree>>,
    ///`free_text` — required
    pub child_2: NodeSlot<'tree, FreeTextNode<'tree>>,
    ///`newline` — required
    pub child_3: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `spa_dependent_tier` nodes.
#[derive(Debug)]
pub struct SpaDependentTierChildren<'tree> {
    ///`spa_tier_prefix` — required
    pub child_0: NodeSlot<'tree, SpaTierPrefixNode<'tree>>,
    ///`tier_sep` — required
    pub child_1: NodeSlot<'tree, TierSepNode<'tree>>,
    ///`text_with_bullets` — required
    pub child_2: NodeSlot<'tree, TextWithBulletsNode<'tree>>,
    ///`newline` — required
    pub child_3: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `standalone_word` nodes.
#[derive(Debug)]
pub struct StandaloneWordChildren<'tree> {
    ///`word_body` — required
    pub child_1: NodeSlot<'tree, WordBodyNode<'tree>>,
    ///`form_marker` — optional
    pub child_2: Option<FormMarkerNode<'tree>>,
    ///`word_lang_suffix` — optional
    pub child_3: Option<WordLangSuffixNode<'tree>>,
    ///`pos_tag` — optional
    pub child_4: Option<PosTagNode<'tree>>,
}
///Extracted children for `t_header` nodes.
#[derive(Debug)]
pub struct THeaderChildren<'tree> {
    ///`t_prefix` — required
    pub child_0: NodeSlot<'tree, TPrefixNode<'tree>>,
    ///`header_sep` — required
    pub child_1: NodeSlot<'tree, HeaderSepNode<'tree>>,
    ///`free_text` — required
    pub child_2: NodeSlot<'tree, FreeTextNode<'tree>>,
    ///`newline` — required
    pub child_3: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `tape_location_header` nodes.
#[derive(Debug)]
pub struct TapeLocationHeaderChildren<'tree> {
    ///`tape_location_prefix` — required
    pub child_0: NodeSlot<'tree, TapeLocationPrefixNode<'tree>>,
    ///`header_sep` — required
    pub child_1: NodeSlot<'tree, HeaderSepNode<'tree>>,
    ///`free_text` — required
    pub child_2: NodeSlot<'tree, FreeTextNode<'tree>>,
    ///`newline` — required
    pub child_3: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `thumbnail_header` nodes.
#[derive(Debug)]
pub struct ThumbnailHeaderChildren<'tree> {
    ///`thumbnail_prefix` — required
    pub child_0: NodeSlot<'tree, ThumbnailPrefixNode<'tree>>,
    ///`header_sep` — required
    pub child_1: NodeSlot<'tree, HeaderSepNode<'tree>>,
    ///`free_text` — required
    pub child_2: NodeSlot<'tree, FreeTextNode<'tree>>,
    ///`newline` — required
    pub child_3: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `tier_body` nodes.
#[derive(Debug)]
pub struct TierBodyChildren<'tree> {
    ///`linkers` — optional (field: `linkers`)
    pub linkers: Option<LinkersNode<'tree>>,
    ///`contents` — required (field: `content`)
    pub content: NodeSlot<'tree, ContentsNode<'tree>>,
    ///`utterance_end` — required (field: `ending`)
    pub ending: NodeSlot<'tree, UtteranceEndNode<'tree>>,
}
///Extracted children for `tier_sep` nodes.
#[derive(Debug)]
pub struct TierSepChildren<'tree> {
    ///`colon` — required
    pub child_0: NodeSlot<'tree, ColonNode<'tree>>,
    ///`tab` — required
    pub child_1: NodeSlot<'tree, TabNode<'tree>>,
}
///Extracted children for `tim_dependent_tier` nodes.
#[derive(Debug)]
pub struct TimDependentTierChildren<'tree> {
    ///`tim_tier_prefix` — required
    pub child_0: NodeSlot<'tree, TimTierPrefixNode<'tree>>,
    ///`tier_sep` — required
    pub child_1: NodeSlot<'tree, TierSepNode<'tree>>,
    ///`text_with_bullets` — required
    pub child_2: NodeSlot<'tree, TextWithBulletsNode<'tree>>,
    ///`newline` — required
    pub child_3: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `time_duration_header` nodes.
#[derive(Debug)]
pub struct TimeDurationHeaderChildren<'tree> {
    ///`time_duration_prefix` — required
    pub child_0: NodeSlot<'tree, TimeDurationPrefixNode<'tree>>,
    ///`header_sep` — required
    pub child_1: NodeSlot<'tree, HeaderSepNode<'tree>>,
    ///`time_duration_contents` — required
    pub child_2: NodeSlot<'tree, TimeDurationContentsNode<'tree>>,
    ///`newline` — required
    pub child_3: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `time_start_header` nodes.
#[derive(Debug)]
pub struct TimeStartHeaderChildren<'tree> {
    ///`time_start_prefix` — required
    pub child_0: NodeSlot<'tree, TimeStartPrefixNode<'tree>>,
    ///`header_sep` — required
    pub child_1: NodeSlot<'tree, HeaderSepNode<'tree>>,
    ///`time_duration_contents` — required
    pub child_2: NodeSlot<'tree, TimeDurationContentsNode<'tree>>,
    ///`newline` — required
    pub child_3: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `transcriber_header` nodes.
#[derive(Debug)]
pub struct TranscriberHeaderChildren<'tree> {
    ///`transcriber_prefix` — required
    pub child_0: NodeSlot<'tree, TranscriberPrefixNode<'tree>>,
    ///`header_sep` — required
    pub child_1: NodeSlot<'tree, HeaderSepNode<'tree>>,
    ///`free_text` — required
    pub child_2: NodeSlot<'tree, FreeTextNode<'tree>>,
    ///`newline` — required
    pub child_3: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `transcription_header` nodes.
#[derive(Debug)]
pub struct TranscriptionHeaderChildren<'tree> {
    ///`transcription_prefix` — required
    pub child_0: NodeSlot<'tree, TranscriptionPrefixNode<'tree>>,
    ///`header_sep` — required
    pub child_1: NodeSlot<'tree, HeaderSepNode<'tree>>,
    ///`transcription_option` — required
    pub child_2: NodeSlot<'tree, TranscriptionOptionNode<'tree>>,
    ///`newline` — required
    pub child_3: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `types_header` nodes.
#[derive(Debug)]
pub struct TypesHeaderChildren<'tree> {
    ///`types_prefix` — required
    pub child_0: NodeSlot<'tree, TypesPrefixNode<'tree>>,
    ///`header_sep` — required
    pub child_1: NodeSlot<'tree, HeaderSepNode<'tree>>,
    ///`types_design` — required
    pub child_2: NodeSlot<'tree, TypesDesignNode<'tree>>,
    ///`whitespaces` — optional
    pub child_3: Option<WhitespacesNode<'tree>>,
    ///`comma` — required
    pub child_4: NodeSlot<'tree, CommaNode<'tree>>,
    ///`whitespaces` — optional
    pub child_5: Option<WhitespacesNode<'tree>>,
    ///`types_activity` — required
    pub child_6: NodeSlot<'tree, TypesActivityNode<'tree>>,
    ///`whitespaces` — optional
    pub child_7: Option<WhitespacesNode<'tree>>,
    ///`comma` — required
    pub child_8: NodeSlot<'tree, CommaNode<'tree>>,
    ///`whitespaces` — optional
    pub child_9: Option<WhitespacesNode<'tree>>,
    ///`types_group` — required
    pub child_10: NodeSlot<'tree, TypesGroupNode<'tree>>,
    ///`newline` — required
    pub child_11: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `unsupported_dependent_tier` nodes.
#[derive(Debug)]
pub struct UnsupportedDependentTierChildren<'tree> {
    ///`tier_sep` — required
    pub child_1: NodeSlot<'tree, TierSepNode<'tree>>,
    ///`newline` — required
    pub child_3: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `unsupported_header` nodes.
#[derive(Debug)]
pub struct UnsupportedHeaderChildren<'tree> {
    ///`header_sep` — required
    pub child_1: NodeSlot<'tree, HeaderSepNode<'tree>>,
    ///`rest_of_line` — required
    pub child_2: NodeSlot<'tree, RestOfLineNode<'tree>>,
    ///`newline` — required
    pub child_3: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `unsupported_line` nodes.
#[derive(Debug)]
pub struct UnsupportedLineChildren<'tree> {
    ///`newline` — required
    pub child_1: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `utf8_header` nodes.
#[derive(Debug)]
pub struct Utf8HeaderChildren<'tree> {
    ///`newline` — required
    pub child_1: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `utterance` nodes.
#[derive(Debug)]
pub struct UtteranceChildren<'tree> {
    ///`main_tier` — required
    pub child_0: NodeSlot<'tree, MainTierNode<'tree>>,
}
///Extracted children for `utterance_end` nodes.
#[derive(Debug)]
pub struct UtteranceEndChildren<'tree> {
    ///`terminator` — optional
    pub child_0: Option<TerminatorNode<'tree>>,
    ///`final_codes` — optional
    pub child_1: Option<FinalCodesNode<'tree>>,
    ///`whitespaces` — optional
    pub child_3: Option<WhitespacesNode<'tree>>,
    ///`newline` — required
    pub child_4: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `videos_header` nodes.
#[derive(Debug)]
pub struct VideosHeaderChildren<'tree> {
    ///`videos_prefix` — required
    pub child_0: NodeSlot<'tree, VideosPrefixNode<'tree>>,
    ///`header_sep` — required
    pub child_1: NodeSlot<'tree, HeaderSepNode<'tree>>,
    ///`free_text` — required
    pub child_2: NodeSlot<'tree, FreeTextNode<'tree>>,
    ///`newline` — required
    pub child_3: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `warning_header` nodes.
#[derive(Debug)]
pub struct WarningHeaderChildren<'tree> {
    ///`warning_prefix` — required
    pub child_0: NodeSlot<'tree, WarningPrefixNode<'tree>>,
    ///`header_sep` — required
    pub child_1: NodeSlot<'tree, HeaderSepNode<'tree>>,
    ///`free_text` — required
    pub child_2: NodeSlot<'tree, FreeTextNode<'tree>>,
    ///`newline` — required
    pub child_3: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `window_header` nodes.
#[derive(Debug)]
pub struct WindowHeaderChildren<'tree> {
    ///`window_prefix` — required
    pub child_0: NodeSlot<'tree, WindowPrefixNode<'tree>>,
    ///`header_sep` — required
    pub child_1: NodeSlot<'tree, HeaderSepNode<'tree>>,
    ///`free_text` — required
    pub child_2: NodeSlot<'tree, FreeTextNode<'tree>>,
    ///`newline` — required
    pub child_3: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `wor_dependent_tier` nodes.
#[derive(Debug)]
pub struct WorDependentTierChildren<'tree> {
    ///`wor_tier_prefix` — required
    pub child_0: NodeSlot<'tree, WorTierPrefixNode<'tree>>,
    ///`tier_sep` — required
    pub child_1: NodeSlot<'tree, TierSepNode<'tree>>,
    ///`wor_tier_body` — required
    pub child_2: NodeSlot<'tree, WorTierBodyNode<'tree>>,
}
///Extracted children for `wor_tier_body` nodes.
#[derive(Debug)]
pub struct WorTierBodyChildren<'tree> {
    ///`terminator` — optional
    pub child_2: Option<TerminatorNode<'tree>>,
    ///`newline` — required
    pub child_3: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Extracted children for `word_with_optional_annotations` nodes.
#[derive(Debug)]
pub struct WordWithOptionalAnnotationsChildren<'tree> {
    ///`standalone_word` — required (field: `word`)
    pub word: NodeSlot<'tree, StandaloneWordNode<'tree>>,
    ///`base_annotations` — optional (field: `annotations`)
    pub annotations: Option<BaseAnnotationsNode<'tree>>,
}
///Extracted children for `x_dependent_tier` nodes.
#[derive(Debug)]
pub struct XDependentTierChildren<'tree> {
    ///`tier_sep` — required
    pub child_1: NodeSlot<'tree, TierSepNode<'tree>>,
    ///`text_with_bullets` — required
    pub child_2: NodeSlot<'tree, TextWithBulletsNode<'tree>>,
    ///`newline` — required
    pub child_3: NodeSlot<'tree, NewlineNode<'tree>>,
}
///Semantic payload for `_id_demographic_fields` — 4 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct IdDemographicFieldsPayload<'tree> {
    ///`id_age` — nested rule `id_age`
    pub child_1: Option<IdAgeNode<'tree>>,
    ///`id_sex` — enum: male | female
    pub child_5: Option<IdSexNode<'tree>>,
    ///`id_group` — free text
    pub child_9: Option<IdGroupNode<'tree>>,
    ///`id_ses` — nested rule `id_ses`
    pub child_13: Option<IdSesNode<'tree>>,
}
///Semantic payload for `_id_identity_fields` — 3 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct IdIdentityFieldsPayload<'tree> {
    ///`id_languages` — nested rule `id_languages`
    pub child_0: Option<IdLanguagesNode<'tree>>,
    ///`id_corpus` — free text
    pub child_3: Option<IdCorpusNode<'tree>>,
    ///`id_speaker` — free text
    pub child_6: Option<IdSpeakerNode<'tree>>,
}
///Semantic payload for `_id_role_fields` — 3 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct IdRoleFieldsPayload<'tree> {
    ///`id_role` — free text
    pub child_0: Option<IdRoleNode<'tree>>,
    ///`id_education` — free text
    pub child_3: Option<IdEducationNode<'tree>>,
    ///`id_custom_field` — free text
    pub child_7: Option<IdCustomFieldNode<'tree>>,
}
///Semantic payload for `activities_header` — 1 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct ActivitiesHeaderPayload<'tree> {
    ///`free_text` — nested rule `free_text`
    pub child_2: Option<FreeTextNode<'tree>>,
}
///Semantic payload for `alt_annotation` — 1 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct AltAnnotationPayload<'tree> {
    ///`annotation_content` — free text
    pub text: Option<AnnotationContentNode<'tree>>,
}
///Semantic payload for `bck_header` — 1 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct BckHeaderPayload<'tree> {
    ///`free_text` — nested rule `free_text`
    pub child_2: Option<FreeTextNode<'tree>>,
}
///Semantic payload for `birth_of_header` — 3 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct BirthOfHeaderPayload<'tree> {
    ///`header_gap` — nested rule `header_gap`
    pub child_1: Option<HeaderGapNode<'tree>>,
    ///`speaker` — free text
    pub child_2: Option<SpeakerNode<'tree>>,
    ///`date_contents` — nested rule `date_contents`
    pub child_4: Option<DateContentsNode<'tree>>,
}
///Semantic payload for `birthplace_of_header` — 3 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct BirthplaceOfHeaderPayload<'tree> {
    ///`header_gap` — nested rule `header_gap`
    pub child_1: Option<HeaderGapNode<'tree>>,
    ///`speaker` — free text
    pub child_2: Option<SpeakerNode<'tree>>,
    ///`free_text` — nested rule `free_text`
    pub child_4: Option<FreeTextNode<'tree>>,
}
///Semantic payload for `bullet` — 2 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct BulletPayload<'tree> {
    ///`bullet_timestamp` — integer
    pub start_time: Option<BulletTimestampNode<'tree>>,
    ///`bullet_timestamp` — integer
    pub end_time: Option<BulletTimestampNode<'tree>>,
}
///Semantic payload for `color_words_header` — 1 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct ColorWordsHeaderPayload<'tree> {
    ///`free_text` — nested rule `free_text`
    pub child_2: Option<FreeTextNode<'tree>>,
}
///Semantic payload for `com_dependent_tier` — 1 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct ComDependentTierPayload<'tree> {
    ///`text_with_bullets_and_pics` — nested rule `text_with_bullets_and_pics`
    pub child_2: Option<TextWithBulletsAndPicsNode<'tree>>,
}
///Semantic payload for `comment_header` — 1 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct CommentHeaderPayload<'tree> {
    ///`text_with_bullets_and_pics` — nested rule `text_with_bullets_and_pics`
    pub child_2: Option<TextWithBulletsAndPicsNode<'tree>>,
}
///Semantic payload for `date_header` — 1 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct DateHeaderPayload<'tree> {
    ///`date_contents` — nested rule `date_contents`
    pub child_2: Option<DateContentsNode<'tree>>,
}
///Semantic payload for `event` — 2 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct EventPayload<'tree> {
    ///`event_marker` — opaque token (parse separately)
    pub child_0: Option<EventMarkerNode<'tree>>,
    ///`event_segment` — opaque token (parse separately)
    pub description: Option<EventSegmentNode<'tree>>,
}
///Semantic payload for `explanation_annotation` — 1 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct ExplanationAnnotationPayload<'tree> {
    ///`annotation_content` — free text
    pub text: Option<AnnotationContentNode<'tree>>,
}
///Semantic payload for `font_header` — 1 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct FontHeaderPayload<'tree> {
    ///`free_text` — nested rule `free_text`
    pub child_2: Option<FreeTextNode<'tree>>,
}
///Semantic payload for `full_document` — 3 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct FullDocumentPayload<'tree> {
    ///`utf8_header` — nested rule `utf8_header`
    pub child_0: Option<Utf8HeaderNode<'tree>>,
    ///`begin_header` — nested rule `begin_header`
    pub child_1: Option<BeginHeaderNode<'tree>>,
    ///`end_header` — nested rule `end_header`
    pub child_2: Option<EndHeaderNode<'tree>>,
}
///Semantic payload for `g_header` — 1 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct GHeaderPayload<'tree> {
    ///`free_text` — nested rule `free_text`
    pub child_2: Option<FreeTextNode<'tree>>,
}
///Semantic payload for `gra_contents` — 1 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct GraContentsPayload<'tree> {
    ///`gra_relation` — nested rule `gra_relation`
    pub child_0: Option<GraRelationNode<'tree>>,
}
///Semantic payload for `gra_dependent_tier` — 1 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct GraDependentTierPayload<'tree> {
    ///`gra_contents` — nested rule `gra_contents`
    pub child_2: Option<GraContentsNode<'tree>>,
}
///Semantic payload for `gra_relation` — 3 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct GraRelationPayload<'tree> {
    ///`gra_index` — integer
    pub index: Option<GraIndexNode<'tree>>,
    ///`gra_head` — integer
    pub head: Option<GraHeadNode<'tree>>,
    ///`gra_relation_name` — free text
    pub relation: Option<GraRelationNameNode<'tree>>,
}
///Semantic payload for `group_with_annotations` — 2 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct GroupWithAnnotationsPayload<'tree> {
    ///`contents` — nested rule `contents`
    pub content: Option<ContentsNode<'tree>>,
    ///`base_annotations` — nested rule `base_annotations`
    pub annotations: Option<BaseAnnotationsNode<'tree>>,
}
///Semantic payload for `id_contents` — 3 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct IdContentsPayload<'tree> {
    ///`_id_identity_fields` — nested rule `_id_identity_fields`
    pub child_0: Option<IdIdentityFieldsNode<'tree>>,
    ///`_id_demographic_fields` — nested rule `_id_demographic_fields`
    pub child_1: Option<IdDemographicFieldsNode<'tree>>,
    ///`_id_role_fields` — nested rule `_id_role_fields`
    pub child_2: Option<IdRoleFieldsNode<'tree>>,
}
///Semantic payload for `id_header` — 1 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct IdHeaderPayload<'tree> {
    ///`id_contents` — nested rule `id_contents`
    pub child_2: Option<IdContentsNode<'tree>>,
}
///Semantic payload for `l1_of_header` — 3 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct L1OfHeaderPayload<'tree> {
    ///`header_gap` — nested rule `header_gap`
    pub child_1: Option<HeaderGapNode<'tree>>,
    ///`speaker` — free text
    pub child_2: Option<SpeakerNode<'tree>>,
    ///`language_code` — free text
    pub child_4: Option<LanguageCodeNode<'tree>>,
}
///Semantic payload for `langcode` — 1 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct LangcodePayload<'tree> {
    ///`language_code` — free text
    pub code: Option<LanguageCodeNode<'tree>>,
}
///Semantic payload for `languages_contents` — 1 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct LanguagesContentsPayload<'tree> {
    ///`language_code` — free text
    pub child_0: Option<LanguageCodeNode<'tree>>,
}
///Semantic payload for `languages_header` — 1 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct LanguagesHeaderPayload<'tree> {
    ///`languages_contents` — nested rule `languages_contents`
    pub child_2: Option<LanguagesContentsNode<'tree>>,
}
///Semantic payload for `location_header` — 1 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct LocationHeaderPayload<'tree> {
    ///`free_text` — nested rule `free_text`
    pub child_2: Option<FreeTextNode<'tree>>,
}
///Semantic payload for `long_feature_begin` — 1 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct LongFeatureBeginPayload<'tree> {
    ///`long_feature_label` — free text
    pub child_2: Option<LongFeatureLabelNode<'tree>>,
}
///Semantic payload for `long_feature_end` — 1 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct LongFeatureEndPayload<'tree> {
    ///`long_feature_label` — free text
    pub child_2: Option<LongFeatureLabelNode<'tree>>,
}
///Semantic payload for `main_pho_group` — 3 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct MainPhoGroupPayload<'tree> {
    ///`pho_begin_group` — opaque token (parse separately)
    pub child_0: Option<PhoBeginGroupNode<'tree>>,
    ///`contents` — nested rule `contents`
    pub child_1: Option<ContentsNode<'tree>>,
    ///`pho_end_group` — opaque token (parse separately)
    pub child_2: Option<PhoEndGroupNode<'tree>>,
}
///Semantic payload for `main_sin_group` — 3 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct MainSinGroupPayload<'tree> {
    ///`sin_begin_group` — opaque token (parse separately)
    pub child_0: Option<SinBeginGroupNode<'tree>>,
    ///`contents` — nested rule `contents`
    pub child_1: Option<ContentsNode<'tree>>,
    ///`sin_end_group` — opaque token (parse separately)
    pub child_2: Option<SinEndGroupNode<'tree>>,
}
///Semantic payload for `main_tier` — 2 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct MainTierPayload<'tree> {
    ///`speaker` — free text
    pub speaker: Option<SpeakerNode<'tree>>,
    ///`tier_body` — nested rule `tier_body`
    pub child_4: Option<TierBodyNode<'tree>>,
}
///Semantic payload for `media_contents` — 2 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct MediaContentsPayload<'tree> {
    ///`media_filename` — nested rule `media_filename`
    pub child_0: Option<MediaFilenameNode<'tree>>,
    ///`media_type` — enum: video | audio | missing
    pub child_3: Option<MediaTypeNode<'tree>>,
}
///Semantic payload for `media_header` — 1 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct MediaHeaderPayload<'tree> {
    ///`media_contents` — nested rule `media_contents`
    pub child_2: Option<MediaContentsNode<'tree>>,
}
///Semantic payload for `mod_dependent_tier` — 1 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct ModDependentTierPayload<'tree> {
    ///`pho_groups` — nested rule `pho_groups`
    pub child_2: Option<PhoGroupsNode<'tree>>,
}
///Semantic payload for `mor_content` — 1 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct MorContentPayload<'tree> {
    ///`mor_word` — nested rule `mor_word`
    pub main: Option<MorWordNode<'tree>>,
}
///Semantic payload for `mor_dependent_tier` — 1 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct MorDependentTierPayload<'tree> {
    ///`mor_contents` — nested rule `mor_contents`
    pub child_2: Option<MorContentsNode<'tree>>,
}
///Semantic payload for `mor_feature` — 1 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct MorFeaturePayload<'tree> {
    ///`mor_feature_value` — free text
    pub child_1: Option<MorFeatureValueNode<'tree>>,
}
///Semantic payload for `mor_post_clitic` — 1 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct MorPostCliticPayload<'tree> {
    ///`mor_word` — nested rule `mor_word`
    pub child_1: Option<MorWordNode<'tree>>,
}
///Semantic payload for `mor_word` — 2 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct MorWordPayload<'tree> {
    ///`mor_pos` — free text
    pub child_0: Option<MorPosNode<'tree>>,
    ///`mor_lemma` — free text
    pub child_2: Option<MorLemmaNode<'tree>>,
}
///Semantic payload for `nonvocal_begin` — 1 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct NonvocalBeginPayload<'tree> {
    ///`long_feature_label` — free text
    pub child_2: Option<LongFeatureLabelNode<'tree>>,
}
///Semantic payload for `nonvocal_end` — 1 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct NonvocalEndPayload<'tree> {
    ///`long_feature_label` — free text
    pub child_2: Option<LongFeatureLabelNode<'tree>>,
}
///Semantic payload for `nonvocal_simple` — 1 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct NonvocalSimplePayload<'tree> {
    ///`long_feature_label` — free text
    pub child_2: Option<LongFeatureLabelNode<'tree>>,
}
///Semantic payload for `nonword_with_optional_annotations` — 1 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct NonwordWithOptionalAnnotationsPayload<'tree> {
    ///`nonword` — nested rule `nonword`
    pub nonword: Option<NonwordNode<'tree>>,
}
///Semantic payload for `number_header` — 1 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct NumberHeaderPayload<'tree> {
    ///`number_option` — enum: 1 | 2 | 3 | 4 | 5 | more | audience
    pub child_2: Option<NumberOptionNode<'tree>>,
}
///Semantic payload for `options_contents` — 1 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct OptionsContentsPayload<'tree> {
    ///`option_name` — enum: CA | NoAlign
    pub child_0: Option<OptionNameNode<'tree>>,
}
///Semantic payload for `options_header` — 1 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct OptionsHeaderPayload<'tree> {
    ///`options_contents` — nested rule `options_contents`
    pub child_2: Option<OptionsContentsNode<'tree>>,
}
///Semantic payload for `other_spoken_event` — 2 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct OtherSpokenEventPayload<'tree> {
    ///`speaker` — free text
    pub child_2: Option<SpeakerNode<'tree>>,
    ///`standalone_word` — nested rule `standalone_word`
    pub child_4: Option<StandaloneWordNode<'tree>>,
}
///Semantic payload for `page_header` — 1 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct PageHeaderPayload<'tree> {
    ///`page_number` — integer
    pub child_2: Option<PageNumberNode<'tree>>,
}
///Semantic payload for `para_annotation` — 1 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct ParaAnnotationPayload<'tree> {
    ///`annotation_content` — free text
    pub text: Option<AnnotationContentNode<'tree>>,
}
///Semantic payload for `participant` — 1 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct ParticipantPayload<'tree> {
    ///`speaker` — free text
    pub code: Option<SpeakerNode<'tree>>,
}
///Semantic payload for `participants_contents` — 1 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct ParticipantsContentsPayload<'tree> {
    ///`participant` — nested rule `participant`
    pub child_0: Option<ParticipantNode<'tree>>,
}
///Semantic payload for `participants_header` — 1 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct ParticipantsHeaderPayload<'tree> {
    ///`participants_contents` — nested rule `participants_contents`
    pub child_2: Option<ParticipantsContentsNode<'tree>>,
}
///Semantic payload for `percent_annotation` — 1 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct PercentAnnotationPayload<'tree> {
    ///`annotation_content` — free text
    pub text: Option<AnnotationContentNode<'tree>>,
}
///Semantic payload for `pho_dependent_tier` — 1 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct PhoDependentTierPayload<'tree> {
    ///`pho_groups` — nested rule `pho_groups`
    pub child_2: Option<PhoGroupsNode<'tree>>,
}
///Semantic payload for `pho_grouped_content` — 1 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct PhoGroupedContentPayload<'tree> {
    ///`pho_words` — nested rule `pho_words`
    pub child_0: Option<PhoWordsNode<'tree>>,
}
///Semantic payload for `pho_groups` — 1 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct PhoGroupsPayload<'tree> {
    ///`pho_group` — nested rule `pho_group`
    pub child_0: Option<PhoGroupNode<'tree>>,
}
///Semantic payload for `pho_words` — 1 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct PhoWordsPayload<'tree> {
    ///`pho_word` — free text
    pub child_0: Option<PhoWordNode<'tree>>,
}
///Semantic payload for `pid_header` — 1 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct PidHeaderPayload<'tree> {
    ///`free_text` — nested rule `free_text`
    pub child_2: Option<FreeTextNode<'tree>>,
}
///Semantic payload for `postcode` — 1 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct PostcodePayload<'tree> {
    ///`annotation_content` — free text
    pub code: Option<AnnotationContentNode<'tree>>,
}
///Semantic payload for `quotation` — 3 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct QuotationPayload<'tree> {
    ///`left_double_quote` — opaque token (parse separately)
    pub child_0: Option<LeftDoubleQuoteNode<'tree>>,
    ///`contents` — nested rule `contents`
    pub child_1: Option<ContentsNode<'tree>>,
    ///`right_double_quote` — opaque token (parse separately)
    pub child_2: Option<RightDoubleQuoteNode<'tree>>,
}
///Semantic payload for `recording_quality_header` — 1 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct RecordingQualityHeaderPayload<'tree> {
    ///`recording_quality_option` — enum: 1 | 2 | 3 | 4 | 5
    pub child_2: Option<RecordingQualityOptionNode<'tree>>,
}
///Semantic payload for `room_layout_header` — 1 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct RoomLayoutHeaderPayload<'tree> {
    ///`free_text` — nested rule `free_text`
    pub child_2: Option<FreeTextNode<'tree>>,
}
///Semantic payload for `shortening` — 1 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct ShorteningPayload<'tree> {
    ///`word_segment` — opaque token (parse separately)
    pub child_0: Option<WordSegmentNode<'tree>>,
}
///Semantic payload for `sin_dependent_tier` — 1 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct SinDependentTierPayload<'tree> {
    ///`sin_groups` — nested rule `sin_groups`
    pub child_2: Option<SinGroupsNode<'tree>>,
}
///Semantic payload for `sin_grouped_content` — 1 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct SinGroupedContentPayload<'tree> {
    ///`sin_word` — nested rule `sin_word`
    pub child_0: Option<SinWordNode<'tree>>,
}
///Semantic payload for `sin_groups` — 1 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct SinGroupsPayload<'tree> {
    ///`sin_group` — nested rule `sin_group`
    pub child_0: Option<SinGroupNode<'tree>>,
}
///Semantic payload for `situation_header` — 1 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct SituationHeaderPayload<'tree> {
    ///`free_text` — nested rule `free_text`
    pub child_2: Option<FreeTextNode<'tree>>,
}
///Semantic payload for `standalone_word` — 4 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct StandaloneWordPayload<'tree> {
    ///`word_body` — nested rule `word_body`
    pub child_0: Option<WordBodyNode<'tree>>,
    ///`form_marker` — opaque token (parse separately)
    pub child_1: Option<FormMarkerNode<'tree>>,
    ///`word_lang_suffix` — opaque token (parse separately)
    pub child_2: Option<WordLangSuffixNode<'tree>>,
    ///`pos_tag` — nested rule `pos_tag`
    pub child_3: Option<PosTagNode<'tree>>,
}
///Semantic payload for `t_header` — 1 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct THeaderPayload<'tree> {
    ///`free_text` — nested rule `free_text`
    pub child_2: Option<FreeTextNode<'tree>>,
}
///Semantic payload for `tape_location_header` — 1 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct TapeLocationHeaderPayload<'tree> {
    ///`free_text` — nested rule `free_text`
    pub child_2: Option<FreeTextNode<'tree>>,
}
///Semantic payload for `thumbnail_header` — 1 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct ThumbnailHeaderPayload<'tree> {
    ///`free_text` — nested rule `free_text`
    pub child_2: Option<FreeTextNode<'tree>>,
}
///Semantic payload for `tier_body` — 2 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct TierBodyPayload<'tree> {
    ///`contents` — nested rule `contents`
    pub content: Option<ContentsNode<'tree>>,
    ///`utterance_end` — nested rule `utterance_end`
    pub ending: Option<UtteranceEndNode<'tree>>,
}
///Semantic payload for `time_duration_header` — 1 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct TimeDurationHeaderPayload<'tree> {
    ///`time_duration_contents` — nested rule `time_duration_contents`
    pub child_2: Option<TimeDurationContentsNode<'tree>>,
}
///Semantic payload for `time_start_header` — 1 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct TimeStartHeaderPayload<'tree> {
    ///`time_duration_contents` — nested rule `time_duration_contents`
    pub child_2: Option<TimeDurationContentsNode<'tree>>,
}
///Semantic payload for `transcriber_header` — 1 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct TranscriberHeaderPayload<'tree> {
    ///`free_text` — nested rule `free_text`
    pub child_2: Option<FreeTextNode<'tree>>,
}
///Semantic payload for `transcription_header` — 1 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct TranscriptionHeaderPayload<'tree> {
    ///`transcription_option` — enum: eye_dialect | partial | full | detailed | coarse | checked | anonymized
    pub child_2: Option<TranscriptionOptionNode<'tree>>,
}
///Semantic payload for `types_header` — 3 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct TypesHeaderPayload<'tree> {
    ///`types_design` — free text
    pub child_2: Option<TypesDesignNode<'tree>>,
    ///`types_activity` — free text
    pub child_6: Option<TypesActivityNode<'tree>>,
    ///`types_group` — free text
    pub child_10: Option<TypesGroupNode<'tree>>,
}
///Semantic payload for `unsupported_header` — 1 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct UnsupportedHeaderPayload<'tree> {
    ///`rest_of_line` — free text
    pub child_1: Option<RestOfLineNode<'tree>>,
}
///Semantic payload for `utterance` — 1 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct UtterancePayload<'tree> {
    ///`main_tier` — nested rule `main_tier`
    pub child_0: Option<MainTierNode<'tree>>,
}
///Semantic payload for `utterance_end` — 2 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct UtteranceEndPayload<'tree> {
    ///`terminator` — enum: . | ? | !
    pub child_0: Option<TerminatorNode<'tree>>,
    ///`final_codes` — nested rule `final_codes`
    pub child_1: Option<FinalCodesNode<'tree>>,
}
///Semantic payload for `videos_header` — 1 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct VideosHeaderPayload<'tree> {
    ///`free_text` — nested rule `free_text`
    pub child_2: Option<FreeTextNode<'tree>>,
}
///Semantic payload for `warning_header` — 1 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct WarningHeaderPayload<'tree> {
    ///`free_text` — nested rule `free_text`
    pub child_2: Option<FreeTextNode<'tree>>,
}
///Semantic payload for `window_header` — 1 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct WindowHeaderPayload<'tree> {
    ///`free_text` — nested rule `free_text`
    pub child_2: Option<FreeTextNode<'tree>>,
}
///Semantic payload for `wor_dependent_tier` — 1 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct WorDependentTierPayload<'tree> {
    ///`wor_tier_body` — nested rule `wor_tier_body`
    pub child_2: Option<WorTierBodyNode<'tree>>,
}
///Semantic payload for `wor_tier_body` — 1 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct WorTierBodyPayload<'tree> {
    ///`terminator` — enum: . | ? | !
    pub child_0: Option<TerminatorNode<'tree>>,
}
///Semantic payload for `word_with_optional_annotations` — 1 payload field(s), structural children stripped.
#[derive(Debug)]
pub struct WordWithOptionalAnnotationsPayload<'tree> {
    ///`standalone_word` — nested rule `standalone_word`
    pub word: Option<StandaloneWordNode<'tree>>,
}
///Validated values for `id_sex` — 2 known + catch-all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdSexValue {
    Male,
    Female,
    /// Unknown value — not in the known set.
    Other(String),
}
impl IdSexValue {
    /// Parse a string into this validated enum.
    #[must_use]
    pub fn from_text(s: &str) -> Self {
        match s {
            "male" => Self::Male,
            "female" => Self::Female,
            other => Self::Other(other.to_string()),
        }
    }
    /// Whether this is a known (non-Other) value.
    #[must_use]
    pub fn is_known(&self) -> bool {
        !matches!(self, Self::Other(_))
    }
}
///Validated values for `media_type` — 3 known + catch-all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediaTypeValue {
    Video,
    Audio,
    Missing,
    /// Unknown value — not in the known set.
    Other(String),
}
impl MediaTypeValue {
    /// Parse a string into this validated enum.
    #[must_use]
    pub fn from_text(s: &str) -> Self {
        match s {
            "video" => Self::Video,
            "audio" => Self::Audio,
            "missing" => Self::Missing,
            other => Self::Other(other.to_string()),
        }
    }
    /// Whether this is a known (non-Other) value.
    #[must_use]
    pub fn is_known(&self) -> bool {
        !matches!(self, Self::Other(_))
    }
}
///Validated values for `number_option` — 7 known + catch-all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NumberOptionValue {
    V1,
    V2,
    V3,
    V4,
    V5,
    More,
    Audience,
    /// Unknown value — not in the known set.
    Other(String),
}
impl NumberOptionValue {
    /// Parse a string into this validated enum.
    #[must_use]
    pub fn from_text(s: &str) -> Self {
        match s {
            "1" => Self::V1,
            "2" => Self::V2,
            "3" => Self::V3,
            "4" => Self::V4,
            "5" => Self::V5,
            "more" => Self::More,
            "audience" => Self::Audience,
            other => Self::Other(other.to_string()),
        }
    }
    /// Whether this is a known (non-Other) value.
    #[must_use]
    pub fn is_known(&self) -> bool {
        !matches!(self, Self::Other(_))
    }
}
///Validated values for `option_name` — 2 known + catch-all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptionNameValue {
    CA,
    NoAlign,
    /// Unknown value — not in the known set.
    Other(String),
}
impl OptionNameValue {
    /// Parse a string into this validated enum.
    #[must_use]
    pub fn from_text(s: &str) -> Self {
        match s {
            "CA" => Self::CA,
            "NoAlign" => Self::NoAlign,
            other => Self::Other(other.to_string()),
        }
    }
    /// Whether this is a known (non-Other) value.
    #[must_use]
    pub fn is_known(&self) -> bool {
        !matches!(self, Self::Other(_))
    }
}
///Validated values for `recording_quality_option` — 5 known + catch-all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordingQualityOptionValue {
    V1,
    V2,
    V3,
    V4,
    V5,
    /// Unknown value — not in the known set.
    Other(String),
}
impl RecordingQualityOptionValue {
    /// Parse a string into this validated enum.
    #[must_use]
    pub fn from_text(s: &str) -> Self {
        match s {
            "1" => Self::V1,
            "2" => Self::V2,
            "3" => Self::V3,
            "4" => Self::V4,
            "5" => Self::V5,
            other => Self::Other(other.to_string()),
        }
    }
    /// Whether this is a known (non-Other) value.
    #[must_use]
    pub fn is_known(&self) -> bool {
        !matches!(self, Self::Other(_))
    }
}
///Validated values for `transcription_option` — 7 known + catch-all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TranscriptionOptionValue {
    EyeDialect,
    Partial,
    Full,
    Detailed,
    Coarse,
    Checked,
    Anonymized,
    /// Unknown value — not in the known set.
    Other(String),
}
impl TranscriptionOptionValue {
    /// Parse a string into this validated enum.
    #[must_use]
    pub fn from_text(s: &str) -> Self {
        match s {
            "eye_dialect" => Self::EyeDialect,
            "partial" => Self::Partial,
            "full" => Self::Full,
            "detailed" => Self::Detailed,
            "coarse" => Self::Coarse,
            "checked" => Self::Checked,
            "anonymized" => Self::Anonymized,
            other => Self::Other(other.to_string()),
        }
    }
    /// Whether this is a known (non-Other) value.
    #[must_use]
    pub fn is_known(&self) -> bool {
        !matches!(self, Self::Other(_))
    }
}
/// CST traversal trait — the complete parser interface.
///
/// Each extraction method walks a node's children in production order
/// and returns a struct where every position is faithfully represented
/// as a `NodeSlot` (for required children) or `Option<T>` (for optional).
///
/// Default implementations handle the mechanical work. Override
/// specific methods where you need custom recovery logic.
///
/// `&mut self` carries your parser state.
pub trait GrammarTraversal {
    ///Extract children from a `_id_demographic_fields` node.
    ///
    ///Production (children in order):
    ///- `whitespaces` [optional]
    ///- `id_age` [optional]
    ///- `whitespaces` [optional]
    ///- `pipe` [required]
    ///- `whitespaces` [optional]
    ///- `id_sex` [optional]
    ///- `whitespaces` [optional]
    ///- `pipe` [required]
    ///- `whitespaces` [optional]
    ///- `id_group` [optional]
    ///- `whitespaces` [optional]
    ///- `pipe` [required]
    ///- `whitespaces` [optional]
    ///- `id_ses` [optional]
    ///- `whitespaces` [optional]
    ///- `pipe` [required]
    fn extract__id_demographic_fields<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> IdDemographicFieldsChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: Option<WhitespacesNode<'tree>> = None;
        let mut child_1: Option<IdAgeNode<'tree>> = None;
        let mut child_2: Option<WhitespacesNode<'tree>> = None;
        let mut child_3: NodeSlot<'tree, PipeNode<'tree>> = NodeSlot::Absent;
        let mut child_4: Option<WhitespacesNode<'tree>> = None;
        let mut child_5: Option<IdSexNode<'tree>> = None;
        let mut child_6: Option<WhitespacesNode<'tree>> = None;
        let mut child_7: NodeSlot<'tree, PipeNode<'tree>> = NodeSlot::Absent;
        let mut child_8: Option<WhitespacesNode<'tree>> = None;
        let mut child_9: Option<IdGroupNode<'tree>> = None;
        let mut child_10: Option<WhitespacesNode<'tree>> = None;
        let mut child_11: NodeSlot<'tree, PipeNode<'tree>> = NodeSlot::Absent;
        let mut child_12: Option<WhitespacesNode<'tree>> = None;
        let mut child_13: Option<IdSesNode<'tree>> = None;
        let mut child_14: Option<WhitespacesNode<'tree>> = None;
        let mut child_15: NodeSlot<'tree, PipeNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                if child.kind() == "whitespaces" && !child.is_error() {
                    child_0 = Some(WhitespacesNode(child));
                    idx += 1;
                }
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                if child.kind() == "id_age" && !child.is_error() {
                    child_1 = Some(IdAgeNode(child));
                    idx += 1;
                }
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                if child.kind() == "whitespaces" && !child.is_error() {
                    child_2 = Some(WhitespacesNode(child));
                    idx += 1;
                }
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "pipe", PipeNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                if child.kind() == "whitespaces" && !child.is_error() {
                    child_4 = Some(WhitespacesNode(child));
                    idx += 1;
                }
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                if child.kind() == "id_sex" && !child.is_error() {
                    child_5 = Some(IdSexNode(child));
                    idx += 1;
                }
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                if child.kind() == "whitespaces" && !child.is_error() {
                    child_6 = Some(WhitespacesNode(child));
                    idx += 1;
                }
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_7 = classify_child(child, "pipe", PipeNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                if child.kind() == "whitespaces" && !child.is_error() {
                    child_8 = Some(WhitespacesNode(child));
                    idx += 1;
                }
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                if child.kind() == "id_group" && !child.is_error() {
                    child_9 = Some(IdGroupNode(child));
                    idx += 1;
                }
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                if child.kind() == "whitespaces" && !child.is_error() {
                    child_10 = Some(WhitespacesNode(child));
                    idx += 1;
                }
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_11 = classify_child(child, "pipe", PipeNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                if child.kind() == "whitespaces" && !child.is_error() {
                    child_12 = Some(WhitespacesNode(child));
                    idx += 1;
                }
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                if child.kind() == "id_ses" && !child.is_error() {
                    child_13 = Some(IdSesNode(child));
                    idx += 1;
                }
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                if child.kind() == "whitespaces" && !child.is_error() {
                    child_14 = Some(WhitespacesNode(child));
                    idx += 1;
                }
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_15 = classify_child(child, "pipe", PipeNode);
                idx += 1;
            }
        }
        IdDemographicFieldsChildren {
            child_0,
            child_1,
            child_2,
            child_3,
            child_4,
            child_5,
            child_6,
            child_7,
            child_8,
            child_9,
            child_10,
            child_11,
            child_12,
            child_13,
            child_14,
            child_15,
        }
    }
    ///Extract children from a `_id_identity_fields` node.
    ///
    ///Production (children in order):
    ///- `id_languages` [required]
    ///- `pipe` [required]
    ///- `whitespaces` [optional]
    ///- `id_corpus` [optional]
    ///- `whitespaces` [optional]
    ///- `pipe` [required]
    ///- `id_speaker` [required]
    ///- `pipe` [required]
    fn extract__id_identity_fields<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> IdIdentityFieldsChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, IdLanguagesNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, PipeNode<'tree>> = NodeSlot::Absent;
        let mut child_2: Option<WhitespacesNode<'tree>> = None;
        let mut child_3: Option<IdCorpusNode<'tree>> = None;
        let mut child_4: Option<WhitespacesNode<'tree>> = None;
        let mut child_5: NodeSlot<'tree, PipeNode<'tree>> = NodeSlot::Absent;
        let mut child_6: NodeSlot<'tree, IdSpeakerNode<'tree>> = NodeSlot::Absent;
        let mut child_7: NodeSlot<'tree, PipeNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "id_languages", IdLanguagesNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "pipe", PipeNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                if child.kind() == "whitespaces" && !child.is_error() {
                    child_2 = Some(WhitespacesNode(child));
                    idx += 1;
                }
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                if child.kind() == "id_corpus" && !child.is_error() {
                    child_3 = Some(IdCorpusNode(child));
                    idx += 1;
                }
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                if child.kind() == "whitespaces" && !child.is_error() {
                    child_4 = Some(WhitespacesNode(child));
                    idx += 1;
                }
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_5 = classify_child(child, "pipe", PipeNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_6 = classify_child(child, "id_speaker", IdSpeakerNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_7 = classify_child(child, "pipe", PipeNode);
                idx += 1;
            }
        }
        IdIdentityFieldsChildren {
            child_0,
            child_1,
            child_2,
            child_3,
            child_4,
            child_5,
            child_6,
            child_7,
        }
    }
    ///Extract children from a `_id_role_fields` node.
    ///
    ///Production (children in order):
    ///- `id_role` [required]
    ///- `pipe` [required]
    ///- `whitespaces` [optional]
    ///- `id_education` [optional]
    ///- `whitespaces` [optional]
    ///- `pipe` [required]
    ///- `whitespaces` [optional]
    ///- `id_custom_field` [optional]
    ///- `whitespaces` [optional]
    ///- `pipe` [required]
    fn extract__id_role_fields<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> IdRoleFieldsChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, IdRoleNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, PipeNode<'tree>> = NodeSlot::Absent;
        let mut child_2: Option<WhitespacesNode<'tree>> = None;
        let mut child_3: Option<IdEducationNode<'tree>> = None;
        let mut child_4: Option<WhitespacesNode<'tree>> = None;
        let mut child_5: NodeSlot<'tree, PipeNode<'tree>> = NodeSlot::Absent;
        let mut child_6: Option<WhitespacesNode<'tree>> = None;
        let mut child_7: Option<IdCustomFieldNode<'tree>> = None;
        let mut child_8: Option<WhitespacesNode<'tree>> = None;
        let mut child_9: NodeSlot<'tree, PipeNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "id_role", IdRoleNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "pipe", PipeNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                if child.kind() == "whitespaces" && !child.is_error() {
                    child_2 = Some(WhitespacesNode(child));
                    idx += 1;
                }
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                if child.kind() == "id_education" && !child.is_error() {
                    child_3 = Some(IdEducationNode(child));
                    idx += 1;
                }
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                if child.kind() == "whitespaces" && !child.is_error() {
                    child_4 = Some(WhitespacesNode(child));
                    idx += 1;
                }
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_5 = classify_child(child, "pipe", PipeNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                if child.kind() == "whitespaces" && !child.is_error() {
                    child_6 = Some(WhitespacesNode(child));
                    idx += 1;
                }
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                if child.kind() == "id_custom_field" && !child.is_error() {
                    child_7 = Some(IdCustomFieldNode(child));
                    idx += 1;
                }
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                if child.kind() == "whitespaces" && !child.is_error() {
                    child_8 = Some(WhitespacesNode(child));
                    idx += 1;
                }
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_9 = classify_child(child, "pipe", PipeNode);
                idx += 1;
            }
        }
        IdRoleFieldsChildren {
            child_0,
            child_1,
            child_2,
            child_3,
            child_4,
            child_5,
            child_6,
            child_7,
            child_8,
            child_9,
        }
    }
    ///Extract children from a `act_dependent_tier` node.
    ///
    ///Production (children in order):
    ///- `act_tier_prefix` [required]
    ///- `tier_sep` [required]
    ///- `text_with_bullets` [required]
    ///- `newline` [required]
    fn extract_act_dependent_tier<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> ActDependentTierChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, ActTierPrefixNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, TierSepNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, TextWithBulletsNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "act_tier_prefix", ActTierPrefixNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "tier_sep", TierSepNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "text_with_bullets", TextWithBulletsNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        ActDependentTierChildren {
            child_0,
            child_1,
            child_2,
            child_3,
        }
    }
    ///Extract children from a `activities_header` node.
    ///
    ///Production (children in order):
    ///- `activities_prefix` [required]
    ///- `header_sep` [required]
    ///- `free_text` [required]
    ///- `newline` [required]
    fn extract_activities_header<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> ActivitiesHeaderChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, ActivitiesPrefixNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, HeaderSepNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, FreeTextNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "activities_prefix", ActivitiesPrefixNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "header_sep", HeaderSepNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "free_text", FreeTextNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        ActivitiesHeaderChildren {
            child_0,
            child_1,
            child_2,
            child_3,
        }
    }
    ///Extract children from a `add_dependent_tier` node.
    ///
    ///Production (children in order):
    ///- `add_tier_prefix` [required]
    ///- `tier_sep` [required]
    ///- `text_with_bullets` [required]
    ///- `newline` [required]
    fn extract_add_dependent_tier<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> AddDependentTierChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, AddTierPrefixNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, TierSepNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, TextWithBulletsNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "add_tier_prefix", AddTierPrefixNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "tier_sep", TierSepNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "text_with_bullets", TextWithBulletsNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        AddDependentTierChildren {
            child_0,
            child_1,
            child_2,
            child_3,
        }
    }
    ///Extract children from a `alt_annotation` node.
    ///
    ///Production (children in order):
    ///- `(terminal)` [required]
    ///- `space` [required]
    ///- `annotation_content` (field: `text`) [required]
    ///- `right_bracket` [required]
    fn extract_alt_annotation<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> AltAnnotationChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_1: NodeSlot<'tree, SpaceNode<'tree>> = NodeSlot::Absent;
        let mut text: NodeSlot<'tree, AnnotationContentNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, RightBracketNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            idx += 1;
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "space", SpaceNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                text = classify_child(child, "annotation_content", AnnotationContentNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "right_bracket", RightBracketNode);
                idx += 1;
            }
        }
        AltAnnotationChildren {
            child_1,
            text,
            child_3,
        }
    }
    ///Extract children from a `alt_dependent_tier` node.
    ///
    ///Production (children in order):
    ///- `alt_tier_prefix` [required]
    ///- `tier_sep` [required]
    ///- `text_with_bullets` [required]
    ///- `newline` [required]
    fn extract_alt_dependent_tier<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> AltDependentTierChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, AltTierPrefixNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, TierSepNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, TextWithBulletsNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "alt_tier_prefix", AltTierPrefixNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "tier_sep", TierSepNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "text_with_bullets", TextWithBulletsNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        AltDependentTierChildren {
            child_0,
            child_1,
            child_2,
            child_3,
        }
    }
    ///Extract children from a `bck_header` node.
    ///
    ///Production (children in order):
    ///- `bck_prefix` [required]
    ///- `header_sep` [required]
    ///- `free_text` [required]
    ///- `newline` [required]
    fn extract_bck_header<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> BckHeaderChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, BckPrefixNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, HeaderSepNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, FreeTextNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "bck_prefix", BckPrefixNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "header_sep", HeaderSepNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "free_text", FreeTextNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        BckHeaderChildren {
            child_0,
            child_1,
            child_2,
            child_3,
        }
    }
    ///Extract children from a `begin_header` node.
    ///
    ///Production (children in order):
    ///- `(terminal)` [required]
    ///- `newline` [required]
    fn extract_begin_header<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> BeginHeaderChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_1: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            idx += 1;
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        BeginHeaderChildren { child_1 }
    }
    ///Extract children from a `bg_header` node.
    ///
    ///Production (children in order):
    ///- `bg_prefix` [required]
    ///- `(terminal)` [optional]
    ///- `newline` [required]
    fn extract_bg_header<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> BgHeaderChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, BgPrefixNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "bg_prefix", BgPrefixNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            idx += 1;
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        BgHeaderChildren { child_0, child_2 }
    }
    ///Extract children from a `birth_of_header` node.
    ///
    ///Production (children in order):
    ///- `birth_of_prefix` [required]
    ///- `header_gap` [optional]
    ///- `speaker` [required]
    ///- `header_sep` [required]
    ///- `date_contents` [required]
    ///- `newline` [required]
    fn extract_birth_of_header<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> BirthOfHeaderChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, BirthOfPrefixNode<'tree>> = NodeSlot::Absent;
        let mut child_1: Option<HeaderGapNode<'tree>> = None;
        let mut child_2: NodeSlot<'tree, SpeakerNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, HeaderSepNode<'tree>> = NodeSlot::Absent;
        let mut child_4: NodeSlot<'tree, DateContentsNode<'tree>> = NodeSlot::Absent;
        let mut child_5: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "birth_of_prefix", BirthOfPrefixNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                if child.kind() == "header_gap" && !child.is_error() {
                    child_1 = Some(HeaderGapNode(child));
                    idx += 1;
                }
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "speaker", SpeakerNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "header_sep", HeaderSepNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_4 = classify_child(child, "date_contents", DateContentsNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_5 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        BirthOfHeaderChildren {
            child_0,
            child_1,
            child_2,
            child_3,
            child_4,
            child_5,
        }
    }
    ///Extract children from a `birthplace_of_header` node.
    ///
    ///Production (children in order):
    ///- `birthplace_of_prefix` [required]
    ///- `header_gap` [optional]
    ///- `speaker` [required]
    ///- `header_sep` [required]
    ///- `free_text` [required]
    ///- `newline` [required]
    fn extract_birthplace_of_header<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> BirthplaceOfHeaderChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, BirthplaceOfPrefixNode<'tree>> = NodeSlot::Absent;
        let mut child_1: Option<HeaderGapNode<'tree>> = None;
        let mut child_2: NodeSlot<'tree, SpeakerNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, HeaderSepNode<'tree>> = NodeSlot::Absent;
        let mut child_4: NodeSlot<'tree, FreeTextNode<'tree>> = NodeSlot::Absent;
        let mut child_5: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "birthplace_of_prefix", BirthplaceOfPrefixNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                if child.kind() == "header_gap" && !child.is_error() {
                    child_1 = Some(HeaderGapNode(child));
                    idx += 1;
                }
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "speaker", SpeakerNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "header_sep", HeaderSepNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_4 = classify_child(child, "free_text", FreeTextNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_5 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        BirthplaceOfHeaderChildren {
            child_0,
            child_1,
            child_2,
            child_3,
            child_4,
            child_5,
        }
    }
    ///Extract children from a `blank_header` node.
    ///
    ///Production (children in order):
    ///- `blank_prefix` [required]
    ///- `newline` [required]
    fn extract_blank_header<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> BlankHeaderChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, BlankPrefixNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "blank_prefix", BlankPrefixNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        BlankHeaderChildren { child_0, child_1 }
    }
    ///Extract children from a `bullet` node.
    ///
    ///Production (children in order):
    ///- `bullet_start` [required]
    ///- `bullet_timestamp` (field: `start_time`) [required]
    ///- `(terminal)` [required]
    ///- `bullet_timestamp` (field: `end_time`) [required]
    ///- `bullet_end` [required]
    fn extract_bullet<'tree>(&mut self, node: tree_sitter::Node<'tree>) -> BulletChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, BulletStartNode<'tree>> = NodeSlot::Absent;
        let mut start_time: NodeSlot<'tree, BulletTimestampNode<'tree>> = NodeSlot::Absent;
        let mut end_time: NodeSlot<'tree, BulletTimestampNode<'tree>> = NodeSlot::Absent;
        let mut child_4: NodeSlot<'tree, BulletEndNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "bullet_start", BulletStartNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                start_time = classify_child(child, "bullet_timestamp", BulletTimestampNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            idx += 1;
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                end_time = classify_child(child, "bullet_timestamp", BulletTimestampNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_4 = classify_child(child, "bullet_end", BulletEndNode);
                idx += 1;
            }
        }
        BulletChildren {
            child_0,
            start_time,
            end_time,
            child_4,
        }
    }
    ///Extract children from a `cod_dependent_tier` node.
    ///
    ///Production (children in order):
    ///- `cod_tier_prefix` [required]
    ///- `tier_sep` [required]
    ///- `text_with_bullets` [required]
    ///- `newline` [required]
    fn extract_cod_dependent_tier<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> CodDependentTierChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, CodTierPrefixNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, TierSepNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, TextWithBulletsNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "cod_tier_prefix", CodTierPrefixNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "tier_sep", TierSepNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "text_with_bullets", TextWithBulletsNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        CodDependentTierChildren {
            child_0,
            child_1,
            child_2,
            child_3,
        }
    }
    ///Extract children from a `coh_dependent_tier` node.
    ///
    ///Production (children in order):
    ///- `coh_tier_prefix` [required]
    ///- `tier_sep` [required]
    ///- `text_with_bullets` [required]
    ///- `newline` [required]
    fn extract_coh_dependent_tier<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> CohDependentTierChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, CohTierPrefixNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, TierSepNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, TextWithBulletsNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "coh_tier_prefix", CohTierPrefixNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "tier_sep", TierSepNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "text_with_bullets", TextWithBulletsNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        CohDependentTierChildren {
            child_0,
            child_1,
            child_2,
            child_3,
        }
    }
    ///Extract children from a `color_words_header` node.
    ///
    ///Production (children in order):
    ///- `color_words_prefix` [required]
    ///- `header_sep` [required]
    ///- `free_text` [required]
    ///- `newline` [required]
    fn extract_color_words_header<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> ColorWordsHeaderChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, ColorWordsPrefixNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, HeaderSepNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, FreeTextNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "color_words_prefix", ColorWordsPrefixNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "header_sep", HeaderSepNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "free_text", FreeTextNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        ColorWordsHeaderChildren {
            child_0,
            child_1,
            child_2,
            child_3,
        }
    }
    ///Extract children from a `com_dependent_tier` node.
    ///
    ///Production (children in order):
    ///- `com_tier_prefix` [required]
    ///- `tier_sep` [required]
    ///- `text_with_bullets_and_pics` [required]
    ///- `newline` [required]
    fn extract_com_dependent_tier<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> ComDependentTierChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, ComTierPrefixNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, TierSepNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, TextWithBulletsAndPicsNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "com_tier_prefix", ComTierPrefixNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "tier_sep", TierSepNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(
                    child,
                    "text_with_bullets_and_pics",
                    TextWithBulletsAndPicsNode,
                );
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        ComDependentTierChildren {
            child_0,
            child_1,
            child_2,
            child_3,
        }
    }
    ///Extract children from a `comment_header` node.
    ///
    ///Production (children in order):
    ///- `comment_prefix` [required]
    ///- `header_sep` [required]
    ///- `text_with_bullets_and_pics` [required]
    ///- `newline` [required]
    fn extract_comment_header<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> CommentHeaderChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, CommentPrefixNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, HeaderSepNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, TextWithBulletsAndPicsNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "comment_prefix", CommentPrefixNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "header_sep", HeaderSepNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(
                    child,
                    "text_with_bullets_and_pics",
                    TextWithBulletsAndPicsNode,
                );
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        CommentHeaderChildren {
            child_0,
            child_1,
            child_2,
            child_3,
        }
    }
    ///Extract children from a `date_header` node.
    ///
    ///Production (children in order):
    ///- `date_prefix` [required]
    ///- `header_sep` [required]
    ///- `date_contents` [required]
    ///- `newline` [required]
    fn extract_date_header<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> DateHeaderChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, DatePrefixNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, HeaderSepNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, DateContentsNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "date_prefix", DatePrefixNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "header_sep", HeaderSepNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "date_contents", DateContentsNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        DateHeaderChildren {
            child_0,
            child_1,
            child_2,
            child_3,
        }
    }
    ///Extract children from a `def_dependent_tier` node.
    ///
    ///Production (children in order):
    ///- `def_tier_prefix` [required]
    ///- `tier_sep` [required]
    ///- `text_with_bullets` [required]
    ///- `newline` [required]
    fn extract_def_dependent_tier<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> DefDependentTierChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, DefTierPrefixNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, TierSepNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, TextWithBulletsNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "def_tier_prefix", DefTierPrefixNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "tier_sep", TierSepNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "text_with_bullets", TextWithBulletsNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        DefDependentTierChildren {
            child_0,
            child_1,
            child_2,
            child_3,
        }
    }
    ///Extract children from a `eg_header` node.
    ///
    ///Production (children in order):
    ///- `eg_prefix` [required]
    ///- `(terminal)` [optional]
    ///- `newline` [required]
    fn extract_eg_header<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> EgHeaderChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, EgPrefixNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "eg_prefix", EgPrefixNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            idx += 1;
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        EgHeaderChildren { child_0, child_2 }
    }
    ///Extract children from a `end_header` node.
    ///
    ///Production (children in order):
    ///- `(terminal)` [required]
    ///- `newline` [required]
    fn extract_end_header<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> EndHeaderChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_1: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            idx += 1;
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        EndHeaderChildren { child_1 }
    }
    ///Extract children from a `eng_dependent_tier` node.
    ///
    ///Production (children in order):
    ///- `eng_tier_prefix` [required]
    ///- `tier_sep` [required]
    ///- `text_with_bullets` [required]
    ///- `newline` [required]
    fn extract_eng_dependent_tier<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> EngDependentTierChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, EngTierPrefixNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, TierSepNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, TextWithBulletsNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "eng_tier_prefix", EngTierPrefixNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "tier_sep", TierSepNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "text_with_bullets", TextWithBulletsNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        EngDependentTierChildren {
            child_0,
            child_1,
            child_2,
            child_3,
        }
    }
    ///Extract children from a `err_dependent_tier` node.
    ///
    ///Production (children in order):
    ///- `err_tier_prefix` [required]
    ///- `tier_sep` [required]
    ///- `text_with_bullets` [required]
    ///- `newline` [required]
    fn extract_err_dependent_tier<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> ErrDependentTierChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, ErrTierPrefixNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, TierSepNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, TextWithBulletsNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "err_tier_prefix", ErrTierPrefixNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "tier_sep", TierSepNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "text_with_bullets", TextWithBulletsNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        ErrDependentTierChildren {
            child_0,
            child_1,
            child_2,
            child_3,
        }
    }
    ///Extract children from a `event` node.
    ///
    ///Production (children in order):
    ///- `event_marker` [required]
    ///- `event_segment` (field: `description`) [required]
    fn extract_event<'tree>(&mut self, node: tree_sitter::Node<'tree>) -> EventChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, EventMarkerNode<'tree>> = NodeSlot::Absent;
        let mut description: NodeSlot<'tree, EventSegmentNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "event_marker", EventMarkerNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                description = classify_child(child, "event_segment", EventSegmentNode);
                idx += 1;
            }
        }
        EventChildren {
            child_0,
            description,
        }
    }
    ///Extract children from a `exp_dependent_tier` node.
    ///
    ///Production (children in order):
    ///- `exp_tier_prefix` [required]
    ///- `tier_sep` [required]
    ///- `text_with_bullets` [required]
    ///- `newline` [required]
    fn extract_exp_dependent_tier<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> ExpDependentTierChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, ExpTierPrefixNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, TierSepNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, TextWithBulletsNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "exp_tier_prefix", ExpTierPrefixNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "tier_sep", TierSepNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "text_with_bullets", TextWithBulletsNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        ExpDependentTierChildren {
            child_0,
            child_1,
            child_2,
            child_3,
        }
    }
    ///Extract children from a `explanation_annotation` node.
    ///
    ///Production (children in order):
    ///- `(terminal)` [required]
    ///- `space` [required]
    ///- `annotation_content` (field: `text`) [required]
    ///- `right_bracket` [required]
    fn extract_explanation_annotation<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> ExplanationAnnotationChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_1: NodeSlot<'tree, SpaceNode<'tree>> = NodeSlot::Absent;
        let mut text: NodeSlot<'tree, AnnotationContentNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, RightBracketNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            idx += 1;
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "space", SpaceNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                text = classify_child(child, "annotation_content", AnnotationContentNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "right_bracket", RightBracketNode);
                idx += 1;
            }
        }
        ExplanationAnnotationChildren {
            child_1,
            text,
            child_3,
        }
    }
    ///Extract children from a `fac_dependent_tier` node.
    ///
    ///Production (children in order):
    ///- `fac_tier_prefix` [required]
    ///- `tier_sep` [required]
    ///- `text_with_bullets` [required]
    ///- `newline` [required]
    fn extract_fac_dependent_tier<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> FacDependentTierChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, FacTierPrefixNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, TierSepNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, TextWithBulletsNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "fac_tier_prefix", FacTierPrefixNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "tier_sep", TierSepNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "text_with_bullets", TextWithBulletsNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        FacDependentTierChildren {
            child_0,
            child_1,
            child_2,
            child_3,
        }
    }
    ///Extract children from a `flo_dependent_tier` node.
    ///
    ///Production (children in order):
    ///- `flo_tier_prefix` [required]
    ///- `tier_sep` [required]
    ///- `text_with_bullets` [required]
    ///- `newline` [required]
    fn extract_flo_dependent_tier<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> FloDependentTierChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, FloTierPrefixNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, TierSepNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, TextWithBulletsNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "flo_tier_prefix", FloTierPrefixNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "tier_sep", TierSepNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "text_with_bullets", TextWithBulletsNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        FloDependentTierChildren {
            child_0,
            child_1,
            child_2,
            child_3,
        }
    }
    ///Extract children from a `font_header` node.
    ///
    ///Production (children in order):
    ///- `font_prefix` [required]
    ///- `header_sep` [required]
    ///- `free_text` [required]
    ///- `newline` [required]
    fn extract_font_header<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> FontHeaderChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, FontPrefixNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, HeaderSepNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, FreeTextNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "font_prefix", FontPrefixNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "header_sep", HeaderSepNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "free_text", FreeTextNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        FontHeaderChildren {
            child_0,
            child_1,
            child_2,
            child_3,
        }
    }
    ///Extract children from a `full_document` node.
    ///
    ///Production (children in order):
    ///- `utf8_header` [required]
    ///- `(terminal)` [required]
    ///- `begin_header` [required]
    ///- `(terminal)` [required]
    ///- `end_header` [required]
    fn extract_full_document<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> FullDocumentChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, Utf8HeaderNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, BeginHeaderNode<'tree>> = NodeSlot::Absent;
        let mut child_4: NodeSlot<'tree, EndHeaderNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "utf8_header", Utf8HeaderNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            idx += 1;
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "begin_header", BeginHeaderNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            idx += 1;
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_4 = classify_child(child, "end_header", EndHeaderNode);
                idx += 1;
            }
        }
        FullDocumentChildren {
            child_0,
            child_2,
            child_4,
        }
    }
    ///Extract children from a `g_header` node.
    ///
    ///Production (children in order):
    ///- `g_prefix` [required]
    ///- `header_sep` [required]
    ///- `free_text` [required]
    ///- `newline` [required]
    fn extract_g_header<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> GHeaderChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, GPrefixNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, HeaderSepNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, FreeTextNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "g_prefix", GPrefixNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "header_sep", HeaderSepNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "free_text", FreeTextNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        GHeaderChildren {
            child_0,
            child_1,
            child_2,
            child_3,
        }
    }
    ///Extract children from a `gls_dependent_tier` node.
    ///
    ///Production (children in order):
    ///- `gls_tier_prefix` [required]
    ///- `tier_sep` [required]
    ///- `text_with_bullets` [required]
    ///- `newline` [required]
    fn extract_gls_dependent_tier<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> GlsDependentTierChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, GlsTierPrefixNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, TierSepNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, TextWithBulletsNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "gls_tier_prefix", GlsTierPrefixNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "tier_sep", TierSepNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "text_with_bullets", TextWithBulletsNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        GlsDependentTierChildren {
            child_0,
            child_1,
            child_2,
            child_3,
        }
    }
    ///Extract children from a `gpx_dependent_tier` node.
    ///
    ///Production (children in order):
    ///- `gpx_tier_prefix` [required]
    ///- `tier_sep` [required]
    ///- `text_with_bullets` [required]
    ///- `newline` [required]
    fn extract_gpx_dependent_tier<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> GpxDependentTierChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, GpxTierPrefixNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, TierSepNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, TextWithBulletsNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "gpx_tier_prefix", GpxTierPrefixNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "tier_sep", TierSepNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "text_with_bullets", TextWithBulletsNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        GpxDependentTierChildren {
            child_0,
            child_1,
            child_2,
            child_3,
        }
    }
    ///Extract children from a `gra_contents` node.
    ///
    ///Production (children in order):
    ///- `gra_relation` [required]
    ///- `(terminal)` [required]
    fn extract_gra_contents<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> GraContentsChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, GraRelationNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "gra_relation", GraRelationNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            idx += 1;
        }
        GraContentsChildren { child_0 }
    }
    ///Extract children from a `gra_dependent_tier` node.
    ///
    ///Production (children in order):
    ///- `gra_tier_prefix` [required]
    ///- `tier_sep` [required]
    ///- `gra_contents` [required]
    ///- `newline` [required]
    fn extract_gra_dependent_tier<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> GraDependentTierChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, GraTierPrefixNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, TierSepNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, GraContentsNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "gra_tier_prefix", GraTierPrefixNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "tier_sep", TierSepNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "gra_contents", GraContentsNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        GraDependentTierChildren {
            child_0,
            child_1,
            child_2,
            child_3,
        }
    }
    ///Extract children from a `gra_relation` node.
    ///
    ///Production (children in order):
    ///- `gra_index` (field: `index`) [required]
    ///- `pipe` [required]
    ///- `gra_head` (field: `head`) [required]
    ///- `pipe` [required]
    ///- `gra_relation_name` (field: `relation`) [required]
    fn extract_gra_relation<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> GraRelationChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut index: NodeSlot<'tree, GraIndexNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, PipeNode<'tree>> = NodeSlot::Absent;
        let mut head: NodeSlot<'tree, GraHeadNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, PipeNode<'tree>> = NodeSlot::Absent;
        let mut relation: NodeSlot<'tree, GraRelationNameNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                index = classify_child(child, "gra_index", GraIndexNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "pipe", PipeNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                head = classify_child(child, "gra_head", GraHeadNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "pipe", PipeNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                relation = classify_child(child, "gra_relation_name", GraRelationNameNode);
                idx += 1;
            }
        }
        GraRelationChildren {
            index,
            child_1,
            head,
            child_3,
            relation,
        }
    }
    ///Extract children from a `group_with_annotations` node.
    ///
    ///Production (children in order):
    ///- `less_than` [required]
    ///- `contents` (field: `content`) [required]
    ///- `greater_than` [required]
    ///- `base_annotations` (field: `annotations`) [required]
    fn extract_group_with_annotations<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> GroupWithAnnotationsChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, LessThanNode<'tree>> = NodeSlot::Absent;
        let mut content: NodeSlot<'tree, ContentsNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, GreaterThanNode<'tree>> = NodeSlot::Absent;
        let mut annotations: NodeSlot<'tree, BaseAnnotationsNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "less_than", LessThanNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                content = classify_child(child, "contents", ContentsNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "greater_than", GreaterThanNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                annotations = classify_child(child, "base_annotations", BaseAnnotationsNode);
                idx += 1;
            }
        }
        GroupWithAnnotationsChildren {
            child_0,
            content,
            child_2,
            annotations,
        }
    }
    ///Extract children from a `header_sep` node.
    ///
    ///Production (children in order):
    ///- `colon` [required]
    ///- `tab` [required]
    fn extract_header_sep<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> HeaderSepChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, ColonNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, TabNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "colon", ColonNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "tab", TabNode);
                idx += 1;
            }
        }
        HeaderSepChildren { child_0, child_1 }
    }
    ///Extract children from a `id_contents` node.
    ///
    ///Production (children in order):
    ///- `_id_identity_fields` [required]
    ///- `_id_demographic_fields` [required]
    ///- `_id_role_fields` [required]
    fn extract_id_contents<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> IdContentsChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, IdIdentityFieldsNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, IdDemographicFieldsNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, IdRoleFieldsNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "_id_identity_fields", IdIdentityFieldsNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "_id_demographic_fields", IdDemographicFieldsNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "_id_role_fields", IdRoleFieldsNode);
                idx += 1;
            }
        }
        IdContentsChildren {
            child_0,
            child_1,
            child_2,
        }
    }
    ///Extract children from a `id_header` node.
    ///
    ///Production (children in order):
    ///- `id_prefix` [required]
    ///- `header_sep` [required]
    ///- `id_contents` [required]
    ///- `newline` [required]
    fn extract_id_header<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> IdHeaderChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, IdPrefixNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, HeaderSepNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, IdContentsNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "id_prefix", IdPrefixNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "header_sep", HeaderSepNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "id_contents", IdContentsNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        IdHeaderChildren {
            child_0,
            child_1,
            child_2,
            child_3,
        }
    }
    ///Extract children from a `int_dependent_tier` node.
    ///
    ///Production (children in order):
    ///- `int_tier_prefix` [required]
    ///- `tier_sep` [required]
    ///- `text_with_bullets` [required]
    ///- `newline` [required]
    fn extract_int_dependent_tier<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> IntDependentTierChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, IntTierPrefixNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, TierSepNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, TextWithBulletsNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "int_tier_prefix", IntTierPrefixNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "tier_sep", TierSepNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "text_with_bullets", TextWithBulletsNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        IntDependentTierChildren {
            child_0,
            child_1,
            child_2,
            child_3,
        }
    }
    ///Extract children from a `l1_of_header` node.
    ///
    ///Production (children in order):
    ///- `l1_of_prefix` [required]
    ///- `header_gap` [optional]
    ///- `speaker` [required]
    ///- `header_sep` [required]
    ///- `language_code` [required]
    ///- `newline` [required]
    fn extract_l1_of_header<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> L1OfHeaderChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, L1OfPrefixNode<'tree>> = NodeSlot::Absent;
        let mut child_1: Option<HeaderGapNode<'tree>> = None;
        let mut child_2: NodeSlot<'tree, SpeakerNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, HeaderSepNode<'tree>> = NodeSlot::Absent;
        let mut child_4: NodeSlot<'tree, LanguageCodeNode<'tree>> = NodeSlot::Absent;
        let mut child_5: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "l1_of_prefix", L1OfPrefixNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                if child.kind() == "header_gap" && !child.is_error() {
                    child_1 = Some(HeaderGapNode(child));
                    idx += 1;
                }
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "speaker", SpeakerNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "header_sep", HeaderSepNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_4 = classify_child(child, "language_code", LanguageCodeNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_5 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        L1OfHeaderChildren {
            child_0,
            child_1,
            child_2,
            child_3,
            child_4,
            child_5,
        }
    }
    ///Extract children from a `langcode` node.
    ///
    ///Production (children in order):
    ///- `(terminal)` [required]
    ///- `space` [required]
    ///- `language_code` (field: `code`) [required]
    ///- `right_bracket` [required]
    fn extract_langcode<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> LangcodeChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_1: NodeSlot<'tree, SpaceNode<'tree>> = NodeSlot::Absent;
        let mut code: NodeSlot<'tree, LanguageCodeNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, RightBracketNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            idx += 1;
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "space", SpaceNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                code = classify_child(child, "language_code", LanguageCodeNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "right_bracket", RightBracketNode);
                idx += 1;
            }
        }
        LangcodeChildren {
            child_1,
            code,
            child_3,
        }
    }
    ///Extract children from a `languages_contents` node.
    ///
    ///Production (children in order):
    ///- `language_code` [required]
    ///- `(terminal)` [required]
    fn extract_languages_contents<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> LanguagesContentsChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, LanguageCodeNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "language_code", LanguageCodeNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            idx += 1;
        }
        LanguagesContentsChildren { child_0 }
    }
    ///Extract children from a `languages_header` node.
    ///
    ///Production (children in order):
    ///- `languages_prefix` [required]
    ///- `header_sep` [required]
    ///- `languages_contents` [required]
    ///- `newline` [required]
    fn extract_languages_header<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> LanguagesHeaderChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, LanguagesPrefixNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, HeaderSepNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, LanguagesContentsNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "languages_prefix", LanguagesPrefixNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "header_sep", HeaderSepNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "languages_contents", LanguagesContentsNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        LanguagesHeaderChildren {
            child_0,
            child_1,
            child_2,
            child_3,
        }
    }
    ///Extract children from a `location_header` node.
    ///
    ///Production (children in order):
    ///- `location_prefix` [required]
    ///- `header_sep` [required]
    ///- `free_text` [required]
    ///- `newline` [required]
    fn extract_location_header<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> LocationHeaderChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, LocationPrefixNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, HeaderSepNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, FreeTextNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "location_prefix", LocationPrefixNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "header_sep", HeaderSepNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "free_text", FreeTextNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        LocationHeaderChildren {
            child_0,
            child_1,
            child_2,
            child_3,
        }
    }
    ///Extract children from a `long_feature_begin` node.
    ///
    ///Production (children in order):
    ///- `ampersand` [required]
    ///- `long_feature_begin_marker` [required]
    ///- `long_feature_label` [required]
    fn extract_long_feature_begin<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> LongFeatureBeginChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, AmpersandNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, LongFeatureBeginMarkerNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, LongFeatureLabelNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "ampersand", AmpersandNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(
                    child,
                    "long_feature_begin_marker",
                    LongFeatureBeginMarkerNode,
                );
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "long_feature_label", LongFeatureLabelNode);
                idx += 1;
            }
        }
        LongFeatureBeginChildren {
            child_0,
            child_1,
            child_2,
        }
    }
    ///Extract children from a `long_feature_end` node.
    ///
    ///Production (children in order):
    ///- `ampersand` [required]
    ///- `long_feature_end_marker` [required]
    ///- `long_feature_label` [required]
    fn extract_long_feature_end<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> LongFeatureEndChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, AmpersandNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, LongFeatureEndMarkerNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, LongFeatureLabelNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "ampersand", AmpersandNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 =
                    classify_child(child, "long_feature_end_marker", LongFeatureEndMarkerNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "long_feature_label", LongFeatureLabelNode);
                idx += 1;
            }
        }
        LongFeatureEndChildren {
            child_0,
            child_1,
            child_2,
        }
    }
    ///Extract children from a `main_pho_group` node.
    ///
    ///Production (children in order):
    ///- `pho_begin_group` [required]
    ///- `contents` [required]
    ///- `pho_end_group` [required]
    fn extract_main_pho_group<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> MainPhoGroupChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, PhoBeginGroupNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, ContentsNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, PhoEndGroupNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "pho_begin_group", PhoBeginGroupNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "contents", ContentsNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "pho_end_group", PhoEndGroupNode);
                idx += 1;
            }
        }
        MainPhoGroupChildren {
            child_0,
            child_1,
            child_2,
        }
    }
    ///Extract children from a `main_sin_group` node.
    ///
    ///Production (children in order):
    ///- `sin_begin_group` [required]
    ///- `contents` [required]
    ///- `sin_end_group` [required]
    fn extract_main_sin_group<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> MainSinGroupChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, SinBeginGroupNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, ContentsNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, SinEndGroupNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "sin_begin_group", SinBeginGroupNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "contents", ContentsNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "sin_end_group", SinEndGroupNode);
                idx += 1;
            }
        }
        MainSinGroupChildren {
            child_0,
            child_1,
            child_2,
        }
    }
    ///Extract children from a `main_tier` node.
    ///
    ///Production (children in order):
    ///- `star` [required]
    ///- `speaker` (field: `speaker`) [required]
    ///- `colon` [required]
    ///- `tab` [required]
    ///- `tier_body` [required]
    fn extract_main_tier<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> MainTierChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, StarNode<'tree>> = NodeSlot::Absent;
        let mut speaker: NodeSlot<'tree, SpeakerNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, ColonNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, TabNode<'tree>> = NodeSlot::Absent;
        let mut child_4: NodeSlot<'tree, TierBodyNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "star", StarNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                speaker = classify_child(child, "speaker", SpeakerNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "colon", ColonNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "tab", TabNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_4 = classify_child(child, "tier_body", TierBodyNode);
                idx += 1;
            }
        }
        MainTierChildren {
            child_0,
            speaker,
            child_2,
            child_3,
            child_4,
        }
    }
    ///Extract children from a `media_contents` node.
    ///
    ///Production (children in order):
    ///- `media_filename` [required]
    ///- `comma` [required]
    ///- `whitespaces` [required]
    ///- `media_type` [required]
    ///- `(terminal)` [optional]
    fn extract_media_contents<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> MediaContentsChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, MediaFilenameNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, CommaNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, WhitespacesNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, MediaTypeNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "media_filename", MediaFilenameNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "comma", CommaNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "whitespaces", WhitespacesNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "media_type", MediaTypeNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            idx += 1;
        }
        MediaContentsChildren {
            child_0,
            child_1,
            child_2,
            child_3,
        }
    }
    ///Extract children from a `media_header` node.
    ///
    ///Production (children in order):
    ///- `media_prefix` [required]
    ///- `header_sep` [required]
    ///- `media_contents` [required]
    ///- `newline` [required]
    fn extract_media_header<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> MediaHeaderChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, MediaPrefixNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, HeaderSepNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, MediaContentsNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "media_prefix", MediaPrefixNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "header_sep", HeaderSepNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "media_contents", MediaContentsNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        MediaHeaderChildren {
            child_0,
            child_1,
            child_2,
            child_3,
        }
    }
    ///Extract children from a `mod_dependent_tier` node.
    ///
    ///Production (children in order):
    ///- `mod_tier_prefix` [required]
    ///- `tier_sep` [required]
    ///- `pho_groups` [required]
    ///- `newline` [required]
    fn extract_mod_dependent_tier<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> ModDependentTierChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, ModTierPrefixNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, TierSepNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, PhoGroupsNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "mod_tier_prefix", ModTierPrefixNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "tier_sep", TierSepNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "pho_groups", PhoGroupsNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        ModDependentTierChildren {
            child_0,
            child_1,
            child_2,
            child_3,
        }
    }
    ///Extract children from a `modsyl_dependent_tier` node.
    ///
    ///Production (children in order):
    ///- `modsyl_tier_prefix` [required]
    ///- `tier_sep` [required]
    ///- `text_with_bullets` [required]
    ///- `newline` [required]
    fn extract_modsyl_dependent_tier<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> ModsylDependentTierChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, ModsylTierPrefixNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, TierSepNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, TextWithBulletsNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "modsyl_tier_prefix", ModsylTierPrefixNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "tier_sep", TierSepNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "text_with_bullets", TextWithBulletsNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        ModsylDependentTierChildren {
            child_0,
            child_1,
            child_2,
            child_3,
        }
    }
    ///Extract children from a `mor_content` node.
    ///
    ///Production (children in order):
    ///- `mor_word` (field: `main`) [required]
    ///- `(terminal)` (field: `post_clitics`) [required]
    fn extract_mor_content<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> MorContentChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut main: NodeSlot<'tree, MorWordNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                main = classify_child(child, "mor_word", MorWordNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            idx += 1;
        }
        MorContentChildren { main }
    }
    ///Extract children from a `mor_contents` node.
    ///
    ///Production (children in order):
    ///- `(terminal)` [required]
    ///- `whitespaces` [optional]
    fn extract_mor_contents<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> MorContentsChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_1: Option<WhitespacesNode<'tree>> = None;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            idx += 1;
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                if child.kind() == "whitespaces" && !child.is_error() {
                    child_1 = Some(WhitespacesNode(child));
                    idx += 1;
                }
            }
        }
        MorContentsChildren { child_1 }
    }
    ///Extract children from a `mor_dependent_tier` node.
    ///
    ///Production (children in order):
    ///- `mor_tier_prefix` [required]
    ///- `tier_sep` [required]
    ///- `mor_contents` [required]
    ///- `newline` [required]
    fn extract_mor_dependent_tier<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> MorDependentTierChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, MorTierPrefixNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, TierSepNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, MorContentsNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "mor_tier_prefix", MorTierPrefixNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "tier_sep", TierSepNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "mor_contents", MorContentsNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        MorDependentTierChildren {
            child_0,
            child_1,
            child_2,
            child_3,
        }
    }
    ///Extract children from a `mor_feature` node.
    ///
    ///Production (children in order):
    ///- `hyphen` [required]
    ///- `mor_feature_value` [required]
    fn extract_mor_feature<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> MorFeatureChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, HyphenNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, MorFeatureValueNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "hyphen", HyphenNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "mor_feature_value", MorFeatureValueNode);
                idx += 1;
            }
        }
        MorFeatureChildren { child_0, child_1 }
    }
    ///Extract children from a `mor_post_clitic` node.
    ///
    ///Production (children in order):
    ///- `tilde` [required]
    ///- `mor_word` [required]
    fn extract_mor_post_clitic<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> MorPostCliticChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, TildeNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, MorWordNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "tilde", TildeNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "mor_word", MorWordNode);
                idx += 1;
            }
        }
        MorPostCliticChildren { child_0, child_1 }
    }
    ///Extract children from a `mor_word` node.
    ///
    ///Production (children in order):
    ///- `mor_pos` [required]
    ///- `pipe` [required]
    ///- `mor_lemma` [required]
    ///- `(terminal)` [required]
    fn extract_mor_word<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> MorWordChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, MorPosNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, PipeNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, MorLemmaNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "mor_pos", MorPosNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "pipe", PipeNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "mor_lemma", MorLemmaNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            idx += 1;
        }
        MorWordChildren {
            child_0,
            child_1,
            child_2,
        }
    }
    ///Extract children from a `new_episode_header` node.
    ///
    ///Production (children in order):
    ///- `new_episode_prefix` [required]
    ///- `newline` [required]
    fn extract_new_episode_header<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> NewEpisodeHeaderChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, NewEpisodePrefixNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "new_episode_prefix", NewEpisodePrefixNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        NewEpisodeHeaderChildren { child_0, child_1 }
    }
    ///Extract children from a `nonvocal_begin` node.
    ///
    ///Production (children in order):
    ///- `ampersand` [required]
    ///- `nonvocal_begin_marker` [required]
    ///- `long_feature_label` [required]
    fn extract_nonvocal_begin<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> NonvocalBeginChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, AmpersandNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, NonvocalBeginMarkerNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, LongFeatureLabelNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "ampersand", AmpersandNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "nonvocal_begin_marker", NonvocalBeginMarkerNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "long_feature_label", LongFeatureLabelNode);
                idx += 1;
            }
        }
        NonvocalBeginChildren {
            child_0,
            child_1,
            child_2,
        }
    }
    ///Extract children from a `nonvocal_end` node.
    ///
    ///Production (children in order):
    ///- `ampersand` [required]
    ///- `nonvocal_end_marker` [required]
    ///- `long_feature_label` [required]
    fn extract_nonvocal_end<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> NonvocalEndChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, AmpersandNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, NonvocalEndMarkerNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, LongFeatureLabelNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "ampersand", AmpersandNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "nonvocal_end_marker", NonvocalEndMarkerNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "long_feature_label", LongFeatureLabelNode);
                idx += 1;
            }
        }
        NonvocalEndChildren {
            child_0,
            child_1,
            child_2,
        }
    }
    ///Extract children from a `nonvocal_simple` node.
    ///
    ///Production (children in order):
    ///- `ampersand` [required]
    ///- `nonvocal_begin_marker` [required]
    ///- `long_feature_label` [required]
    ///- `right_brace` [required]
    fn extract_nonvocal_simple<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> NonvocalSimpleChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, AmpersandNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, NonvocalBeginMarkerNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, LongFeatureLabelNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, RightBraceNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "ampersand", AmpersandNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "nonvocal_begin_marker", NonvocalBeginMarkerNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "long_feature_label", LongFeatureLabelNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "right_brace", RightBraceNode);
                idx += 1;
            }
        }
        NonvocalSimpleChildren {
            child_0,
            child_1,
            child_2,
            child_3,
        }
    }
    ///Extract children from a `nonword_with_optional_annotations` node.
    ///
    ///Production (children in order):
    ///- `nonword` (field: `nonword`) [required]
    ///- `base_annotations` (field: `annotations`) [optional]
    fn extract_nonword_with_optional_annotations<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> NonwordWithOptionalAnnotationsChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut nonword: NodeSlot<'tree, NonwordNode<'tree>> = NodeSlot::Absent;
        let mut annotations: Option<BaseAnnotationsNode<'tree>> = None;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                nonword = classify_child(child, "nonword", NonwordNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                if child.kind() == "base_annotations" && !child.is_error() {
                    annotations = Some(BaseAnnotationsNode(child));
                    idx += 1;
                }
            }
        }
        NonwordWithOptionalAnnotationsChildren {
            nonword,
            annotations,
        }
    }
    ///Extract children from a `number_header` node.
    ///
    ///Production (children in order):
    ///- `number_prefix` [required]
    ///- `header_sep` [required]
    ///- `number_option` [required]
    ///- `newline` [required]
    fn extract_number_header<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> NumberHeaderChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, NumberPrefixNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, HeaderSepNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, NumberOptionNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "number_prefix", NumberPrefixNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "header_sep", HeaderSepNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "number_option", NumberOptionNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        NumberHeaderChildren {
            child_0,
            child_1,
            child_2,
            child_3,
        }
    }
    ///Extract children from a `options_contents` node.
    ///
    ///Production (children in order):
    ///- `option_name` [required]
    ///- `(terminal)` [required]
    fn extract_options_contents<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> OptionsContentsChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, OptionNameNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "option_name", OptionNameNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            idx += 1;
        }
        OptionsContentsChildren { child_0 }
    }
    ///Extract children from a `options_header` node.
    ///
    ///Production (children in order):
    ///- `options_prefix` [required]
    ///- `header_sep` [required]
    ///- `options_contents` [required]
    ///- `newline` [required]
    fn extract_options_header<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> OptionsHeaderChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, OptionsPrefixNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, HeaderSepNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, OptionsContentsNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "options_prefix", OptionsPrefixNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "header_sep", HeaderSepNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "options_contents", OptionsContentsNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        OptionsHeaderChildren {
            child_0,
            child_1,
            child_2,
            child_3,
        }
    }
    ///Extract children from a `ort_dependent_tier` node.
    ///
    ///Production (children in order):
    ///- `ort_tier_prefix` [required]
    ///- `tier_sep` [required]
    ///- `text_with_bullets` [required]
    ///- `newline` [required]
    fn extract_ort_dependent_tier<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> OrtDependentTierChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, OrtTierPrefixNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, TierSepNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, TextWithBulletsNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "ort_tier_prefix", OrtTierPrefixNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "tier_sep", TierSepNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "text_with_bullets", TextWithBulletsNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        OrtDependentTierChildren {
            child_0,
            child_1,
            child_2,
            child_3,
        }
    }
    ///Extract children from a `other_spoken_event` node.
    ///
    ///Production (children in order):
    ///- `ampersand` [required]
    ///- `star` [required]
    ///- `speaker` [required]
    ///- `colon` [required]
    ///- `standalone_word` [required]
    fn extract_other_spoken_event<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> OtherSpokenEventChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, AmpersandNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, StarNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, SpeakerNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, ColonNode<'tree>> = NodeSlot::Absent;
        let mut child_4: NodeSlot<'tree, StandaloneWordNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "ampersand", AmpersandNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "star", StarNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "speaker", SpeakerNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "colon", ColonNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_4 = classify_child(child, "standalone_word", StandaloneWordNode);
                idx += 1;
            }
        }
        OtherSpokenEventChildren {
            child_0,
            child_1,
            child_2,
            child_3,
            child_4,
        }
    }
    ///Extract children from a `page_header` node.
    ///
    ///Production (children in order):
    ///- `page_prefix` [required]
    ///- `header_sep` [required]
    ///- `page_number` [required]
    ///- `newline` [required]
    fn extract_page_header<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> PageHeaderChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, PagePrefixNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, HeaderSepNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, PageNumberNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "page_prefix", PagePrefixNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "header_sep", HeaderSepNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "page_number", PageNumberNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        PageHeaderChildren {
            child_0,
            child_1,
            child_2,
            child_3,
        }
    }
    ///Extract children from a `par_dependent_tier` node.
    ///
    ///Production (children in order):
    ///- `par_tier_prefix` [required]
    ///- `tier_sep` [required]
    ///- `text_with_bullets` [required]
    ///- `newline` [required]
    fn extract_par_dependent_tier<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> ParDependentTierChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, ParTierPrefixNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, TierSepNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, TextWithBulletsNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "par_tier_prefix", ParTierPrefixNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "tier_sep", TierSepNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "text_with_bullets", TextWithBulletsNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        ParDependentTierChildren {
            child_0,
            child_1,
            child_2,
            child_3,
        }
    }
    ///Extract children from a `para_annotation` node.
    ///
    ///Production (children in order):
    ///- `(terminal)` [required]
    ///- `space` [required]
    ///- `annotation_content` (field: `text`) [required]
    ///- `right_bracket` [required]
    fn extract_para_annotation<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> ParaAnnotationChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_1: NodeSlot<'tree, SpaceNode<'tree>> = NodeSlot::Absent;
        let mut text: NodeSlot<'tree, AnnotationContentNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, RightBracketNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            idx += 1;
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "space", SpaceNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                text = classify_child(child, "annotation_content", AnnotationContentNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "right_bracket", RightBracketNode);
                idx += 1;
            }
        }
        ParaAnnotationChildren {
            child_1,
            text,
            child_3,
        }
    }
    ///Extract children from a `participant` node.
    ///
    ///Production (children in order):
    ///- `speaker` (field: `code`) [required]
    ///- `(terminal)` [required]
    ///- `whitespaces` [optional]
    fn extract_participant<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> ParticipantChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut code: NodeSlot<'tree, SpeakerNode<'tree>> = NodeSlot::Absent;
        let mut child_2: Option<WhitespacesNode<'tree>> = None;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                code = classify_child(child, "speaker", SpeakerNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            idx += 1;
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                if child.kind() == "whitespaces" && !child.is_error() {
                    child_2 = Some(WhitespacesNode(child));
                    idx += 1;
                }
            }
        }
        ParticipantChildren { code, child_2 }
    }
    ///Extract children from a `participants_contents` node.
    ///
    ///Production (children in order):
    ///- `participant` [required]
    ///- `(terminal)` [required]
    fn extract_participants_contents<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> ParticipantsContentsChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, ParticipantNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "participant", ParticipantNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            idx += 1;
        }
        ParticipantsContentsChildren { child_0 }
    }
    ///Extract children from a `participants_header` node.
    ///
    ///Production (children in order):
    ///- `participants_prefix` [required]
    ///- `header_sep` [required]
    ///- `participants_contents` [required]
    ///- `newline` [required]
    fn extract_participants_header<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> ParticipantsHeaderChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, ParticipantsPrefixNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, HeaderSepNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, ParticipantsContentsNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "participants_prefix", ParticipantsPrefixNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "header_sep", HeaderSepNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "participants_contents", ParticipantsContentsNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        ParticipantsHeaderChildren {
            child_0,
            child_1,
            child_2,
            child_3,
        }
    }
    ///Extract children from a `percent_annotation` node.
    ///
    ///Production (children in order):
    ///- `(terminal)` [required]
    ///- `space` [required]
    ///- `annotation_content` (field: `text`) [required]
    ///- `right_bracket` [required]
    fn extract_percent_annotation<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> PercentAnnotationChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_1: NodeSlot<'tree, SpaceNode<'tree>> = NodeSlot::Absent;
        let mut text: NodeSlot<'tree, AnnotationContentNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, RightBracketNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            idx += 1;
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "space", SpaceNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                text = classify_child(child, "annotation_content", AnnotationContentNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "right_bracket", RightBracketNode);
                idx += 1;
            }
        }
        PercentAnnotationChildren {
            child_1,
            text,
            child_3,
        }
    }
    ///Extract children from a `pho_dependent_tier` node.
    ///
    ///Production (children in order):
    ///- `pho_tier_prefix` [required]
    ///- `tier_sep` [required]
    ///- `pho_groups` [required]
    ///- `newline` [required]
    fn extract_pho_dependent_tier<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> PhoDependentTierChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, PhoTierPrefixNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, TierSepNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, PhoGroupsNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "pho_tier_prefix", PhoTierPrefixNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "tier_sep", TierSepNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "pho_groups", PhoGroupsNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        PhoDependentTierChildren {
            child_0,
            child_1,
            child_2,
            child_3,
        }
    }
    ///Extract children from a `pho_grouped_content` node.
    ///
    ///Production (children in order):
    ///- `pho_words` [required]
    ///- `(terminal)` [required]
    fn extract_pho_grouped_content<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> PhoGroupedContentChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, PhoWordsNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "pho_words", PhoWordsNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            idx += 1;
        }
        PhoGroupedContentChildren { child_0 }
    }
    ///Extract children from a `pho_groups` node.
    ///
    ///Production (children in order):
    ///- `pho_group` [required]
    ///- `(terminal)` [required]
    fn extract_pho_groups<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> PhoGroupsChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, PhoGroupNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "pho_group", PhoGroupNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            idx += 1;
        }
        PhoGroupsChildren { child_0 }
    }
    ///Extract children from a `pho_words` node.
    ///
    ///Production (children in order):
    ///- `pho_word` [required]
    ///- `(terminal)` [required]
    fn extract_pho_words<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> PhoWordsChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, PhoWordNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "pho_word", PhoWordNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            idx += 1;
        }
        PhoWordsChildren { child_0 }
    }
    ///Extract children from a `phoaln_dependent_tier` node.
    ///
    ///Production (children in order):
    ///- `phoaln_tier_prefix` [required]
    ///- `tier_sep` [required]
    ///- `text_with_bullets` [required]
    ///- `newline` [required]
    fn extract_phoaln_dependent_tier<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> PhoalnDependentTierChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, PhoalnTierPrefixNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, TierSepNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, TextWithBulletsNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "phoaln_tier_prefix", PhoalnTierPrefixNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "tier_sep", TierSepNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "text_with_bullets", TextWithBulletsNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        PhoalnDependentTierChildren {
            child_0,
            child_1,
            child_2,
            child_3,
        }
    }
    ///Extract children from a `phosyl_dependent_tier` node.
    ///
    ///Production (children in order):
    ///- `phosyl_tier_prefix` [required]
    ///- `tier_sep` [required]
    ///- `text_with_bullets` [required]
    ///- `newline` [required]
    fn extract_phosyl_dependent_tier<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> PhosylDependentTierChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, PhosylTierPrefixNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, TierSepNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, TextWithBulletsNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "phosyl_tier_prefix", PhosylTierPrefixNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "tier_sep", TierSepNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "text_with_bullets", TextWithBulletsNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        PhosylDependentTierChildren {
            child_0,
            child_1,
            child_2,
            child_3,
        }
    }
    ///Extract children from a `pid_header` node.
    ///
    ///Production (children in order):
    ///- `pid_prefix` [required]
    ///- `header_sep` [required]
    ///- `free_text` [required]
    ///- `newline` [required]
    fn extract_pid_header<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> PidHeaderChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, PidPrefixNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, HeaderSepNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, FreeTextNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "pid_prefix", PidPrefixNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "header_sep", HeaderSepNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "free_text", FreeTextNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        PidHeaderChildren {
            child_0,
            child_1,
            child_2,
            child_3,
        }
    }
    ///Extract children from a `pos_tag` node.
    ///
    ///Production (children in order):
    ///- `(terminal)` [required]
    ///- `(terminal)` [required]
    fn extract_pos_tag<'tree>(&mut self, node: tree_sitter::Node<'tree>) -> PosTagChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            idx += 1;
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            idx += 1;
        }
        PosTagChildren {
            _phantom: std::marker::PhantomData,
        }
    }
    ///Extract children from a `postcode` node.
    ///
    ///Production (children in order):
    ///- `(terminal)` [required]
    ///- `space` [required]
    ///- `annotation_content` (field: `code`) [required]
    ///- `right_bracket` [required]
    fn extract_postcode<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> PostcodeChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_1: NodeSlot<'tree, SpaceNode<'tree>> = NodeSlot::Absent;
        let mut code: NodeSlot<'tree, AnnotationContentNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, RightBracketNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            idx += 1;
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "space", SpaceNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                code = classify_child(child, "annotation_content", AnnotationContentNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "right_bracket", RightBracketNode);
                idx += 1;
            }
        }
        PostcodeChildren {
            child_1,
            code,
            child_3,
        }
    }
    ///Extract children from a `quotation` node.
    ///
    ///Production (children in order):
    ///- `left_double_quote` [required]
    ///- `contents` [required]
    ///- `right_double_quote` [required]
    fn extract_quotation<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> QuotationChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, LeftDoubleQuoteNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, ContentsNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, RightDoubleQuoteNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "left_double_quote", LeftDoubleQuoteNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "contents", ContentsNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "right_double_quote", RightDoubleQuoteNode);
                idx += 1;
            }
        }
        QuotationChildren {
            child_0,
            child_1,
            child_2,
        }
    }
    ///Extract children from a `recording_quality_header` node.
    ///
    ///Production (children in order):
    ///- `recording_quality_prefix` [required]
    ///- `header_sep` [required]
    ///- `recording_quality_option` [required]
    ///- `newline` [required]
    fn extract_recording_quality_header<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> RecordingQualityHeaderChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, RecordingQualityPrefixNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, HeaderSepNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, RecordingQualityOptionNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(
                    child,
                    "recording_quality_prefix",
                    RecordingQualityPrefixNode,
                );
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "header_sep", HeaderSepNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(
                    child,
                    "recording_quality_option",
                    RecordingQualityOptionNode,
                );
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        RecordingQualityHeaderChildren {
            child_0,
            child_1,
            child_2,
            child_3,
        }
    }
    ///Extract children from a `replacement` node.
    ///
    ///Production (children in order):
    ///- `left_bracket` [required]
    ///- `colon` [required]
    ///- `(terminal)` [required]
    ///- `right_bracket` [required]
    fn extract_replacement<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> ReplacementChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, LeftBracketNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, ColonNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, RightBracketNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "left_bracket", LeftBracketNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "colon", ColonNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            idx += 1;
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "right_bracket", RightBracketNode);
                idx += 1;
            }
        }
        ReplacementChildren {
            child_0,
            child_1,
            child_3,
        }
    }
    ///Extract children from a `room_layout_header` node.
    ///
    ///Production (children in order):
    ///- `room_layout_prefix` [required]
    ///- `header_sep` [required]
    ///- `free_text` [required]
    ///- `newline` [required]
    fn extract_room_layout_header<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> RoomLayoutHeaderChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, RoomLayoutPrefixNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, HeaderSepNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, FreeTextNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "room_layout_prefix", RoomLayoutPrefixNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "header_sep", HeaderSepNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "free_text", FreeTextNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        RoomLayoutHeaderChildren {
            child_0,
            child_1,
            child_2,
            child_3,
        }
    }
    ///Extract children from a `shortening` node.
    ///
    ///Production (children in order):
    ///- `(terminal)` [required]
    ///- `word_segment` [required]
    ///- `(terminal)` [required]
    fn extract_shortening<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> ShorteningChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_1: NodeSlot<'tree, WordSegmentNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            idx += 1;
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "word_segment", WordSegmentNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            idx += 1;
        }
        ShorteningChildren { child_1 }
    }
    ///Extract children from a `sin_dependent_tier` node.
    ///
    ///Production (children in order):
    ///- `sin_tier_prefix` [required]
    ///- `tier_sep` [required]
    ///- `sin_groups` [required]
    ///- `newline` [required]
    fn extract_sin_dependent_tier<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> SinDependentTierChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, SinTierPrefixNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, TierSepNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, SinGroupsNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "sin_tier_prefix", SinTierPrefixNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "tier_sep", TierSepNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "sin_groups", SinGroupsNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        SinDependentTierChildren {
            child_0,
            child_1,
            child_2,
            child_3,
        }
    }
    ///Extract children from a `sin_grouped_content` node.
    ///
    ///Production (children in order):
    ///- `sin_word` [required]
    ///- `(terminal)` [required]
    fn extract_sin_grouped_content<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> SinGroupedContentChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, SinWordNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "sin_word", SinWordNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            idx += 1;
        }
        SinGroupedContentChildren { child_0 }
    }
    ///Extract children from a `sin_groups` node.
    ///
    ///Production (children in order):
    ///- `sin_group` [required]
    ///- `(terminal)` [required]
    fn extract_sin_groups<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> SinGroupsChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, SinGroupNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "sin_group", SinGroupNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            idx += 1;
        }
        SinGroupsChildren { child_0 }
    }
    ///Extract children from a `sit_dependent_tier` node.
    ///
    ///Production (children in order):
    ///- `sit_tier_prefix` [required]
    ///- `tier_sep` [required]
    ///- `text_with_bullets` [required]
    ///- `newline` [required]
    fn extract_sit_dependent_tier<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> SitDependentTierChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, SitTierPrefixNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, TierSepNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, TextWithBulletsNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "sit_tier_prefix", SitTierPrefixNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "tier_sep", TierSepNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "text_with_bullets", TextWithBulletsNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        SitDependentTierChildren {
            child_0,
            child_1,
            child_2,
            child_3,
        }
    }
    ///Extract children from a `situation_header` node.
    ///
    ///Production (children in order):
    ///- `situation_prefix` [required]
    ///- `header_sep` [required]
    ///- `free_text` [required]
    ///- `newline` [required]
    fn extract_situation_header<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> SituationHeaderChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, SituationPrefixNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, HeaderSepNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, FreeTextNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "situation_prefix", SituationPrefixNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "header_sep", HeaderSepNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "free_text", FreeTextNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        SituationHeaderChildren {
            child_0,
            child_1,
            child_2,
            child_3,
        }
    }
    ///Extract children from a `spa_dependent_tier` node.
    ///
    ///Production (children in order):
    ///- `spa_tier_prefix` [required]
    ///- `tier_sep` [required]
    ///- `text_with_bullets` [required]
    ///- `newline` [required]
    fn extract_spa_dependent_tier<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> SpaDependentTierChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, SpaTierPrefixNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, TierSepNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, TextWithBulletsNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "spa_tier_prefix", SpaTierPrefixNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "tier_sep", TierSepNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "text_with_bullets", TextWithBulletsNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        SpaDependentTierChildren {
            child_0,
            child_1,
            child_2,
            child_3,
        }
    }
    ///Extract children from a `standalone_word` node.
    ///
    ///Production (children in order):
    ///- `(terminal)` [optional]
    ///- `word_body` [required]
    ///- `form_marker` [optional]
    ///- `word_lang_suffix` [optional]
    ///- `pos_tag` [optional]
    fn extract_standalone_word<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> StandaloneWordChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_1: NodeSlot<'tree, WordBodyNode<'tree>> = NodeSlot::Absent;
        let mut child_2: Option<FormMarkerNode<'tree>> = None;
        let mut child_3: Option<WordLangSuffixNode<'tree>> = None;
        let mut child_4: Option<PosTagNode<'tree>> = None;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            idx += 1;
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "word_body", WordBodyNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                if child.kind() == "form_marker" && !child.is_error() {
                    child_2 = Some(FormMarkerNode(child));
                    idx += 1;
                }
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                if child.kind() == "word_lang_suffix" && !child.is_error() {
                    child_3 = Some(WordLangSuffixNode(child));
                    idx += 1;
                }
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                if child.kind() == "pos_tag" && !child.is_error() {
                    child_4 = Some(PosTagNode(child));
                    idx += 1;
                }
            }
        }
        StandaloneWordChildren {
            child_1,
            child_2,
            child_3,
            child_4,
        }
    }
    ///Extract children from a `t_header` node.
    ///
    ///Production (children in order):
    ///- `t_prefix` [required]
    ///- `header_sep` [required]
    ///- `free_text` [required]
    ///- `newline` [required]
    fn extract_t_header<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> THeaderChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, TPrefixNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, HeaderSepNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, FreeTextNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "t_prefix", TPrefixNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "header_sep", HeaderSepNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "free_text", FreeTextNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        THeaderChildren {
            child_0,
            child_1,
            child_2,
            child_3,
        }
    }
    ///Extract children from a `tape_location_header` node.
    ///
    ///Production (children in order):
    ///- `tape_location_prefix` [required]
    ///- `header_sep` [required]
    ///- `free_text` [required]
    ///- `newline` [required]
    fn extract_tape_location_header<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> TapeLocationHeaderChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, TapeLocationPrefixNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, HeaderSepNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, FreeTextNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "tape_location_prefix", TapeLocationPrefixNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "header_sep", HeaderSepNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "free_text", FreeTextNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        TapeLocationHeaderChildren {
            child_0,
            child_1,
            child_2,
            child_3,
        }
    }
    ///Extract children from a `thumbnail_header` node.
    ///
    ///Production (children in order):
    ///- `thumbnail_prefix` [required]
    ///- `header_sep` [required]
    ///- `free_text` [required]
    ///- `newline` [required]
    fn extract_thumbnail_header<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> ThumbnailHeaderChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, ThumbnailPrefixNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, HeaderSepNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, FreeTextNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "thumbnail_prefix", ThumbnailPrefixNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "header_sep", HeaderSepNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "free_text", FreeTextNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        ThumbnailHeaderChildren {
            child_0,
            child_1,
            child_2,
            child_3,
        }
    }
    ///Extract children from a `tier_body` node.
    ///
    ///Production (children in order):
    ///- `linkers` (field: `linkers`) [optional]
    ///- `(terminal)` (field: `language_code`) [optional]
    ///- `contents` (field: `content`) [required]
    ///- `utterance_end` (field: `ending`) [required]
    fn extract_tier_body<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> TierBodyChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut linkers: Option<LinkersNode<'tree>> = None;
        let mut content: NodeSlot<'tree, ContentsNode<'tree>> = NodeSlot::Absent;
        let mut ending: NodeSlot<'tree, UtteranceEndNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                if child.kind() == "linkers" && !child.is_error() {
                    linkers = Some(LinkersNode(child));
                    idx += 1;
                }
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            idx += 1;
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                content = classify_child(child, "contents", ContentsNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                ending = classify_child(child, "utterance_end", UtteranceEndNode);
                idx += 1;
            }
        }
        TierBodyChildren {
            linkers,
            content,
            ending,
        }
    }
    ///Extract children from a `tier_sep` node.
    ///
    ///Production (children in order):
    ///- `colon` [required]
    ///- `tab` [required]
    fn extract_tier_sep<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> TierSepChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, ColonNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, TabNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "colon", ColonNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "tab", TabNode);
                idx += 1;
            }
        }
        TierSepChildren { child_0, child_1 }
    }
    ///Extract children from a `tim_dependent_tier` node.
    ///
    ///Production (children in order):
    ///- `tim_tier_prefix` [required]
    ///- `tier_sep` [required]
    ///- `text_with_bullets` [required]
    ///- `newline` [required]
    fn extract_tim_dependent_tier<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> TimDependentTierChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, TimTierPrefixNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, TierSepNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, TextWithBulletsNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "tim_tier_prefix", TimTierPrefixNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "tier_sep", TierSepNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "text_with_bullets", TextWithBulletsNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        TimDependentTierChildren {
            child_0,
            child_1,
            child_2,
            child_3,
        }
    }
    ///Extract children from a `time_duration_header` node.
    ///
    ///Production (children in order):
    ///- `time_duration_prefix` [required]
    ///- `header_sep` [required]
    ///- `time_duration_contents` [required]
    ///- `newline` [required]
    fn extract_time_duration_header<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> TimeDurationHeaderChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, TimeDurationPrefixNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, HeaderSepNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, TimeDurationContentsNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "time_duration_prefix", TimeDurationPrefixNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "header_sep", HeaderSepNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "time_duration_contents", TimeDurationContentsNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        TimeDurationHeaderChildren {
            child_0,
            child_1,
            child_2,
            child_3,
        }
    }
    ///Extract children from a `time_start_header` node.
    ///
    ///Production (children in order):
    ///- `time_start_prefix` [required]
    ///- `header_sep` [required]
    ///- `time_duration_contents` [required]
    ///- `newline` [required]
    fn extract_time_start_header<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> TimeStartHeaderChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, TimeStartPrefixNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, HeaderSepNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, TimeDurationContentsNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "time_start_prefix", TimeStartPrefixNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "header_sep", HeaderSepNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "time_duration_contents", TimeDurationContentsNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        TimeStartHeaderChildren {
            child_0,
            child_1,
            child_2,
            child_3,
        }
    }
    ///Extract children from a `transcriber_header` node.
    ///
    ///Production (children in order):
    ///- `transcriber_prefix` [required]
    ///- `header_sep` [required]
    ///- `free_text` [required]
    ///- `newline` [required]
    fn extract_transcriber_header<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> TranscriberHeaderChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, TranscriberPrefixNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, HeaderSepNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, FreeTextNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "transcriber_prefix", TranscriberPrefixNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "header_sep", HeaderSepNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "free_text", FreeTextNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        TranscriberHeaderChildren {
            child_0,
            child_1,
            child_2,
            child_3,
        }
    }
    ///Extract children from a `transcription_header` node.
    ///
    ///Production (children in order):
    ///- `transcription_prefix` [required]
    ///- `header_sep` [required]
    ///- `transcription_option` [required]
    ///- `newline` [required]
    fn extract_transcription_header<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> TranscriptionHeaderChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, TranscriptionPrefixNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, HeaderSepNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, TranscriptionOptionNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "transcription_prefix", TranscriptionPrefixNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "header_sep", HeaderSepNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "transcription_option", TranscriptionOptionNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        TranscriptionHeaderChildren {
            child_0,
            child_1,
            child_2,
            child_3,
        }
    }
    ///Extract children from a `types_header` node.
    ///
    ///Production (children in order):
    ///- `types_prefix` [required]
    ///- `header_sep` [required]
    ///- `types_design` [required]
    ///- `whitespaces` [optional]
    ///- `comma` [required]
    ///- `whitespaces` [optional]
    ///- `types_activity` [required]
    ///- `whitespaces` [optional]
    ///- `comma` [required]
    ///- `whitespaces` [optional]
    ///- `types_group` [required]
    ///- `newline` [required]
    fn extract_types_header<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> TypesHeaderChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, TypesPrefixNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, HeaderSepNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, TypesDesignNode<'tree>> = NodeSlot::Absent;
        let mut child_3: Option<WhitespacesNode<'tree>> = None;
        let mut child_4: NodeSlot<'tree, CommaNode<'tree>> = NodeSlot::Absent;
        let mut child_5: Option<WhitespacesNode<'tree>> = None;
        let mut child_6: NodeSlot<'tree, TypesActivityNode<'tree>> = NodeSlot::Absent;
        let mut child_7: Option<WhitespacesNode<'tree>> = None;
        let mut child_8: NodeSlot<'tree, CommaNode<'tree>> = NodeSlot::Absent;
        let mut child_9: Option<WhitespacesNode<'tree>> = None;
        let mut child_10: NodeSlot<'tree, TypesGroupNode<'tree>> = NodeSlot::Absent;
        let mut child_11: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "types_prefix", TypesPrefixNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "header_sep", HeaderSepNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "types_design", TypesDesignNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                if child.kind() == "whitespaces" && !child.is_error() {
                    child_3 = Some(WhitespacesNode(child));
                    idx += 1;
                }
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_4 = classify_child(child, "comma", CommaNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                if child.kind() == "whitespaces" && !child.is_error() {
                    child_5 = Some(WhitespacesNode(child));
                    idx += 1;
                }
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_6 = classify_child(child, "types_activity", TypesActivityNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                if child.kind() == "whitespaces" && !child.is_error() {
                    child_7 = Some(WhitespacesNode(child));
                    idx += 1;
                }
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_8 = classify_child(child, "comma", CommaNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                if child.kind() == "whitespaces" && !child.is_error() {
                    child_9 = Some(WhitespacesNode(child));
                    idx += 1;
                }
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_10 = classify_child(child, "types_group", TypesGroupNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_11 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        TypesHeaderChildren {
            child_0,
            child_1,
            child_2,
            child_3,
            child_4,
            child_5,
            child_6,
            child_7,
            child_8,
            child_9,
            child_10,
            child_11,
        }
    }
    ///Extract children from a `unsupported_dependent_tier` node.
    ///
    ///Production (children in order):
    ///- `(terminal)` [required]
    ///- `tier_sep` [required]
    ///- `(terminal)` [required]
    ///- `newline` [required]
    fn extract_unsupported_dependent_tier<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> UnsupportedDependentTierChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_1: NodeSlot<'tree, TierSepNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            idx += 1;
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "tier_sep", TierSepNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            idx += 1;
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        UnsupportedDependentTierChildren { child_1, child_3 }
    }
    ///Extract children from a `unsupported_header` node.
    ///
    ///Production (children in order):
    ///- `(terminal)` [required]
    ///- `header_sep` [required]
    ///- `rest_of_line` [required]
    ///- `newline` [required]
    fn extract_unsupported_header<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> UnsupportedHeaderChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_1: NodeSlot<'tree, HeaderSepNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, RestOfLineNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            idx += 1;
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "header_sep", HeaderSepNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "rest_of_line", RestOfLineNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        UnsupportedHeaderChildren {
            child_1,
            child_2,
            child_3,
        }
    }
    ///Extract children from a `unsupported_line` node.
    ///
    ///Production (children in order):
    ///- `(terminal)` [required]
    ///- `newline` [required]
    fn extract_unsupported_line<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> UnsupportedLineChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_1: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            idx += 1;
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        UnsupportedLineChildren { child_1 }
    }
    ///Extract children from a `utf8_header` node.
    ///
    ///Production (children in order):
    ///- `(terminal)` [required]
    ///- `newline` [required]
    fn extract_utf8_header<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> Utf8HeaderChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_1: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            idx += 1;
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        Utf8HeaderChildren { child_1 }
    }
    ///Extract children from a `utterance` node.
    ///
    ///Production (children in order):
    ///- `main_tier` [required]
    ///- `(terminal)` [required]
    fn extract_utterance<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> UtteranceChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, MainTierNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "main_tier", MainTierNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            idx += 1;
        }
        UtteranceChildren { child_0 }
    }
    ///Extract children from a `utterance_end` node.
    ///
    ///Production (children in order):
    ///- `terminator` [optional]
    ///- `final_codes` [optional]
    ///- `(terminal)` [optional]
    ///- `whitespaces` [optional]
    ///- `newline` [required]
    fn extract_utterance_end<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> UtteranceEndChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: Option<TerminatorNode<'tree>> = None;
        let mut child_1: Option<FinalCodesNode<'tree>> = None;
        let mut child_3: Option<WhitespacesNode<'tree>> = None;
        let mut child_4: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                if child.kind() == "terminator" && !child.is_error() {
                    child_0 = Some(TerminatorNode(child));
                    idx += 1;
                }
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                if child.kind() == "final_codes" && !child.is_error() {
                    child_1 = Some(FinalCodesNode(child));
                    idx += 1;
                }
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            idx += 1;
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                if child.kind() == "whitespaces" && !child.is_error() {
                    child_3 = Some(WhitespacesNode(child));
                    idx += 1;
                }
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_4 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        UtteranceEndChildren {
            child_0,
            child_1,
            child_3,
            child_4,
        }
    }
    ///Extract children from a `videos_header` node.
    ///
    ///Production (children in order):
    ///- `videos_prefix` [required]
    ///- `header_sep` [required]
    ///- `free_text` [required]
    ///- `newline` [required]
    fn extract_videos_header<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> VideosHeaderChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, VideosPrefixNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, HeaderSepNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, FreeTextNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "videos_prefix", VideosPrefixNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "header_sep", HeaderSepNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "free_text", FreeTextNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        VideosHeaderChildren {
            child_0,
            child_1,
            child_2,
            child_3,
        }
    }
    ///Extract children from a `warning_header` node.
    ///
    ///Production (children in order):
    ///- `warning_prefix` [required]
    ///- `header_sep` [required]
    ///- `free_text` [required]
    ///- `newline` [required]
    fn extract_warning_header<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> WarningHeaderChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, WarningPrefixNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, HeaderSepNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, FreeTextNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "warning_prefix", WarningPrefixNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "header_sep", HeaderSepNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "free_text", FreeTextNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        WarningHeaderChildren {
            child_0,
            child_1,
            child_2,
            child_3,
        }
    }
    ///Extract children from a `window_header` node.
    ///
    ///Production (children in order):
    ///- `window_prefix` [required]
    ///- `header_sep` [required]
    ///- `free_text` [required]
    ///- `newline` [required]
    fn extract_window_header<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> WindowHeaderChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, WindowPrefixNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, HeaderSepNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, FreeTextNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "window_prefix", WindowPrefixNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "header_sep", HeaderSepNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "free_text", FreeTextNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        WindowHeaderChildren {
            child_0,
            child_1,
            child_2,
            child_3,
        }
    }
    ///Extract children from a `wor_dependent_tier` node.
    ///
    ///Production (children in order):
    ///- `wor_tier_prefix` [required]
    ///- `tier_sep` [required]
    ///- `wor_tier_body` [required]
    fn extract_wor_dependent_tier<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> WorDependentTierChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_0: NodeSlot<'tree, WorTierPrefixNode<'tree>> = NodeSlot::Absent;
        let mut child_1: NodeSlot<'tree, TierSepNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, WorTierBodyNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_0 = classify_child(child, "wor_tier_prefix", WorTierPrefixNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "tier_sep", TierSepNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "wor_tier_body", WorTierBodyNode);
                idx += 1;
            }
        }
        WorDependentTierChildren {
            child_0,
            child_1,
            child_2,
        }
    }
    ///Extract children from a `wor_tier_body` node.
    ///
    ///Production (children in order):
    ///- `(terminal)` (field: `language_code`) [optional]
    ///- `(terminal)` [required]
    ///- `terminator` [optional]
    ///- `newline` [required]
    fn extract_wor_tier_body<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> WorTierBodyChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_2: Option<TerminatorNode<'tree>> = None;
        let mut child_3: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            idx += 1;
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            idx += 1;
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                if child.kind() == "terminator" && !child.is_error() {
                    child_2 = Some(TerminatorNode(child));
                    idx += 1;
                }
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        WorTierBodyChildren { child_2, child_3 }
    }
    ///Extract children from a `word_with_optional_annotations` node.
    ///
    ///Production (children in order):
    ///- `standalone_word` (field: `word`) [required]
    ///- `(terminal)` [optional]
    ///- `base_annotations` (field: `annotations`) [optional]
    fn extract_word_with_optional_annotations<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> WordWithOptionalAnnotationsChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut word: NodeSlot<'tree, StandaloneWordNode<'tree>> = NodeSlot::Absent;
        let mut annotations: Option<BaseAnnotationsNode<'tree>> = None;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                word = classify_child(child, "standalone_word", StandaloneWordNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            idx += 1;
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                if child.kind() == "base_annotations" && !child.is_error() {
                    annotations = Some(BaseAnnotationsNode(child));
                    idx += 1;
                }
            }
        }
        WordWithOptionalAnnotationsChildren { word, annotations }
    }
    ///Extract children from a `x_dependent_tier` node.
    ///
    ///Production (children in order):
    ///- `(terminal)` [required]
    ///- `tier_sep` [required]
    ///- `text_with_bullets` [required]
    ///- `newline` [required]
    fn extract_x_dependent_tier<'tree>(
        &mut self,
        node: tree_sitter::Node<'tree>,
    ) -> XDependentTierChildren<'tree> {
        let child_count = node.child_count();
        let mut idx: u32 = 0;
        let skip_extras = |node: tree_sitter::Node, idx: &mut u32, count: usize| {
            while (*idx as usize) < count {
                if let Some(child) = node.child(*idx) {
                    if matches!(child.kind(), "whitespaces") {
                        *idx += 1;
                        continue;
                    }
                }
                break;
            }
        };
        let mut child_1: NodeSlot<'tree, TierSepNode<'tree>> = NodeSlot::Absent;
        let mut child_2: NodeSlot<'tree, TextWithBulletsNode<'tree>> = NodeSlot::Absent;
        let mut child_3: NodeSlot<'tree, NewlineNode<'tree>> = NodeSlot::Absent;
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            idx += 1;
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_1 = classify_child(child, "tier_sep", TierSepNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_2 = classify_child(child, "text_with_bullets", TextWithBulletsNode);
                idx += 1;
            }
        }
        skip_extras(node, &mut idx, child_count);
        if (idx as usize) < child_count {
            if let Some(child) = node.child(idx) {
                child_3 = classify_child(child, "newline", NewlineNode);
                idx += 1;
            }
        }
        XDependentTierChildren {
            child_1,
            child_2,
            child_3,
        }
    }
}
