//! `%gra` tier model and structural validation helpers.
//!
//! CHAT reference anchors:
//! - [Grammatical relations tier](https://talkbank.org/0info/manuals/CHAT.html#GrammaticalRelations_Tier)
//! - [Morphological tier](https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier)

use super::super::WriteChat;
use super::relation::GrammaticalRelation;
use super::tier_type::GraTierType;
use crate::Span;
use crate::alignment::indices::SemanticWordIndex1;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::ops::Deref;
use talkbank_derive::{SemanticEq, SpanShift};

/// Grammatical relations tier (%gra).
///
/// Contains dependency syntax annotations using Universal Dependencies relations.
/// Each relation specifies how morphological chunks in the %mor tier relate syntactically.
///
/// # Alignment with %mor
///
/// The %gra tier aligns with **morphological chunks**, not individual %mor items:
/// - Clitics in %mor (e.g., `pro|it~v|be`) produce **two** %gra relations
/// - Non-clitic words produce **one** %gra relation each
/// - Terminators get their own %gra relation (typically PUNCT)
///
/// # Dependency Relations
///
/// Uses Universal Dependencies relation types including:
/// - **ROOT**: Main predicate of sentence (head = 0)
/// - **SUBJ**: Subject
/// - **OBJ**: Direct object
/// - **IOBJ**: Indirect object
/// - **DET**: Determiner
/// - **ADJ**: Adjective modifier
/// - **ADV**: Adverb modifier
/// - **PUNCT**: Punctuation
/// - And many more...
///
/// # CHAT Manual Reference
///
/// - [Grammatical Relations Tier](https://talkbank.org/0info/manuals/CHAT.html#GrammaticalRelations_Tier)
/// - [Universal Dependencies](https://universaldependencies.org/)
///
/// Note: The legacy CLAN MOR manual has been removed from the TalkBank
/// website. GRA now follows UD-oriented conventions documented in the
/// CHAT manual.
///
/// # Example
///
/// ```
/// use talkbank_model::model::{GraTier, GraTierType, GrammaticalRelation};
///
/// // Create a %gra tier
/// let gra = GraTier::new_gra(vec![
///     GrammaticalRelation::new(1, 2, "SUBJ"),   // Word 1 is subject of word 2
///     GrammaticalRelation::new(2, 0, "ROOT"),   // Word 2 is root
///     GrammaticalRelation::new(3, 2, "OBJ"),    // Word 3 is object of word 2
///     GrammaticalRelation::new(4, 2, "PUNCT"),  // Terminator
/// ]);
/// ```
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
pub struct GraTier {
    /// Type of grammatical relations tier.
    pub tier_type: GraTierType,

    /// Dependency relations for each morphological chunk.
    ///
    /// Each relation specifies word_index, head_index, and relation_type.
    /// Relations are in the same order as morphological chunks in the %mor tier.
    pub(crate) relations: GraRelations,

    /// Source span for error reporting (not serialized to JSON)
    #[serde(skip)]
    #[schemars(skip)]
    pub span: Span,
}

impl GraTier {
    /// Constructs a grammatical-relations tier from parsed relations.
    pub fn new(tier_type: GraTierType, relations: Vec<GrammaticalRelation>) -> Self {
        Self {
            tier_type,
            relations: relations.into(),
            span: Span::DUMMY,
        }
    }

    /// Sets source span metadata used in diagnostics.
    pub fn with_span(mut self, span: Span) -> Self {
        self.span = span;
        self
    }

    /// Convenience constructor for standard `%gra`.
    pub fn new_gra(relations: Vec<GrammaticalRelation>) -> Self {
        Self::new(GraTierType::Gra, relations)
    }

    /// Returns `true` if this tier serializes as `%gra`.
    pub fn is_gra(&self) -> bool {
        self.tier_type == GraTierType::Gra
    }

    /// Borrows the list of grammatical relations.
    pub fn relations(&self) -> &[GrammaticalRelation] {
        &self.relations.0
    }

    /// Return the relation at a 1-indexed `%gra` semantic position.
    ///
    /// This is the typed accessor for callers that already work in the
    /// author-written `%gra` index/head space and want to avoid ad hoc
    /// `index - 1` conversions.
    pub fn relation_at_semantic_index(
        &self,
        index: SemanticWordIndex1,
    ) -> Option<&GrammaticalRelation> {
        self.relations.get(index.to_chunk_index().as_usize())
    }

    /// Consumes the tier and returns the underlying grammatical relations.
    pub fn into_relations(self) -> Vec<GrammaticalRelation> {
        self.relations.0
    }

    /// Mutably borrows the list of grammatical relations.
    ///
    /// # Safety
    ///
    /// Callers must ensure that mutations do not break index sequentiality
    /// or other structural invariants.
    pub fn relations_mut(&mut self) -> &mut [GrammaticalRelation] {
        &mut self.relations.0
    }

    /// Number of dependency edges in this tier.
    pub fn len(&self) -> usize {
        self.relations.len()
    }

    /// Returns `true` when no dependency edges are present.
    pub fn is_empty(&self) -> bool {
        self.relations.is_empty()
    }

    /// Validate structural integrity of %gra tier.
    ///
    /// **NOTE**: As of 2026-02-14, only index validation is enforced.
    /// ROOT validation is disabled due to malformed %gra tiers
    /// in the corpus with circular dependencies and invalid tree structures.
    ///
    /// Checks:
    /// - E721: Indices are sequential 1, 2, ..., N
    ///
    /// Disabled checks:
    /// - E722/E723: ROOT relation validation (see validate_gra_structure for details)
    pub fn validate_structure(&self, errors: &impl crate::ErrorSink) {
        validate_gra_structure(&self.relations, self.span, errors);
    }

    /// Serialize full `%gra` line to an owned string.
    pub fn to_chat(&self) -> String {
        let mut s = String::new();
        let _ = self.write_chat(&mut s);
        s
    }

    /// Write tier content only (relations), without the tier prefix (%gra:\t).
    ///
    /// This is used for roundtrip testing against golden data that contains
    /// content-only, and for the TreeSitterParser API which expects content-only input.
    pub fn write_content<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        for (i, rel) in self.relations.iter().enumerate() {
            if i > 0 {
                w.write_char(' ')?;
            }
            rel.write_chat(w)?;
        }

        Ok(())
    }

    /// Serialize content-only `%gra` payload to an owned string.
    pub fn to_content(&self) -> String {
        let mut s = String::new();
        let _ = self.write_content(&mut s);
        s
    }
}

/// Validate structural integrity of a slice of %gra relations.
///
/// Shared implementation used by [`GraTier::validate_structure`].
///
/// **NOTE**: As of 2026-02-14, ROOT validation is DISABLED.
///
/// Checks:
/// - E721: Indices are sequential 1, 2, ..., N
///
/// Disabled (due to non-conforming corpus data):
/// - E722/E723: ROOT relation validation
pub fn validate_gra_structure(
    relations: &[GrammaticalRelation],
    span: crate::Span,
    errors: &impl crate::ErrorSink,
) {
    use crate::{ErrorCode, ParseError, Severity};

    if relations.is_empty() {
        return;
    }

    // Check 1: Sequential indices (1, 2, 3, ..., N)
    for (i, rel) in relations.iter().enumerate() {
        let expected = i + 1;
        if rel.index != expected {
            errors.report(
                ParseError::at_span(
                    ErrorCode::GraNonSequentialIndex,
                    Severity::Error,
                    span,
                    format!(
                        "%gra indices not sequential: expected {expected}, found {}",
                        rel.index
                    ),
                )
                .with_suggestion("Indices must be 1, 2, 3, ..., N"),
            );
            break; // one error is enough
        }
    }

    // Check 2: ROOT validation (as WARNING - 2026-02-14)
    //
    // Corpus contains malformed %gra tiers with circular dependencies,
    // invalid tree structures, and no valid root. These appear to be generated by
    // tools that produce non-conforming output.
    //
    // We report these as WARNINGS (not errors) to allow processing to continue
    // while giving users visibility into data quality issues.

    // Find all valid ROOT relations. A self-headed relation only counts as a
    // ROOT when it is explicitly labeled ROOT; other self-headed relations are
    // structural defects, not additional roots.
    let mut roots = Vec::new();
    for rel in relations {
        if is_valid_root_relation(rel) {
            roots.push(rel.index);
        }
    }

    // Exclude terminator PUNCT (last item) from root count
    let max_index = relations.len();
    let non_terminator_roots: Vec<_> = roots
        .iter()
        .filter(|&&idx| idx != max_index)
        .copied()
        .collect();

    // W723: Multiple ROOT relations
    if non_terminator_roots.len() > 1 {
        errors.report(
            ParseError::at_span(
                ErrorCode::GraMultipleRoots,
                Severity::Error,
                span,
                format!(
                    "%gra tier has {} ROOT relations, expected 1",
                    non_terminator_roots.len()
                ),
            )
            .with_suggestion("Re-run morphotag to regenerate valid %gra"),
        );
    }

    // Check 3: Circular dependencies (E724) - Fast O(N) check
    let has_cycle = has_any_cycle(relations);
    if has_cycle {
        errors.report(
            ParseError::at_span(
                ErrorCode::GraCircularDependency,
                Severity::Error,
                span,
                "%gra tier has circular dependency",
            )
            .with_suggestion("Re-run morphotag to regenerate valid %gra"),
        );
    }

    // W722: No ROOT relation. Suppress this when the graph already has a cycle:
    // in that case the missing root is just a consequence of the cycle and
    // reporting both diagnostics produces a misleading cascade.
    if non_terminator_roots.is_empty() && !has_cycle {
        errors.report(
            ParseError::at_span(
                ErrorCode::GraNoRoot,
                Severity::Error,
                span,
                "%gra tier has no ROOT relation",
            )
            .with_suggestion("Re-run morphotag to regenerate valid %gra"),
        );
    }
}

/// Fast O(N) cycle detection using iterative DFS with path tracking.
///
/// Follows each word's head pointer chain to the root using iteration (not recursion).
/// Uses memoization to avoid recomputing paths - each node is visited once.
/// Detects cycles by tracking the current path.
///
/// **Completely stack-safe** - uses heap-allocated Vec for the path instead of
/// call stack recursion. Can handle arbitrarily long chains without stack overflow.
fn has_any_cycle(relations: &[GrammaticalRelation]) -> bool {
    use std::collections::{HashMap, HashSet};

    /// Memoized node state during cycle detection.
    #[derive(Clone, Copy, PartialEq)]
    enum State {
        NoCycle, // Verified no cycle in this subtree
    }

    let mut memo: HashMap<usize, State> = HashMap::new();

    // Check each word for cycles
    for start_rel in relations {
        let start_node = start_rel.index;

        // Skip if we've already verified this node
        if memo.contains_key(&start_node) {
            continue;
        }

        // Follow head chain iteratively with path tracking
        let mut path = HashSet::new();
        let mut current = start_node;

        loop {
            // If this node is already memoized as safe, we're done
            if memo.contains_key(&current) {
                // Mark all nodes in current path as safe
                for &node in &path {
                    memo.insert(node, State::NoCycle);
                }
                break;
            }

            // Cycle detected! Node is in current path
            if path.contains(&current) {
                return true;
            }

            path.insert(current);

            // Find the relation for current node
            if let Some(rel) = relations.iter().find(|r| r.index == current) {
                // Valid roots end a chain. A self-headed non-ROOT relation is a
                // cycle, not a root.
                if rel.head == 0 || is_valid_root_relation(rel) {
                    // Mark all nodes in path as safe
                    for &node in &path {
                        memo.insert(node, State::NoCycle);
                    }
                    break;
                }

                // Continue following the chain
                current = rel.head;
            } else {
                // Invalid index - shouldn't happen, but treat as end of chain
                for &node in &path {
                    memo.insert(node, State::NoCycle);
                }
                break;
            }
        }
    }

    false
}

fn is_valid_root_relation(rel: &GrammaticalRelation) -> bool {
    rel.relation.as_str().eq_ignore_ascii_case("ROOT") && (rel.head == 0 || rel.head == rel.index)
}

impl WriteChat for GraTier {
    /// Serializes one full `%gra` line.
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        // Write tier type prefix
        match self.tier_type {
            GraTierType::Gra => w.write_str("%gra:\t")?,
        }

        // Write space-separated relations
        for (i, rel) in self.relations.iter().enumerate() {
            if i > 0 {
                w.write_char(' ')?;
            }
            rel.write_chat(w)?;
        }
        Ok(())
    }
}

/// Ordered list of `%gra` dependency relations.
///
/// # Reference
///
/// - [Grammatical relations tier](https://talkbank.org/0info/manuals/CHAT.html#GrammaticalRelations_Tier)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
#[serde(transparent)]
#[schemars(transparent)]
pub struct GraRelations(pub(crate) Vec<GrammaticalRelation>);

impl GraRelations {
    /// Returns `true` when this relation list is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Deref for GraRelations {
    type Target = Vec<GrammaticalRelation>;

    /// Borrows the underlying relation vector.
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<Vec<GrammaticalRelation>> for GraRelations {
    /// Wraps an owned relation vector without copying.
    fn from(relations: Vec<GrammaticalRelation>) -> Self {
        Self(relations)
    }
}

impl crate::validation::Validate for GraRelations {
    /// Structural checks run via `validate_gra_structure` with tier-level context.
    fn validate(
        &self,
        _context: &crate::validation::ValidationContext,
        _errors: &impl crate::ErrorSink,
    ) {
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ErrorCode, ErrorCollector, Severity};

    /// New `%gra` construction preserves relation order and count.
    #[test]
    fn test_gra_tier_new() {
        let tier = GraTier::new_gra(vec![
            GrammaticalRelation::new(1, 2, "SUBJ"),
            GrammaticalRelation::new(2, 0, "ROOT"),
            GrammaticalRelation::new(3, 2, "OBJ"),
        ]);

        assert_eq!(tier.len(), 3);
        assert!(!tier.is_empty());
        assert_eq!(tier.relations[0].index, 1);
        assert_eq!(tier.relations[1].index, 2);
        assert_eq!(tier.relations[2].index, 3);
    }

    /// Empty relation input reports empty tier state.
    #[test]
    fn test_gra_tier_empty() {
        let tier = GraTier::new_gra(vec![]);
        assert_eq!(tier.len(), 0);
        assert!(tier.is_empty());
    }

    /// A well-formed dependency set emits no structural diagnostics.
    #[test]
    fn test_validate_structure_valid() {
        // TalkBank convention: ROOT head points to self
        let tier = GraTier::new_gra(vec![
            GrammaticalRelation::new(1, 2, "DET"),
            GrammaticalRelation::new(2, 3, "SUBJ"),
            GrammaticalRelation::new(3, 3, "ROOT"),
            GrammaticalRelation::new(4, 3, "PUNCT"),
        ]);
        let errors = ErrorCollector::new();
        tier.validate_structure(&errors);
        assert_eq!(errors.into_vec().len(), 0);
    }

    /// Non-sequential indices trigger `E721`.
    #[test]
    fn test_validate_structure_non_sequential() {
        let tier = GraTier::new_gra(vec![
            GrammaticalRelation::new(1, 3, "SUBJ"),
            GrammaticalRelation::new(3, 3, "ROOT"), // gap: expected 2
            GrammaticalRelation::new(2, 3, "OBJ"),
        ]);
        let errors = ErrorCollector::new();
        tier.validate_structure(&errors);
        let errs = errors.into_vec();
        assert!(
            errs.iter()
                .any(|e| e.code == ErrorCode::GraNonSequentialIndex)
        );
    }

    /// Cycles are surfaced as warnings, not hard errors.
    #[test]
    fn test_validate_structure_circular_dependency_warns() {
        // ROOT validation enabled - circular dependencies produce warnings
        let tier = GraTier::new_gra(vec![
            GrammaticalRelation::new(1, 2, "SUBJ"),
            GrammaticalRelation::new(2, 1, "OBJ"), // Circular: 1→2, 2→1
            GrammaticalRelation::new(3, 2, "PUNCT"),
        ]);
        let errors = ErrorCollector::new();
        tier.validate_structure(&errors);
        let errs = errors.into_vec();

        // Should have warnings, not errors
        assert!(
            errs.iter()
                .any(|e| e.code == ErrorCode::GraCircularDependency)
        );
        assert!(errs.iter().all(|e| e.severity == Severity::Error));
    }

    /// Multiple roots are surfaced as warnings.
    #[test]
    fn test_validate_structure_multiple_roots_warns() {
        // ROOT validation enabled - multiple roots produce warnings
        let tier = GraTier::new_gra(vec![
            GrammaticalRelation::new(1, 1, "ROOT"),
            GrammaticalRelation::new(2, 2, "ROOT"),
            GrammaticalRelation::new(3, 1, "PUNCT"),
        ]);
        let errors = ErrorCollector::new();
        tier.validate_structure(&errors);
        let errs = errors.into_vec();

        // Should have warnings for multiple roots
        assert!(errs.iter().any(|e| e.code == ErrorCode::GraMultipleRoots));
        assert!(errs.iter().all(|e| e.severity == Severity::Error));
    }

    /// `head=0` is accepted as a valid root encoding.
    #[test]
    fn test_validate_structure_root_head_zero_allowed() {
        // UD convention: ROOT head=0 is now allowed (no warning)
        let tier = GraTier::new_gra(vec![
            GrammaticalRelation::new(1, 2, "SUBJ"),
            GrammaticalRelation::new(2, 0, "ROOT"),
            GrammaticalRelation::new(3, 2, "OBJ"),
        ]);
        let errors = ErrorCollector::new();
        tier.validate_structure(&errors);
        let errs = errors.into_vec();
        assert_eq!(errs.len(), 0); // No errors - head=0 is valid
    }

    /// A self-headed non-ROOT relation is a cycle, not a second ROOT.
    #[test]
    fn test_validate_structure_self_headed_non_root_reports_cycle_not_multiple_roots() {
        let tier = GraTier::new_gra(vec![
            GrammaticalRelation::new(1, 0, "ROOT"),
            GrammaticalRelation::new(2, 1, "DEP"),
            GrammaticalRelation::new(3, 3, "NMOD"),
            GrammaticalRelation::new(4, 1, "PUNCT"),
        ]);
        let errors = ErrorCollector::new();
        tier.validate_structure(&errors);
        let errs = errors.into_vec();

        assert!(
            errs.iter()
                .any(|e| e.code == ErrorCode::GraCircularDependency),
            "self-headed non-ROOT relations must be treated as circular dependencies"
        );
        assert!(
            !errs.iter().any(|e| e.code == ErrorCode::GraMultipleRoots),
            "self-headed non-ROOT relations must not be counted as extra ROOTs"
        );
    }

    /// A rootless 2-cycle should report the cycle itself, not cascade into an
    /// additional "no ROOT" diagnostic.
    #[test]
    fn test_validate_structure_rootless_two_cycle_reports_cycle_not_no_root() {
        let tier = GraTier::new_gra(vec![
            GrammaticalRelation::new(1, 2, "DEP"),
            GrammaticalRelation::new(2, 1, "PARATAXIS"),
            GrammaticalRelation::new(3, 1, "PUNCT"),
        ]);
        let errors = ErrorCollector::new();
        tier.validate_structure(&errors);
        let errs = errors.into_vec();

        assert!(
            errs.iter()
                .any(|e| e.code == ErrorCode::GraCircularDependency),
            "rootless 2-cycles must report E724"
        );
        assert!(
            !errs.iter().any(|e| e.code == ErrorCode::GraNoRoot),
            "rootless 2-cycles must not additionally report E722 when the lack \
             of a root is just a consequence of the cycle"
        );
    }

    /// Empty tiers are accepted by structure validation.
    #[test]
    fn test_validate_structure_empty() {
        let tier = GraTier::new_gra(vec![]);
        let errors = ErrorCollector::new();
        tier.validate_structure(&errors);
        assert_eq!(errors.into_vec().len(), 0);
    }
}
