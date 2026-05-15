//! CLAN MOR → Universal Dependencies UPOS mapping.
//!
//! CHAT main-tier words may carry a `$POS` suffix that encodes the
//! transcriber's part-of-speech annotation using CLAN's MOR conventions
//! (e.g. `$n`, `$v`, `$pro:per`, `$n:prop`). Tools that produce
//! Universal Dependencies output (Stanza, UDPipe, spaCy) use the UPOS
//! tagset defined by <https://universaldependencies.org/u/pos/>.
//!
//! This module provides the canonical CLAN → UD UPOS mapping so any
//! downstream consumer — morphotag pipelines, BA2-vs-BA3 parity
//! audits, cross-tool reconciliation, validators that compare
//! transcriber annotations against automatic tags — can normalize
//! both sides to a single tagset.
//!
//! ## Why it lives here
//!
//! `PosCategory` already models the CLAN-side tag; this module is its
//! natural companion for the UD side. Keeping the mapping in
//! `talkbank-model` rather than in any particular consumer crate
//! means:
//!
//! * One authoritative table, versioned with the CHAT model itself.
//! * No UD-type dependency leaks into `talkbank-model` — the output
//!   is a `&'static str` with the canonical UPOS name, which callers
//!   parse into whatever typed enum they use internally (e.g.,
//!   `UniversalPos` in `batchalign-chat-ops`).
//! * Tests live alongside every other `PosCategory` test.
//!
//! ## Coverage policy
//!
//! The mapping is intentionally conservative. Only well-known CLAN
//! tags are recognized. Unknown tags return `None`, and callers
//! decide whether to fall back to automatic analysis or treat the
//! hint as absent. This is safer than guessing — a wrong mapping
//! propagates silently into morphotag output.
//!
//! ## Refinements
//!
//! CLAN tags often carry a colon-separated refinement
//! (`pro:per`, `det:dem`, `n:prop`, `adv:temp`). UD UPOS is a coarse
//! tagset: refinements that don't affect UPOS are stripped (the
//! coarse category decides the mapping). The one refinement that
//! does cross a UPOS boundary is `n:prop` → `PROPN` (vs plain `n` →
//! `NOUN`); this is handled by the `clan_to_ud_upos` function which
//! inspects the refinement before falling back to the coarse head.
//!
//! References:
//! - CHAT manual, §11 Morphosyntactic Coding:
//!   <https://talkbank.org/0info/manuals/CHAT.html>
//! - UD UPOS: <https://universaldependencies.org/u/pos/>

/// Map a CLAN POS tag (without the leading `$`) to a UD UPOS name.
///
/// Accepts tags in CLAN's native form, including colon-refined
/// variants (`pro:per`, `det:dem`, `adv:temp`, `n:prop`). Returns
/// the UPOS name as an uppercase string literal:
///
/// ```text
/// clan_to_ud_upos("n")       == Some("NOUN")
/// clan_to_ud_upos("n:prop")  == Some("PROPN")
/// clan_to_ud_upos("pro:per") == Some("PRON")
/// clan_to_ud_upos("comp")    == Some("SCONJ")
/// clan_to_ud_upos("zzzzzz")  == None
/// ```
///
/// Returns `None` for unknown tags. Callers should NOT substitute a
/// default POS when the mapping fails — treat an unmapped tag as
/// "no hint".
pub fn clan_to_ud_upos(clan_tag: &str) -> Option<&'static str> {
    // Special-case refinements that cross UPOS boundaries before
    // falling back to the coarse category.
    if clan_tag == "n:prop" {
        return Some("PROPN");
    }

    // Split on colon to get the coarse CLAN category. UD UPOS does
    // not encode refinement (e.g., `pro:per` and `pro:dem` are both
    // `PRON` in UPOS; the subtype would live in a feature like
    // `PronType=Dem`).
    let coarse = clan_tag.split(':').next()?;
    match coarse {
        // Open-class content words.
        "n" => Some("NOUN"),
        "v" => Some("VERB"),
        "adj" => Some("ADJ"),
        "adv" => Some("ADV"),
        // Closed-class function words.
        "pro" => Some("PRON"),
        "det" => Some("DET"),
        "prep" | "post" => Some("ADP"),
        "conj" => Some("CCONJ"),
        "comp" => Some("SCONJ"),
        "part" => Some("PART"),
        "mod" | "aux" => Some("AUX"),
        "qn" => Some("DET"),
        "num" => Some("NUM"),
        // Interjections / communicators / fillers.
        "co" | "int" | "intj" => Some("INTJ"),
        // Symbols / punctuation.
        "sym" => Some("SYM"),
        "punct" | "cm" | "end" | "beg" => Some("PUNCT"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_class() {
        assert_eq!(clan_to_ud_upos("n"), Some("NOUN"));
        assert_eq!(clan_to_ud_upos("v"), Some("VERB"));
        assert_eq!(clan_to_ud_upos("adj"), Some("ADJ"));
        assert_eq!(clan_to_ud_upos("adv"), Some("ADV"));
    }

    #[test]
    fn proper_noun_refinement_crosses_upos() {
        assert_eq!(clan_to_ud_upos("n:prop"), Some("PROPN"));
        // Other n refinements stay NOUN.
        assert_eq!(clan_to_ud_upos("n:gerund"), Some("NOUN"));
        assert_eq!(clan_to_ud_upos("n:deverbal"), Some("NOUN"));
    }

    #[test]
    fn pronoun_subtypes_all_map_to_pron() {
        for tag in ["pro", "pro:per", "pro:dem", "pro:int", "pro:sub", "pro:rel"] {
            assert_eq!(clan_to_ud_upos(tag), Some("PRON"), "tag = {tag}");
        }
    }

    #[test]
    fn determiner_subtypes_all_map_to_det() {
        for tag in ["det", "det:dem", "det:poss", "det:art"] {
            assert_eq!(clan_to_ud_upos(tag), Some("DET"), "tag = {tag}");
        }
    }

    #[test]
    fn adposition_covers_prep_and_post() {
        assert_eq!(clan_to_ud_upos("prep"), Some("ADP"));
        assert_eq!(clan_to_ud_upos("post"), Some("ADP"));
    }

    #[test]
    fn conjunction_coarse_vs_complementizer() {
        // CLAN `conj` is coordinating by default.
        assert_eq!(clan_to_ud_upos("conj"), Some("CCONJ"));
        // Subordinating conjunctions get a distinct CLAN tag.
        assert_eq!(clan_to_ud_upos("comp"), Some("SCONJ"));
    }

    #[test]
    fn modal_and_aux_both_aux() {
        assert_eq!(clan_to_ud_upos("mod"), Some("AUX"));
        assert_eq!(clan_to_ud_upos("aux"), Some("AUX"));
    }

    #[test]
    fn quantifier_maps_to_det() {
        // CLAN `qn` ("quantifier") maps to UD DET (UD has no separate
        // quantifier UPOS; quantifiers are DET with features).
        assert_eq!(clan_to_ud_upos("qn"), Some("DET"));
    }

    #[test]
    fn interjection_forms_all_map_to_intj() {
        for tag in ["co", "int", "intj"] {
            assert_eq!(clan_to_ud_upos(tag), Some("INTJ"), "tag = {tag}");
        }
    }

    #[test]
    fn punctuation_forms_all_map_to_punct() {
        for tag in ["punct", "cm", "end", "beg"] {
            assert_eq!(clan_to_ud_upos(tag), Some("PUNCT"), "tag = {tag}");
        }
    }

    #[test]
    fn refinements_are_stripped_when_not_upos_crossing() {
        // adv:temp is still ADV, not some different UPOS.
        assert_eq!(clan_to_ud_upos("adv:temp"), Some("ADV"));
        // adj:att is still ADJ.
        assert_eq!(clan_to_ud_upos("adj:att"), Some("ADJ"));
    }

    #[test]
    fn unknown_tag_returns_none() {
        assert_eq!(clan_to_ud_upos("zzzunknown"), None);
        assert_eq!(clan_to_ud_upos(""), None);
        assert_eq!(clan_to_ud_upos("notacat"), None);
    }

    #[test]
    fn empty_refinement_strip_does_not_panic() {
        // "pro:" should still map via coarse head "pro".
        assert_eq!(clan_to_ud_upos("pro:"), Some("PRON"));
    }
}
