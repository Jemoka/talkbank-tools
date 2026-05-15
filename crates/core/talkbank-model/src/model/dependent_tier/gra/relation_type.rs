//! Interned grammatical-relation labels used by `%gra`.
//!
//! Relation labels are interned because `%gra` tiers repeat a small vocabulary
//! (`SUBJ`, `OBJ`, `ROOT`, etc.) across large corpora.
//! Interning keeps these comparisons pointer-equality cheap during alignment and
//! serialization passes.
//!
//! CHAT reference anchor:
//! - [Grammatical relations tier](https://talkbank.org/0info/manuals/CHAT.html#GrammaticalRelations_Tier)

use crate::interned_newtype;

interned_newtype!(
    /// Grammatical relation type (Universal Dependencies).
    ///
    /// Relation labels from the Universal Dependencies framework describing syntactic
    /// relationships between words in a sentence.
    ///
    /// The model intentionally treats relation labels as open-text tokens: it
    /// preserves corpus-provided values as-is and leaves strict vocabulary
    /// policy to higher-level validation.
    ///
    /// # Common Universal Dependencies Relations
    ///
    /// **Core arguments:**
    /// - **ROOT**: Root of the sentence (head = 0)
    /// - **SUBJ**: Subject (nominal subject)
    /// - **OBJ**: Direct object
    /// - **IOBJ**: Indirect object
    /// - **CSUBJ**: Clausal subject
    /// - **CCOMP**: Clausal complement
    ///
    /// **Nominal dependents:**
    /// - **DET**: Determiner (the, a, this)
    /// - **AMOD**: Adjectival modifier
    /// - **NMOD**: Nominal modifier
    /// - **NUM**: Numeric modifier
    /// - **POSS**: Possessive modifier
    ///
    /// **Function words:**
    /// - **AUX**: Auxiliary verb
    /// - **CASE**: Case marking (prepositions, postpositions)
    /// - **MARK**: Marker (subordinating conjunction)
    /// - **PUNCT**: Punctuation
    ///
    /// **Other:**
    /// - **ADV**: Adverbial modifier
    /// - **CONJ**: Conjunct
    /// - **CC**: Coordinating conjunction
    /// - **NEG**: Negation
    ///
    /// # CHAT Format Examples
    ///
    /// ```text
    /// *CHI: I eat cookies .
    /// %mor: pro:sub|I v|eat det:art|the n|cookie-PL .
    /// %gra: 1|2|SUBJ 2|0|ROOT 3|2|OBJ 4|2|PUNCT
    /// ```
    ///
    /// # References
    ///
    /// - [Universal Dependencies Relations](https://universaldependencies.org/u/dep/)
    /// - [CHAT Manual: Grammatical Relations](https://talkbank.org/0info/manuals/CHAT.html#GrammaticalRelations_Tier)
    pub struct GrammaticalRelationType,
    interner: crate::model::pos_interner()
);

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    /// Identical labels are interned to the same backing allocation.
    ///
    /// Pointer equality keeps hot-path comparisons cheap in parser/model code.
    #[test]
    fn test_gra_type_interning() {
        let root1 = GrammaticalRelationType::new("ROOT");
        let root2 = GrammaticalRelationType::new("ROOT");

        // Same Arc (pointer equality) - strings should be interned
        assert!(Arc::ptr_eq(&root1.0, &root2.0));
        assert_eq!(root1.as_str(), "ROOT");
        assert_eq!(root2.as_str(), "ROOT");
    }

    /// Distinct labels keep distinct interned allocations.
    ///
    /// This prevents accidental aliasing across unrelated dependency labels.
    #[test]
    fn test_gra_type_different_values() {
        let root = GrammaticalRelationType::new("ROOT");
        let subj = GrammaticalRelationType::new("SUBJ");

        // Different values - different Arcs
        assert!(!Arc::ptr_eq(&root.0, &subj.0));
        assert_eq!(root.as_str(), "ROOT");
        assert_eq!(subj.as_str(), "SUBJ");
    }

    /// Display output preserves the original relation label text.
    ///
    /// This is required for lossless `%gra` roundtripping.
    #[test]
    fn test_gra_type_display() {
        let rel = GrammaticalRelationType::new("OBJ");
        assert_eq!(rel.to_string(), "OBJ");
        assert_eq!(format!("{}", rel), "OBJ");
    }
}
