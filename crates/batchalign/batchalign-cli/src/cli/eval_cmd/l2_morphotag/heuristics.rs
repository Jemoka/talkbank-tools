//! Rule-based suspicious-output detectors.
//!
//! Each detector inspects one [`AtSAnalysis`] and returns zero or more
//! [`HeuristicFlag`] variants. Detectors err on the side of recall —
//! callers are expected to spot-check flagged records for precision.
//!
//! Closed-class function-word lists are intentionally conservative: we
//! would rather miss a real error than flag a legitimate `PROPN` or
//! `NOUN` as suspicious. The lists here match the Python analyzer's so
//! that CSV columns are numerically comparable across the two tools.

use super::analysis::pos_as_lowercase;
use super::types::{AtSAnalysis, AtSStatus, HeuristicFlag};

// ---------------------------------------------------------------------------
// Closed-class function-word lists per effective language.
//
// These are lowercased forms. The heuristic compares against the spliced
// word's lowercased surface form — casing is ignored.
// ---------------------------------------------------------------------------

/// English closed-class forms.
const FUNCTION_WORDS_ENG: &[&str] = &[
    "a", "an", "the", "of", "to", "in", "on", "at", "by", "for", "with", "from", "about", "and",
    "or", "but", "if", "so", "this", "that", "these", "those", "my", "your", "his", "her", "its",
    "our", "their", "i", "you", "he", "she", "it", "we", "they", "me", "him", "us", "them", "who",
    "what", "where", "when", "why", "how", "not", "no", "yes",
];

/// Spanish closed-class forms.
const FUNCTION_WORDS_SPA: &[&str] = &[
    "el", "la", "los", "las", "un", "una", "unos", "unas", "de", "a", "en", "por", "para", "con",
    "sin", "y", "o", "pero", "si", "este", "ese", "aquel", "mi", "tu", "su", "yo", "tú", "él",
    "ella", "nosotros", "vosotros", "ellos", "que", "qué", "como", "cómo", "cuando", "cuándo",
    "no", "sí",
];

/// German closed-class forms.
const FUNCTION_WORDS_DEU: &[&str] = &[
    "der", "die", "das", "den", "dem", "des", "ein", "eine", "einen", "einem", "einer", "eines",
    "und", "oder", "aber", "wenn", "ich", "du", "er", "sie", "es", "wir", "ihr", "mein", "dein",
    "sein", "unser", "euer", "in", "auf", "an", "von", "mit", "zu", "bei", "aus", "nach", "vor",
    "über", "nicht", "ja", "nein",
];

/// French closed-class forms.
const FUNCTION_WORDS_FRA: &[&str] = &[
    "le", "la", "les", "un", "une", "des", "de", "du", "à", "au", "aux", "en", "sur", "avec",
    "sans", "et", "ou", "mais", "si", "ce", "cet", "cette", "ces", "je", "tu", "il", "elle",
    "nous", "vous", "ils", "elles", "mon", "ton", "son", "notre", "votre", "leur", "ne", "pas",
    "oui", "non",
];

/// Dutch closed-class forms.
const FUNCTION_WORDS_NLD: &[&str] = &[
    "de", "het", "een", "en", "of", "maar", "als", "dat", "dit", "die", "deze", "ik", "jij", "hij",
    "zij", "wij", "jullie", "mijn", "jouw", "zijn", "haar", "onze", "in", "op", "aan", "van",
    "met", "voor", "niet", "ja", "nee",
];

/// Resolve the closed-class list for an effective language code.
/// Returns an empty slice for languages we haven't curated — those
/// skip the `PropnForFunctionWord` heuristic entirely (recall over
/// precision).
fn function_words_for(lang_iso3: &str) -> &'static [&'static str] {
    match lang_iso3 {
        "eng" => FUNCTION_WORDS_ENG,
        "spa" => FUNCTION_WORDS_SPA,
        "deu" => FUNCTION_WORDS_DEU,
        "fra" => FUNCTION_WORDS_FRA,
        "nld" => FUNCTION_WORDS_NLD,
        _ => &[],
    }
}

// ---------------------------------------------------------------------------
// Feature markers for the POS/feature-mismatch heuristic.
//
// Rust/CHAT conventional feature vocabulary. The heuristic matches a
// feature by substring so that both flat (`-Plur`) and keyed
// (`-Number=Plur`) representations are covered.
// ---------------------------------------------------------------------------

/// Features that only make sense on VERB.
const VERB_FEATURE_MARKERS: &[&str] = &["Fin", "Imp", "Sub", "Ind", "VerbForm=", "Mood=", "Tense="];

/// Features that only make sense on NOUN / PROPN.
const NOUN_FEATURE_MARKERS: &[&str] = &["Plur", "Sing", "Gen", "Dat", "Acc", "Case=", "Number="];

// ---------------------------------------------------------------------------
// Main entry point
// ---------------------------------------------------------------------------

/// Apply every heuristic to an analysis, returning the ordered list of
/// fired flags.
///
/// `L2Xxx` and `MissingMor` are derived directly from the splice status —
/// they short-circuit other heuristics because the POS / features fields
/// are unreliable (or absent) in those cases.
pub fn flags_for(analysis: &AtSAnalysis) -> Vec<HeuristicFlag> {
    match analysis.status {
        AtSStatus::L2Xxx => return vec![HeuristicFlag::L2Xxx],
        AtSStatus::MissingMor => return vec![HeuristicFlag::MissingMor],
        AtSStatus::Spliced => {}
    }

    let mut flags = Vec::new();

    let pos_lc = analysis
        .pos
        .as_ref()
        .map(pos_as_lowercase)
        .unwrap_or_default();
    let surface_lc = analysis.occurrence.surface.as_str().to_lowercase();
    let lang_lc = analysis.occurrence.effective_lang.as_str().to_lowercase();
    let features_str = analysis
        .features
        .as_ref()
        .map(|f| f.as_str().to_string())
        .unwrap_or_default();

    // PropnForFunctionWord — PROPN POS assigned to a known closed-class
    // word in the effective language.
    if pos_lc == "propn" {
        let list = function_words_for(&lang_lc);
        if list.iter().any(|w| *w == surface_lc) {
            flags.push(HeuristicFlag::PropnForFunctionWord);
        }
    }

    // FeaturePosMismatch — nominal POS carrying verb-only features OR
    // verb POS carrying nominal-only features. Substring match across
    // the dash-joined feature list.
    if matches!(pos_lc.as_str(), "noun" | "propn") {
        if VERB_FEATURE_MARKERS
            .iter()
            .any(|m| features_str.contains(m))
        {
            flags.push(HeuristicFlag::FeaturePosMismatch);
        }
    } else if pos_lc == "verb"
        && NOUN_FEATURE_MARKERS
            .iter()
            .any(|m| features_str.contains(m))
    {
        flags.push(HeuristicFlag::FeaturePosMismatch);
    }

    flags
}
