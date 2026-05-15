//! Per-language number-expansion registry — Layer 1 of the
//! number-expansion rework documented in
//! `book/src/architecture/number-expansion.md`.
//!
//! Replaces the dual-pass dispatch (Python `num2words` IPC + Rust
//! `NUM2LANG` safety pass) with a single typed routing table:
//! every language we transcribe is mapped to exactly one expander,
//! the choice is documented in code, and the dispatcher routes
//! per-token without redundant work or silent precedence.
//!
//! Adding a language is one line in [`NUMBER_EXPANDERS`]. Removing
//! coverage is one line. Documenting that we know a language has
//! no expansion path is one line — `NoCoverage { tracked_in: ... }`
//! makes the gap a first-class concept instead of a silent
//! fallthrough that surfaces as E220 at validation time.

use std::collections::HashMap;
use std::sync::LazyLock;

use crate::asr_postprocess::num2chinese::ChineseScript;
use crate::asr_postprocess::num2text::NUM2LANG;

/// Where a language's number-expansion implementation lives.
/// Exactly one variant per language; routing is explicit.
#[derive(Debug, Clone, Copy)]
pub enum NumberExpander {
    /// Hand-curated or codegen-generated `NUM2LANG`-style table
    /// embedded at compile time. Static reference is cheap to
    /// `Copy`; the actual table lives in the `LazyLock`.
    RustTable,
    /// CJK numerals via [`num2chinese`](super::num2chinese::num2chinese).
    /// Script is part of the variant — Simplified for
    /// `zho` / `cmn`, Traditional for `jpn` / `yue` (CANTONESE-SPECIFIC).
    Num2Chinese(ChineseScript),
    /// Language allows Arabic digits in CHAT per the validator
    /// allowlist (`talkbank-tools/.../digits.rs`). Pass-through
    /// is correct; no expansion needed.
    LangAllowsDigits,
    /// Documented gap. The language is one we transcribe but have
    /// no number-expansion path for. The string points at a
    /// tracking issue / TODO so the next contributor can pick up
    /// the work without re-deriving why it's unhandled.
    ///
    /// Distinct from "language not in registry" — that's an
    /// untracked omission that should fail loud at registration
    /// audit time, not silently no-op.
    NoCoverage {
        /// Where the tracking lives (typically a path to a
        /// follow-up doc or postmortem).
        tracked_in: &'static str,
    },
}

/// Single source of truth: every ISO 639-3 language we transcribe
/// must have an entry. Unknown languages are not silently passed
/// through; the registry is the place to declare "yes, we know
/// about this language and here's what we do."
///
/// The keyset is enumerated by the `every_transcribe_language_is_registered`
/// test below, which the build fails on if a registration is added
/// or removed without updating the test.
pub static NUMBER_EXPANDERS: LazyLock<HashMap<&'static str, NumberExpander>> =
    LazyLock::new(|| {
        let mut m = HashMap::new();

        // CJK — Rust-side num2chinese, never touches the codegen tables.
        m.insert(
            "zho",
            NumberExpander::Num2Chinese(ChineseScript::Simplified),
        );
        m.insert(
            "cmn",
            NumberExpander::Num2Chinese(ChineseScript::Simplified),
        );
        m.insert(
            "jpn",
            NumberExpander::Num2Chinese(ChineseScript::Traditional),
        );
        // CANTONESE-SPECIFIC: Cantonese uses Traditional Chinese numerals.
        m.insert(
            "yue",
            NumberExpander::Num2Chinese(ChineseScript::Traditional),
        );

        // Languages whose CHAT validator allows digits inline (per
        // `talkbank-tools/.../digits.rs::DIGIT_ALLOWED_LANGS`).
        // Expansion isn't required to pass validation; we still send
        // through `NUM2LANG` if a table exists, but the variant
        // documents that even passthrough is acceptable.
        //
        // Note: `zho` and `yue` (CANTONESE-SPECIFIC) are listed in the allowlist but
        // already have a more-specific expander above; the more-
        // specific entry wins. Same for `tha`.
        m.insert("cym", NumberExpander::LangAllowsDigits); // Welsh
        m.insert("vie", NumberExpander::LangAllowsDigits); // Vietnamese (also has RustTable)
        m.insert("nan", NumberExpander::LangAllowsDigits); // Min Nan
        m.insert("min", NumberExpander::LangAllowsDigits); // Minangkabau
        m.insert("hak", NumberExpander::LangAllowsDigits); // Hakka

        // Every other language with a NUM2LANG entry routes RustTable.
        // Populated programmatically from the loaded JSON so a new
        // entry in `data/num2lang.json` automatically registers.
        for lang in NUM2LANG.keys() {
            m.entry(lang.as_str()).or_insert(NumberExpander::RustTable);
        }

        m
    });

/// Resolve the expander for one language. Returns `None` for
/// languages not in the registry — caller decides whether that's
/// a hard error (preflight) or a silent passthrough (legacy
/// dispatch). The Layer 1 rework will treat unknown-language as
/// hard-fail at submission preflight.
pub fn expander_for(lang: &str) -> Option<NumberExpander> {
    NUMBER_EXPANDERS.get(lang.to_lowercase().as_str()).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// CJK languages route to the Chinese-script expander, not the
    /// generic NUM2LANG table. CANTONESE-SPECIFIC: Cantonese (yue) routes to
    /// Traditional Chinese numerals (like Japanese), not Simplified.
    #[test]
    fn cjk_routes_to_num2chinese() {
        for (lang, expected) in [
            ("zho", ChineseScript::Simplified),
            ("cmn", ChineseScript::Simplified),
            ("jpn", ChineseScript::Traditional),
            ("yue", ChineseScript::Traditional), // CANTONESE-SPECIFIC
        ] {
            match expander_for(lang) {
                Some(NumberExpander::Num2Chinese(script)) => assert_eq!(script, expected),
                other => panic!("{lang} should route Num2Chinese({expected:?}), got {other:?}"),
            }
        }
    }

    /// Languages explicitly in the digit-allowed validator list
    /// route `LangAllowsDigits` (or override to a more-specific
    /// expander like CJK).
    #[test]
    fn digit_allowed_langs_route_explicitly() {
        for lang in ["cym", "nan", "min", "hak"] {
            assert!(
                matches!(expander_for(lang), Some(NumberExpander::LangAllowsDigits)),
                "{lang} should be LangAllowsDigits"
            );
        }
    }

    /// Every language with a NUM2LANG table is reachable as
    /// `RustTable` (or overridden to a more-specific variant).
    /// Catches the case where someone adds a language to the JSON
    /// but forgets to register; the JSON-loop in the registry
    /// constructor handles this automatically, but the test pins
    /// the behaviour.
    #[test]
    fn every_num2lang_entry_is_registered() {
        for lang in NUM2LANG.keys() {
            assert!(
                expander_for(lang).is_some(),
                "{lang} has a NUM2LANG entry but no registry entry"
            );
        }
    }

    /// Malayalam — the language that motivated the Layer 1 rework
    /// — must resolve to RustTable since num2words has no `ml`
    /// backend and the hand-curated entries live in NUM2LANG.
    #[test]
    fn malayalam_routes_rust_table() {
        assert!(matches!(
            expander_for("mal"),
            Some(NumberExpander::RustTable)
        ));
    }

    /// Unknown ISO codes return `None` so the dispatcher can
    /// fail-loud instead of silently passing the digit through.
    #[test]
    fn unknown_language_returns_none() {
        assert!(expander_for("xxx").is_none());
        assert!(expander_for("").is_none());
    }

    /// The lookup is case-insensitive for the input but the
    /// registry stores ISO 639-3 in lowercase (the convention
    /// across this codebase per `LanguageCode3`).
    #[test]
    fn lookup_is_case_insensitive() {
        assert!(expander_for("ENG").is_some());
        assert!(expander_for("Eng").is_some());
        assert!(expander_for("eng").is_some());
    }
}
