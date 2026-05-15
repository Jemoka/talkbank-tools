//! UD deprel newtype and deprel→POS constraint mapping.
//!
//! Provides typed distinction between UD-convention deprel labels (lowercase,
//! colon-separated) and CHAT GRA labels (uppercase, dash-separated), with a
//! single conversion boundary via [`UdDeprel::to_chat_gra`].

use talkbank_model::model::dependent_tier::GrammaticalRelationType;

use crate::morphosyntax::UniversalPos;

// ---------------------------------------------------------------------------
// UdDeprel newtype
// ---------------------------------------------------------------------------

/// A Universal Dependencies dependency relation label in lowercase convention.
///
/// This is the representation Stanza produces (e.g., `"advmod"`, `"nsubj:pass"`,
/// `"flat"`). It is **not** interchangeable with CHAT GRA labels, which use
/// uppercase with dashes (e.g., `"ADVMOD"`, `"NSUBJ-PASS"`, `"FLAT"`).
///
/// Use [`to_chat_gra`](Self::to_chat_gra) to convert to the CHAT representation
/// at the serialization boundary.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UdDeprel(String);

impl UdDeprel {
    /// Wrap a raw UD deprel string. The caller is responsible for ensuring
    /// the value is in UD lowercase convention.
    pub fn new(deprel: impl Into<String>) -> Self {
        Self(deprel.into())
    }

    /// The raw UD label (lowercase, colon-separated subtypes).
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The base relation (before the first colon), e.g., `"obl"` from `"obl:arg"`.
    pub fn base(&self) -> &str {
        self.0.split(':').next().unwrap_or(&self.0)
    }

    /// Convert to CHAT GRA convention: uppercase, colons replaced by dashes.
    ///
    /// This is the single conversion boundary between UD and CHAT deprel
    /// representations. All GRA tier construction should go through this method.
    pub fn to_chat_gra(&self) -> GrammaticalRelationType {
        let chat = self.0.to_uppercase().replace(':', "-");
        GrammaticalRelationType::new(chat)
    }
}

impl std::fmt::Display for UdDeprel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

// ---------------------------------------------------------------------------
// POS constraint
// ---------------------------------------------------------------------------

/// A set of valid Universal POS tags inferred from a UD dependency relation.
///
/// The mapping is cross-linguistically valid by UD design: a word functioning
/// as `advmod` must be an adverb in any language, a word functioning as `det`
/// must be a determiner, etc.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PosConstraint {
    /// Exactly one valid POS — the deprel uniquely determines it.
    Exact(UniversalPos),
    /// A small set of valid POS tags (2-4 candidates).
    OneOf(Vec<UniversalPos>),
    /// No constraint — deprel does not narrow POS meaningfully.
    /// Used for unknown or language-specific deprels.
    Unconstrained,
}

impl PosConstraint {
    /// Check whether a POS tag satisfies this constraint.
    pub fn contains(&self, pos: &UniversalPos) -> bool {
        match self {
            Self::Exact(p) => p == pos,
            Self::OneOf(set) => set.contains(pos),
            Self::Unconstrained => true,
        }
    }

    /// Return the most likely POS from the constraint set.
    ///
    /// For `Exact`, returns that POS. For `OneOf`, returns the first element
    /// (ordered by frequency/likelihood in the mapping table). For
    /// `Unconstrained`, returns `None`.
    pub fn most_likely(&self) -> Option<UniversalPos> {
        match self {
            Self::Exact(p) => Some(*p),
            Self::OneOf(set) => set.first().copied(),
            Self::Unconstrained => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Deprel → POS constraint mapping
// ---------------------------------------------------------------------------

/// Map a UD dependency relation to the set of valid UPOS tags.
///
/// This mapping encodes cross-linguistic constraints from the Universal
/// Dependencies annotation scheme. A word with deprel `advmod` must be an
/// adverb in any language; a word with deprel `det` must be a determiner.
///
/// The constraint sets are ordered by frequency/likelihood: for `OneOf`,
/// the first element is the most common POS for that relation.
pub fn deprel_to_pos_constraint(deprel: &UdDeprel) -> PosConstraint {
    let base = deprel.base();

    match base {
        // Unambiguous — exactly one POS
        "det" => PosConstraint::Exact(UniversalPos::Det),
        "amod" => PosConstraint::Exact(UniversalPos::Adj),
        "advmod" => PosConstraint::Exact(UniversalPos::Adv),
        "case" => PosConstraint::Exact(UniversalPos::Adp),
        "mark" => PosConstraint::Exact(UniversalPos::Sconj),
        "cc" => PosConstraint::Exact(UniversalPos::Cconj),
        "ccomp" => PosConstraint::Exact(UniversalPos::Verb),
        "advcl" => PosConstraint::Exact(UniversalPos::Verb),
        "aux" => PosConstraint::Exact(UniversalPos::Aux),
        "cop" => PosConstraint::Exact(UniversalPos::Aux),
        "nummod" => PosConstraint::Exact(UniversalPos::Num),
        "discourse" => PosConstraint::Exact(UniversalPos::Intj),
        "punct" => PosConstraint::Exact(UniversalPos::Punct),

        // Narrow — 2-3 candidates
        "nsubj" => PosConstraint::OneOf(vec![
            UniversalPos::Noun,
            UniversalPos::Pron,
            UniversalPos::Propn,
        ]),
        "obj" => PosConstraint::OneOf(vec![
            UniversalPos::Noun,
            UniversalPos::Pron,
            UniversalPos::Propn,
        ]),
        "iobj" => PosConstraint::OneOf(vec![
            UniversalPos::Noun,
            UniversalPos::Pron,
            UniversalPos::Propn,
        ]),
        "obl" => PosConstraint::OneOf(vec![UniversalPos::Noun, UniversalPos::Pron]),
        "nmod" => PosConstraint::OneOf(vec![UniversalPos::Noun, UniversalPos::Propn]),
        "appos" => PosConstraint::OneOf(vec![UniversalPos::Noun, UniversalPos::Propn]),
        "xcomp" => PosConstraint::OneOf(vec![UniversalPos::Verb, UniversalPos::Adj]),
        "acl" => PosConstraint::OneOf(vec![UniversalPos::Verb, UniversalPos::Adj]),
        "vocative" => PosConstraint::OneOf(vec![UniversalPos::Noun, UniversalPos::Propn]),

        // Broad — multiple candidates
        "root" => PosConstraint::OneOf(vec![
            UniversalPos::Verb,
            UniversalPos::Noun,
            UniversalPos::Adj,
        ]),
        // `flat` is the parser's fallback for unknown/foreign words. Since
        // the primary model has no lexical knowledge of @s words, any POS
        // the secondary model returns is valid. Use Unconstrained to defer
        // entirely to the secondary model.
        "flat" => PosConstraint::Unconstrained,
        "conj" => PosConstraint::Unconstrained,
        "parataxis" => PosConstraint::Unconstrained,
        "dep" => PosConstraint::Unconstrained,

        // Unknown or language-specific deprels
        _ => PosConstraint::Unconstrained,
    }
}

/// Refine a POS constraint using evidence from the word's dependents.
///
/// If a word has a `det` dependent, it must be a noun (definitively).
/// If it has an `nsubj` dependent, it must be a verb or adjective.
/// These constraints further narrow the set from `deprel_to_pos_constraint`.
pub fn refine_with_dependents(
    constraint: &PosConstraint,
    dependent_deprels: &[UdDeprel],
) -> PosConstraint {
    let mut result = constraint.clone();
    for dep in dependent_deprels {
        let base = dep.base();
        let narrowing: Option<Vec<UniversalPos>> = match base {
            "det" => Some(vec![UniversalPos::Noun, UniversalPos::Propn]),
            "nsubj" | "csubj" => Some(vec![UniversalPos::Verb, UniversalPos::Adj]),
            "obj" | "iobj" => Some(vec![UniversalPos::Verb]),
            "case" => Some(vec![
                UniversalPos::Noun,
                UniversalPos::Pron,
                UniversalPos::Propn,
            ]),
            _ => None,
        };
        if let Some(narrow) = narrowing {
            result = intersect_constraint(&result, &narrow);
        }
    }
    result
}

/// Intersect a constraint with a set of allowed POS tags.
fn intersect_constraint(constraint: &PosConstraint, allowed: &[UniversalPos]) -> PosConstraint {
    match constraint {
        PosConstraint::Exact(p) => {
            if allowed.contains(p) {
                PosConstraint::Exact(*p)
            } else {
                constraint.clone()
            }
        }
        PosConstraint::OneOf(set) => {
            let narrowed: Vec<UniversalPos> = set
                .iter()
                .filter(|p| allowed.contains(p))
                .copied()
                .collect();
            match narrowed.len() {
                0 => constraint.clone(),
                1 => PosConstraint::Exact(narrowed[0]),
                _ => PosConstraint::OneOf(narrowed),
            }
        }
        PosConstraint::Unconstrained => {
            if allowed.len() == 1 {
                PosConstraint::Exact(allowed[0])
            } else {
                PosConstraint::OneOf(allowed.to_vec())
            }
        }
    }
}

/// Infer the correct UD deprel from the resolved POS and the head's POS.
///
/// Called when the primary model produced an unreliable deprel (e.g., `flat`
/// for unknown words, or a deprel that contradicts the resolved POS like
/// `obl` for a DET). Returns the corrected deprel, or `None` if no
/// correction is applicable.
pub fn infer_deprel_from_pos(
    resolved_pos: UniversalPos,
    head_pos: Option<UniversalPos>,
    has_case_dependent: bool,
) -> Option<UdDeprel> {
    let head = head_pos?;
    match (resolved_pos, head) {
        (UniversalPos::Adv, UniversalPos::Verb | UniversalPos::Adj | UniversalPos::Adv) => {
            Some(UdDeprel::new("advmod"))
        }
        (UniversalPos::Adj, UniversalPos::Noun | UniversalPos::Propn) => {
            Some(UdDeprel::new("amod"))
        }
        (UniversalPos::Det, UniversalPos::Noun | UniversalPos::Propn) => Some(UdDeprel::new("det")),
        (UniversalPos::Noun | UniversalPos::Propn, UniversalPos::Verb) => {
            if has_case_dependent {
                Some(UdDeprel::new("obl"))
            } else {
                Some(UdDeprel::new("obj"))
            }
        }
        (UniversalPos::Noun | UniversalPos::Propn, UniversalPos::Noun | UniversalPos::Propn) => {
            Some(UdDeprel::new("nmod"))
        }
        _ => None,
    }
}
