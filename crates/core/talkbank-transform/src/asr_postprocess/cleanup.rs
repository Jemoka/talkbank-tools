//! Post-ASR cleanup: disfluency marking and n-gram retrace detection.
//!
//! Ports batchalign2's `DisfluencyReplacementEngine` and `NgramRetraceEngine`
//! from `batchalign/pipelines/cleanup/`. These ran as pipeline stages after ASR
//! in BA2 and are now integrated into the Rust ASR post-processing pipeline.
//!
//! **Disfluency replacement:** Marks filled pauses ("um" → "&-um") and applies
//! orthographic replacements ("'cause" → "because") from per-language wordlists.
//!
//! **N-gram retrace detection:** Detects repeated n-grams within each utterance
//! and wraps them in CHAT retrace notation (`<word word> [/] word word`).

use super::{AsrNormalizedText, AsrWord, ENDING_PUNCT, MOR_PUNCT, Utterance, WordKind};
use std::collections::HashMap;
use std::sync::LazyLock;

// ---------------------------------------------------------------------------
// Disfluency wordlists
// ---------------------------------------------------------------------------

/// A word replacement entry: original → main_line_form.
///
/// For filled pauses: original="um", main_line="&-um".
/// For replacements: original="'cause", main_line="(be)cause".
#[derive(Debug, Clone)]
struct Replacement {
    /// What appears on the main line in CHAT (e.g. "&-um", "(be)cause").
    main_line: &'static str,
}

/// Filled pauses for English. From BA2's `support/filled_pauses.eng`.
///
/// Format: (original, main_line).
/// In CHAT, filled pauses appear as `&-{text}` on the main line.
const FILLED_PAUSES_ENG: &[(&str, &str)] = &[("um", "&-um"), ("ur", "&-ur"), ("uh", "&-uh")];

/// Orthographic replacements for English. From BA2's `support/replacements.eng`.
///
/// These normalize informal spellings to standard CHAT forms.
const REPLACEMENTS_ENG: &[(&str, &str)] = &[
    ("mm-hmm", "mhm"),
    ("mm-hum", "mhm"),
    ("'em", "(th)em"),
    ("cuz", "(be)cause"),
    ("'cause", "(be)cause"),
];

/// Compiled filled-pause lookup (language → word → replacement).
static FILLED_PAUSE_MAP: LazyLock<HashMap<&'static str, HashMap<&'static str, Replacement>>> =
    LazyLock::new(|| {
        let mut map = HashMap::new();
        let mut eng = HashMap::new();
        for &(orig, main) in FILLED_PAUSES_ENG {
            eng.insert(orig, Replacement { main_line: main });
        }
        map.insert("eng", eng);
        map
    });

/// Compiled replacement lookup (language → word → replacement).
static REPLACEMENT_MAP: LazyLock<HashMap<&'static str, HashMap<&'static str, Replacement>>> =
    LazyLock::new(|| {
        let mut map = HashMap::new();
        let mut eng = HashMap::new();
        for &(orig, main) in REPLACEMENTS_ENG {
            eng.insert(orig, Replacement { main_line: main });
        }
        map.insert("eng", eng);
        map
    });

// ---------------------------------------------------------------------------
// Stage 2c: boundary quote-mark strip
// ---------------------------------------------------------------------------

/// Quote-mark code points that ASR engines leak into word tokens at
/// boundaries when they transcribe quoted speech verbatim.
///
/// The CHAT main-tier grammar rejects raw `"`/`'` inside words; the
/// canonical CHAT representation of quoted speech uses prosodic markers
/// (`+"/.`, `+".`) at the utterance level, not character-level quotes
/// on individual word tokens. Stripping at boundaries silently restores
/// CHAT-legality without inventing an annotation. Embedded quotes
/// inside a word survive the strip and fall through to the gate's
/// structural-only fallback path so reviewers see them via the
/// downstream validator + CHECK.
///
/// ASCII apostrophe `'` is intentionally NOT in this set: it is a
/// legitimate intra-word character in English contractions (`don't`,
/// `it's`) and is handled by the disfluency / replacement maps
/// downstream.
///
/// Note on `TAG_MARKER` (U+201E): also a CHAT structural separator
/// (`Separator::Tag`). Stage 2b `strip_separator_words` runs before
/// Stage 2c and removes legitimate standalone tag markers; only an
/// embedded `„` glued to word content reaches this predicate, where
/// stripping it from a boundary is correct (ASR producers don't emit
/// tag markers structurally).
fn is_boundary_quote(c: char) -> bool {
    use talkbank_model::chars::{
        LEFT_DOUBLE_QUOTE, LEFT_GUILLEMET, LEFT_SINGLE_QUOTE, RIGHT_DOUBLE_QUOTE, RIGHT_GUILLEMET,
        RIGHT_SINGLE_QUOTE, TAG_MARKER,
    };
    matches!(
        c,
        '"' // ASCII double quote — no Unicode-name constant; named in place
        | LEFT_DOUBLE_QUOTE
        | RIGHT_DOUBLE_QUOTE
        | TAG_MARKER
        | LEFT_SINGLE_QUOTE
        | RIGHT_SINGLE_QUOTE
        | LEFT_GUILLEMET
        | RIGHT_GUILLEMET
    )
}

/// Strip stray quote marks from the boundaries of each ASR word.
///
/// ASR engines occasionally emit tokens like `"My` (leading quote stuck
/// to the word) when transcribing quoted speech. Such tokens are
/// structurally illegal as CHAT words: tree-sitter rejects the `"`,
/// surfacing as E330 ("Expected source_file root, got 'ERROR'") at the
/// `transcript_from_asr_utterances` validation gate.
///
/// This pass runs in `prepare_words_pre_expansion` (Stage 2c, after
/// `strip_separator_words`). Embedded quotes mid-word (e.g. `she"s`)
/// are deliberately NOT touched — those are pathological and fall
/// through to the gate's structural-only fallback path so reviewers
/// see them via the downstream validator + CHECK.
///
/// Language-agnostic: every CHAT-supported language uses the same
/// boundary-quote convention.
pub fn strip_boundary_quotes(words: Vec<AsrWord>) -> Vec<AsrWord> {
    super::trim_word_boundaries(words, is_boundary_quote)
}

// ---------------------------------------------------------------------------
// Disfluency replacement
// ---------------------------------------------------------------------------

/// Apply filled-pause marking and orthographic replacements to utterances.
///
/// Matches BA2's `DisfluencyReplacementEngine.process()` which runs
/// `_mark_utterance(ut, "filled_pauses", TokenType.FP, lang)` then
/// `_mark_utterance(ut, "replacements", TokenType.REGULAR, lang)`.
///
/// For filled pauses, the word text is replaced with the `&-` prefixed form
/// (e.g. "um" → "&-um"). For replacements, the word text is replaced with
/// the main-line form (e.g. "'cause" → "(be)cause").
pub fn apply_disfluency_replacements(utterances: &mut [Utterance], lang: &str) {
    let fp_map = FILLED_PAUSE_MAP.get(lang);
    let repl_map = REPLACEMENT_MAP.get(lang);

    // No wordlists for this language — nothing to do.
    if fp_map.is_none() && repl_map.is_none() {
        return;
    }

    for utt in utterances.iter_mut() {
        for word in utt.words.iter_mut() {
            let lower = word.text.to_lowercase();

            // Filled pauses first (higher priority, matches BA2 order).
            if let Some(fp) = fp_map.and_then(|m| m.get(lower.as_str())) {
                word.text = AsrNormalizedText::new(fp.main_line);
                continue;
            }

            // Then orthographic replacements.
            if let Some(repl) = repl_map.and_then(|m| m.get(lower.as_str())) {
                word.text = AsrNormalizedText::new(repl.main_line);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// N-gram retrace detection
// ---------------------------------------------------------------------------

/// Detect repeated n-grams within each utterance and mark them as retraces.
///
/// Matches BA2's `NgramRetraceEngine.process()`. The algorithm scans for
/// repeated n-grams of increasing length (1..len for most languages, 2..len
/// for Chinese/Cantonese). When a repeated n-gram is found, all occurrences
/// except the last are marked with `WordKind::Retrace` and have punctuation
/// stripped from their text.
///
/// Example: words `["I", "am", "I", "am", "going", "."]`
///   → words `["I"(Retrace), "am"(Retrace), "I", "am", "going", "."]`
///
/// The `build_chat` module reads `WordKind::Retrace` to construct proper
/// `<...> [/]` bracketed annotation groups in the CHAT AST, rather than
/// encoding notation into the word text (which would be string hacking).
///
/// **Fillers are excluded from retrace marking.** Matches BA2's gate
/// `if j.type != TokenType.FP` in `NgramRetraceEngine.process()`
/// (`batchalign/pipelines/cleanup/retrace.py:44`). Fillers still PARTICIPATE
/// in n-gram matching (so `&-um I &-um I went` still detects the bigram
/// repeat and marks the first `I`), but filler tokens themselves are
/// never re-typed to `Retrace`. A bare `&-um &-um` therefore produces no
/// retrace mark — the repetition is filler behavior, not a false start.
/// User-facing contract is in `book/src/reference/retrace-detection.md`.
///
/// CANTONESE-SPECIFIC ADJUSTMENT: For Cantonese (yue) and Standard Chinese (zho),
/// the minimum n-gram length is 2 characters instead of 1. This is because
/// single-character false starts are extremely common in Cantonese speech
/// and would flood the output with false-positive retrace marks.
pub fn apply_retrace_detection(utterances: &mut [Utterance], lang: &str) {
    let min_ngram = if lang == "yue" || lang == "zho" { 2 } else { 1 };

    for utt in utterances.iter_mut() {
        let content_len = content_word_count(&utt.words);
        if content_len < 2 {
            continue;
        }

        // N-gram comparison is case-insensitive so a sentence-initial "I"
        // pairs with a mid-sentence "I" (and, defensively, a lowercased "i"
        // from an inconsistent ASR). Word texts are never rewritten — CHAT
        // output keeps the provider's casing.
        let content_indices: Vec<usize> = utt
            .words
            .iter()
            .enumerate()
            .filter(|(_, w)| !is_punct_or_terminator(w.text.as_str()))
            .map(|(idx, _)| idx)
            .collect();

        let mut is_retrace = vec![false; content_indices.len()];

        for n in min_ngram..content_indices.len() {
            let mut begin = 0;
            while begin + n < content_indices.len() {
                let mut root = begin;
                while root + 2 * n <= content_indices.len() {
                    let next_matches = (0..n).all(|i| {
                        utt.words[content_indices[begin + i]]
                            .text
                            .as_str()
                            .eq_ignore_ascii_case(
                                utt.words[content_indices[root + n + i]].text.as_str(),
                            )
                    });
                    if next_matches {
                        // BA2 parity gate (retrace.py:44): fillers participate
                        // in the n-gram match above, but are never re-typed to
                        // Retrace. Only non-filler tokens in the matched slice
                        // get marked.
                        for i in 0..n {
                            let orig_idx = content_indices[begin + i];
                            if !is_filler(utt.words[orig_idx].text.as_str()) {
                                is_retrace[begin + i] = true;
                            }
                        }
                        root += n;
                    } else {
                        break;
                    }
                }
                begin += 1;
            }
        }

        // Apply: set kind=Retrace and strip punctuation on marked words.
        for (content_pos, &orig_idx) in content_indices.iter().enumerate() {
            if is_retrace[content_pos] {
                utt.words[orig_idx].kind = WordKind::Retrace;
                utt.words[orig_idx].text =
                    AsrNormalizedText::new(strip_punct(utt.words[orig_idx].text.as_str()));
            }
        }
    }
}

/// Count non-punctuation words in an utterance.
fn content_word_count(words: &[AsrWord]) -> usize {
    words
        .iter()
        .filter(|w| !is_punct_or_terminator(w.text.as_str()))
        .count()
}

/// Check if a word is punctuation or a sentence terminator.
fn is_punct_or_terminator(text: &str) -> bool {
    ENDING_PUNCT.contains(&text) || MOR_PUNCT.contains(&text)
}

/// Check if a word is a CHAT filler / filled pause.
///
/// After `apply_disfluency_replacements` runs (stage 7 of the pipeline,
/// before retrace detection at stage 8), filled pauses have their text
/// rewritten to the `&-` prefixed form (`um` → `&-um`). The prefix is
/// the stable marker we use for the BA2 `!= TokenType.FP` gate, because
/// `WordKind` does not carry a `Filler` variant and we don't want to
/// add one just for this check.
fn is_filler(text: &str) -> bool {
    text.starts_with("&-")
}

/// Strip CHAT punctuation from a word (for retrace content).
fn strip_punct(text: &str) -> String {
    let mut result = text.to_string();
    for &p in ENDING_PUNCT.iter().chain(MOR_PUNCT.iter()) {
        result = result.replace(p, "");
    }
    result.trim().to_string()
}

// ---------------------------------------------------------------------------
// Transcribe-pipeline corrections (2026-04-23)
// ---------------------------------------------------------------------------
//
// Three English orthographic normalizations approved for shipping
// after the 2026-04-23 Stanza decision-probe adjudication:
//
// * **I-cap** — bare `i` → `I`, including contractions
//   (`i'll` / `i'm` / `i've` / `i'd`).
// * **Title-period strip** — `Dr.` → `Dr`, `Mr.` / `Mrs.` / `Prof.`,
//   place abbreviations (`St.`, `Mt.`, `Ave.`), time (`a.m.`,
//   `p.m.`), initialisms (`U.S.`, `J.F.K.`), degrees (`Ph.D.`,
//   `M.D.`), technical abbreviations (`etc.`, `e.g.`, `i.e.`).
// * **Utterance-initial cap** — first word of each utterance gets
//   its initial letter uppercased.
//
// Each rule is silent-mutation (no `[: replacement]` annotation) per
// the 2026-04-23 provenance-policy resolution. Each rule cites its
// probe-verdict lock in `_decision_cases/english.py` — that's the
// empirical evidence the rule is Stanza-neutral.
//
// English-only. All three rules early-return for non-English input.

use std::collections::HashSet;

/// Surface-form allowlist of English title/abbreviation tokens whose
/// trailing period(s) should be stripped. Sourced from the locked
/// probe cases:
/// * TITLE_PERIOD — `Dr.`, `Mr.`, `Mrs.`, `Prof.`
/// * PLACE_PERIOD — `St.`, `Mt.`, `Ave.`
/// * TIME_PERIOD — `a.m.`, `p.m.`
/// * INITIALISM_PERIOD — `U.S.`, `J.F.K.`
/// * DEGREE_PERIOD — `Ph.D.`, `M.D.`
/// * TECHNICAL_ABBREV — `etc.`, `e.g.`, `i.e.`
///
/// The closed-set approach is deliberate. A regex or suffix-based
/// rule would over-fire on decimals (`3.14`) and utterance-final
/// periods — both flagged as POST_STRICTLY_WORSE in the DECIMAL
/// and SENTENCE probes. Closed surfaces keep the rule safe.
///
/// Matching is **case-insensitive** on the whole surface so ASR
/// variations like `dr.` / `Dr.` / `DR.` all collapse to the
/// period-stripped form with the original casing preserved.
static EN_TITLE_PERIOD_SURFACES: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        "dr.", "mr.", "mrs.", "prof.", "st.", "mt.", "ave.", "a.m.", "p.m.", "u.s.", "j.f.k.",
        "ph.d.", "m.d.", "etc.", "e.g.", "i.e.",
    ]
    .into_iter()
    .collect()
});

/// English I-cap surface-form rewrite table. Lower-case key, upper-
/// case replacement. Handles bare pronoun `i` plus the four most
/// frequent I-contractions.
///
/// The locked probe cases for these (ENGLISH_PRONOUN_I and
/// I_CONTRACTION in `_decision_cases/english.py`) all show
/// POST_NEUTRAL — Stanza's POS tagging is invariant under case for
/// these surfaces, so the rewrite is orthographic policy, not a
/// morphotag fix.
static EN_I_CAP_REWRITES: LazyLock<&'static [(&'static str, &'static str)]> = LazyLock::new(|| {
    &[
        ("i", "I"),
        ("i'll", "I'll"),
        ("i'm", "I'm"),
        ("i've", "I've"),
        ("i'd", "I'd"),
    ]
});

/// Strip trailing period(s) from allowlisted English title /
/// abbreviation surfaces **on raw ASR elements**, before stage 2
/// word extraction.
///
/// This hook must fire earliest in the pipeline because stage 3
/// `split_multiword_tokens` uses `.` as a split separator via
/// `normalized_split_separator`. If `Dr.` reaches stage 3 intact,
/// it becomes `Dr` + `.` (two tokens), and the subsequent
/// utterance retokenizer (stage 6) splits on the trailing `.` —
/// fragmenting the utterance mid-sentence.
///
/// Stripping the period BEFORE stage 2 keeps `Dr` as a single
/// element, which flows cleanly through every subsequent stage.
///
/// English-gated.
pub fn strip_english_title_periods_on_elements(
    mut elements: Vec<super::AsrElement>,
    lang: &str,
) -> Vec<super::AsrElement> {
    if lang != "eng" {
        return elements;
    }
    for element in elements.iter_mut() {
        let lower = element.value.as_str().to_lowercase();
        if EN_TITLE_PERIOD_SURFACES.contains(lower.as_str()) {
            let stripped: String = element
                .value
                .as_str()
                .chars()
                .filter(|c| *c != '.')
                .collect();
            element.value = super::AsrRawText::new(stripped);
        }
    }
    elements
}

/// Apply the two **pre-retokenize** English transcribe rules to a
/// flat word list: I-cap and title-period strip.
///
/// These MUST run before the utterance retokenizer (stage 6),
/// because retokenize splits on trailing `.` characters — if
/// `Dr.` enters retokenize intact, the utterance gets sliced
/// between `Dr` and the following word. Stripping the period
/// here keeps `Dr` as a single token and preserves the
/// utterance boundary.
///
/// English-only. Returns immediately for any other language.
pub fn apply_english_transcribe_rules_pre_retokenize(words: &mut [AsrWord], lang: &str) {
    if lang != "eng" {
        return;
    }
    apply_i_capitalization_to_words(words);
    apply_title_period_strip_to_words(words);
}

/// Apply the **post-retokenize** English transcribe rule:
/// utterance-initial cap. This runs after utterances are formed
/// so it can inspect the first word of each utterance.
///
/// English-only.
pub fn apply_english_transcribe_rules_post_retokenize(utterances: &mut [Utterance], lang: &str) {
    if lang != "eng" {
        return;
    }
    apply_utterance_initial_capitalization(utterances);
}

/// Rewrite bare English pronoun `i` (and contractions) to `I` /
/// `I'll` / `I'm` / `I've` / `I'd`. Idempotent.
///
/// Probe-verdict locks: `_decision_cases/english.py::_PRONOUN_I_CASES`
/// (POST_NEUTRAL) and `_I_CONTRACTION_CASES` (POST_NEUTRAL).
fn apply_i_capitalization_to_words(words: &mut [AsrWord]) {
    for word in words.iter_mut() {
        let lower = word.text.to_lowercase();
        for &(src, dst) in EN_I_CAP_REWRITES.iter() {
            if lower == src {
                word.text = AsrNormalizedText::new(dst);
                break;
            }
        }
    }
}

/// Strip trailing period(s) from known English title/abbreviation
/// surfaces. Preserves the original casing of the non-period
/// characters.
///
/// The implementation: lower-case the surface and test membership
/// in [`EN_TITLE_PERIOD_SURFACES`]. On match, emit the original
/// surface with ALL `.` characters removed.
///
/// Probe-verdict locks: `_TITLE_CASES`, `_PLACE_CASES`,
/// `_TIME_CASES`, `_INITIALISM_CASES`, `_DEGREE_CASES`,
/// `_TECHNICAL_CASES` (all POST_NEUTRAL). The closed-set guard
/// ensures DECIMAL_CONTROL cases (`3.14`) and SENTENCE_PERIOD
/// cases (utterance-final `.`) never fire; those are locked
/// POST_STRICTLY_WORSE in the probe matrix.
fn apply_title_period_strip_to_words(words: &mut [AsrWord]) {
    for word in words.iter_mut() {
        let lower = word.text.to_lowercase();
        if EN_TITLE_PERIOD_SURFACES.contains(lower.as_str()) {
            let stripped: String = word.text.as_str().chars().filter(|c| *c != '.').collect();
            word.text = AsrNormalizedText::new(stripped);
        }
    }
}

/// Capitalize the first letter of the first "real" word of each
/// utterance. Skips CHAT markers (`xxx`, `yyy`, `www`), fragments
/// (`&+...`), nonwords (`&~...`), and tokens that start with
/// non-letter characters.
///
/// Probe-verdict lock: `_UTTERANCE_INITIAL_CASES` (POST_NEUTRAL).
///
/// The "first real word" walk mirrors what a CHAT reader would do
/// when capitalizing a sentence: skip any leading filler / markers
/// that aren't content words. The rule is idempotent — if the
/// first real word already starts with an uppercase letter
/// (because of I-cap, or because the ASR emitted it that way),
/// the rewrite is a no-op.
fn apply_utterance_initial_capitalization(utterances: &mut [Utterance]) {
    for utt in utterances.iter_mut() {
        for word in utt.words.iter_mut() {
            // Skip retrace words: the retrace detector has marked
            // them as repeated precursors to the "real" word at
            // the end of the retrace group. Capitalizing the first
            // retrace copy would put the uppercase on material the
            // speaker repeated but did not conclude with.
            if word.kind == WordKind::Retrace {
                continue;
            }
            let text = word.text.as_str();
            if !is_capitalizable_initial(text) {
                continue;
            }
            // is_capitalizable_initial returned true above, which
            // requires a non-empty first character.
            #[allow(clippy::unwrap_used)]
            let first = text.chars().next().unwrap();
            if first.is_uppercase() {
                break; // already capitalized, idempotent
            }
            if !first.is_lowercase() {
                break; // starts with non-letter content; don't rewrite
            }
            let mut chars = text.chars();
            // Same is_capitalizable_initial guarantee.
            #[allow(clippy::unwrap_used)]
            let head = chars.next().unwrap().to_uppercase().to_string();
            let tail: String = chars.collect();
            word.text = AsrNormalizedText::new(head + &tail);
            break; // done once we hit a real word
        }
    }
}

/// Can this surface be the target of utterance-initial cap?
///
/// Skips:
/// - Untranscribed CHAT markers (`xxx`, `yyy`, `www`).
/// - Fillers (`&-um`), fragments (`&+go`), nonwords (`&~uh`) — the
///   `&`-prefixed family is not regular capitalizable content.
/// - Empty strings.
/// - Pure punctuation / terminators.
fn is_capitalizable_initial(text: &str) -> bool {
    if text.is_empty() {
        return false;
    }
    if matches!(text, "xxx" | "yyy" | "www") {
        return false;
    }
    if text.starts_with('&') {
        return false;
    }
    if is_punct_or_terminator(text) {
        return false;
    }
    true
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asr_postprocess::SpeakerIndex;

    fn make_word(text: &str) -> AsrWord {
        AsrWord::new(text, Some(0), Some(100))
    }

    fn make_utt(speaker: usize, words: &[&str]) -> Utterance {
        Utterance {
            speaker: SpeakerIndex(speaker),
            words: words.iter().map(|w| make_word(w)).collect(),
            lang: None,
        }
    }

    // -- Disfluency tests --

    #[test]
    fn filled_pause_um_becomes_filler_marker() {
        let mut utts = vec![make_utt(0, &["I", "um", "went", "."])];
        apply_disfluency_replacements(&mut utts, "eng");
        assert_eq!(utts[0].words[1].text, "&-um");
    }

    #[test]
    fn filled_pause_uh_becomes_filler_marker() {
        let mut utts = vec![make_utt(0, &["uh", "hello", "."])];
        apply_disfluency_replacements(&mut utts, "eng");
        assert_eq!(utts[0].words[0].text, "&-uh");
    }

    #[test]
    fn replacement_cause_becomes_because() {
        let mut utts = vec![make_utt(0, &["I", "'cause", "you", "know", "."])];
        apply_disfluency_replacements(&mut utts, "eng");
        assert_eq!(utts[0].words[1].text, "(be)cause");
    }

    #[test]
    fn replacement_mmhmm_becomes_mhm() {
        let mut utts = vec![make_utt(0, &["mm-hmm", "."])];
        apply_disfluency_replacements(&mut utts, "eng");
        assert_eq!(utts[0].words[0].text, "mhm");
    }

    #[test]
    fn case_insensitive_matching() {
        let mut utts = vec![make_utt(0, &["Um", "UM", "."])];
        apply_disfluency_replacements(&mut utts, "eng");
        assert_eq!(utts[0].words[0].text, "&-um");
        assert_eq!(utts[0].words[1].text, "&-um");
    }

    #[test]
    fn no_wordlist_for_language_is_noop() {
        let mut utts = vec![make_utt(0, &["um", "hello", "."])];
        let original = utts[0].words[0].text.clone();
        apply_disfluency_replacements(&mut utts, "fra");
        assert_eq!(utts[0].words[0].text, original);
    }

    // -- Retrace tests --

    #[test]
    fn simple_word_retrace() {
        let mut utts = vec![make_utt(0, &["I", "I", "went", "."])];
        apply_retrace_detection(&mut utts, "eng");
        let texts: Vec<&str> = utts[0].words.iter().map(|w| w.text.as_str()).collect();
        assert_eq!(texts, vec!["I", "I", "went", "."]);
        assert_eq!(utts[0].words[0].kind, WordKind::Retrace);
        assert_eq!(utts[0].words[1].kind, WordKind::Regular);
        assert_eq!(utts[0].words[2].kind, WordKind::Regular);
    }

    #[test]
    fn bigram_retrace() {
        let mut utts = vec![make_utt(0, &["I", "am", "I", "am", "going", "."])];
        apply_retrace_detection(&mut utts, "eng");
        let texts: Vec<&str> = utts[0].words.iter().map(|w| w.text.as_str()).collect();
        assert_eq!(texts, vec!["I", "am", "I", "am", "going", "."]);
        assert_eq!(utts[0].words[0].kind, WordKind::Retrace);
        assert_eq!(utts[0].words[1].kind, WordKind::Retrace);
        assert_eq!(utts[0].words[2].kind, WordKind::Regular);
        assert_eq!(utts[0].words[3].kind, WordKind::Regular);
    }

    #[test]
    fn triple_retrace() {
        let mut utts = vec![make_utt(0, &["go", "go", "go", "home", "."])];
        apply_retrace_detection(&mut utts, "eng");
        let texts: Vec<&str> = utts[0].words.iter().map(|w| w.text.as_str()).collect();
        // BA2's algorithm: unigram "go" repeats, so first two marked as retrace.
        assert_eq!(texts, vec!["go", "go", "go", "home", "."]);
        assert_eq!(utts[0].words[0].kind, WordKind::Retrace);
        assert_eq!(utts[0].words[1].kind, WordKind::Retrace);
        assert_eq!(utts[0].words[2].kind, WordKind::Regular);
    }

    #[test]
    fn no_retrace_when_no_repeats() {
        let mut utts = vec![make_utt(0, &["I", "went", "home", "."])];
        apply_retrace_detection(&mut utts, "eng");
        let texts: Vec<&str> = utts[0].words.iter().map(|w| w.text.as_str()).collect();
        assert_eq!(texts, vec!["I", "went", "home", "."]);
        assert!(utts[0].words.iter().all(|w| w.kind == WordKind::Regular));
    }

    #[test]
    fn chinese_skips_unigram_retraces() {
        // In Chinese/Cantonese, BA2 starts at n=2 to avoid single-char retraces.
        let mut utts = vec![make_utt(0, &["我", "我", "去", "."])];
        apply_retrace_detection(&mut utts, "yue");
        // No retrace because min_ngram=2 for Cantonese.
        assert!(utts[0].words.iter().all(|w| w.kind == WordKind::Regular));
    }

    #[test]
    fn chinese_detects_bigram_retrace() {
        let mut utts = vec![make_utt(0, &["我", "去", "我", "去", "了", "."])];
        apply_retrace_detection(&mut utts, "zho");
        assert_eq!(utts[0].words[0].kind, WordKind::Retrace);
        assert_eq!(utts[0].words[1].kind, WordKind::Retrace);
        assert_eq!(utts[0].words[2].kind, WordKind::Regular);
        assert_eq!(utts[0].words[3].kind, WordKind::Regular);
    }

    #[test]
    fn disfluency_and_retrace_compose_ba2_parity() {
        // BA2 parity: fillers participate in n-gram matching but are never
        // re-typed to Retrace. "um um I went" → "&-um &-um I went" →
        // neither &-um gets Retrace, because both are filler tokens.
        // See `is_filler()` and the gate in `apply_retrace_detection`.
        let mut utts = vec![make_utt(0, &["um", "um", "I", "went", "."])];
        apply_disfluency_replacements(&mut utts, "eng");
        apply_retrace_detection(&mut utts, "eng");
        let texts: Vec<&str> = utts[0].words.iter().map(|w| w.text.as_str()).collect();
        assert_eq!(texts, vec!["&-um", "&-um", "I", "went", "."]);
        assert_eq!(utts[0].words[0].kind, WordKind::Regular);
        assert_eq!(utts[0].words[1].kind, WordKind::Regular);
    }

    // -- Filler retrace policy regression tests --
    //
    // These tests pin the BA2-parity rule that filler tokens (`&-um`,
    // `&-uh`, etc.) participate in n-gram matching but are never marked
    // `WordKind::Retrace`. A bare `&-uh &-uh` is filler behavior, not a
    // false start, and must not produce a `[/]` marker.
    //
    // Worked example: the sequence
    //   `&-uh and &-uh &-uh you you can see it , leaves on the bushes .`
    // must produce one retrace mark on the first `you`; every `&-uh` stays
    // `Regular`. See `book/src/reference/retrace-detection.md` for the
    // user-facing policy description.

    #[test]
    fn filler_um_repeat_is_not_retraced() {
        // "um um I went" → after disfluency, "&-um &-um I went".
        // Neither &-um should be marked Retrace.
        let mut utts = vec![make_utt(0, &["um", "um", "I", "went", "."])];
        apply_disfluency_replacements(&mut utts, "eng");
        apply_retrace_detection(&mut utts, "eng");
        assert_eq!(utts[0].words[0].text, "&-um");
        assert_eq!(utts[0].words[1].text, "&-um");
        assert_eq!(
            utts[0].words[0].kind,
            WordKind::Regular,
            "first &-um should not be Retrace (filler, not a false start)"
        );
        assert_eq!(
            utts[0].words[1].kind,
            WordKind::Regular,
            "second &-um should not be Retrace"
        );
    }

    #[test]
    fn filler_uh_in_long_utterance_is_not_retraced() {
        // Raw ASR: "uh and uh uh you you can see it , leaves on the bushes ."
        // After disfluency: "&-uh and &-uh &-uh you you can see it , leaves on the bushes ."
        // Expected: the three &-uh tokens are fillers, not retraces — only
        // the "you" repetition is a genuine retrace.
        let mut utts = vec![make_utt(
            0,
            &[
                "uh", "and", "uh", "uh", "you", "you", "can", "see", "it", ",", "leaves", "on",
                "the", "bushes", ".",
            ],
        )];
        apply_disfluency_replacements(&mut utts, "eng");
        apply_retrace_detection(&mut utts, "eng");

        for w in &utts[0].words {
            if w.text.as_str().starts_with("&-") {
                assert_eq!(
                    w.kind,
                    WordKind::Regular,
                    "filler {:?} should not be Retrace",
                    w.text.as_str()
                );
            }
        }
        // Sanity: the "you you" repetition IS a retrace (first "you" marked).
        let you_positions: Vec<usize> = utts[0]
            .words
            .iter()
            .enumerate()
            .filter(|(_, w)| w.text.as_str().eq_ignore_ascii_case("you"))
            .map(|(i, _)| i)
            .collect();
        assert_eq!(you_positions.len(), 2, "two 'you' tokens expected");
        assert_eq!(
            utts[0].words[you_positions[0]].kind,
            WordKind::Retrace,
            "first 'you' should be Retrace (genuine word repetition)"
        );
        assert_eq!(
            utts[0].words[you_positions[1]].kind,
            WordKind::Regular,
            "second 'you' stays Regular"
        );
    }

    // ── 2026-04-23 transcribe-pipeline corrections ──────────────────

    fn text_of(utt: &Utterance) -> Vec<String> {
        utt.words
            .iter()
            .map(|w| w.text.as_str().to_string())
            .collect()
    }

    /// Test helper: apply ALL three rules in the same order as the
    /// production pipeline (`finalize_words_to_chunks` for the
    /// per-word rules, then `finalize_utterances` for the
    /// utterance-initial rule). This mirrors end-to-end behavior
    /// without needing the full transcribe harness.
    fn apply_english_transcribe_rules(utts: &mut [Utterance], lang: &str) {
        for utt in utts.iter_mut() {
            apply_english_transcribe_rules_pre_retokenize(&mut utt.words, lang);
        }
        apply_english_transcribe_rules_post_retokenize(utts, lang);
    }

    #[test]
    fn i_cap_rewrites_bare_pronoun() {
        let mut utts = vec![make_utt(0, &["i", "went", "home", "."])];
        apply_english_transcribe_rules(&mut utts, "eng");
        assert_eq!(text_of(&utts[0]), vec!["I", "went", "home", "."]);
    }

    #[test]
    fn i_cap_rewrites_contractions() {
        let mut utts = vec![make_utt(0, &["i'll", "i'm", "i've", "i'd", "."])];
        apply_english_transcribe_rules(&mut utts, "eng");
        assert_eq!(text_of(&utts[0]), vec!["I'll", "I'm", "I've", "I'd", "."]);
    }

    #[test]
    fn i_cap_is_idempotent() {
        let mut utts = vec![make_utt(0, &["I", "said", "I'll", "go", "."])];
        apply_english_transcribe_rules(&mut utts, "eng");
        assert_eq!(text_of(&utts[0]), vec!["I", "said", "I'll", "go", "."]);
    }

    #[test]
    fn i_cap_skips_non_english() {
        // Italian `i` (plural masculine article) must NOT be
        // uppercased by the English rule. Language gate is the
        // sole guard.
        let mut utts = vec![make_utt(0, &["ho", "visto", "i", "bambini", "."])];
        apply_english_transcribe_rules(&mut utts, "ita");
        assert_eq!(text_of(&utts[0]), vec!["ho", "visto", "i", "bambini", "."]);
    }

    #[test]
    fn title_period_strip_covers_titles() {
        let mut utts = vec![make_utt(0, &["Dr.", "Smith", "arrived", "."])];
        apply_english_transcribe_rules(&mut utts, "eng");
        assert_eq!(text_of(&utts[0]), vec!["Dr", "Smith", "arrived", "."]);
    }

    #[test]
    fn title_period_strip_covers_all_families() {
        let mut utts = vec![make_utt(
            0,
            &[
                "Dr.", "Mr.", "Mrs.", "Prof.", // TITLE_PERIOD
                "St.", "Mt.", "Ave.", // PLACE_PERIOD
                "a.m.", "p.m.", // TIME_PERIOD
                "U.S.", "J.F.K.", // INITIALISM_PERIOD
                "Ph.D.", "M.D.", // DEGREE_PERIOD
                "etc.", "e.g.", "i.e.", // TECHNICAL_ABBREV
            ],
        )];
        apply_english_transcribe_rules(&mut utts, "eng");
        assert_eq!(
            text_of(&utts[0]),
            vec![
                "Dr", "Mr", "Mrs", "Prof", "St", "Mt", "Ave", "am", "pm", "US", "JFK", "PhD", "MD",
                "etc", "eg", "ie",
            ]
        );
    }

    #[test]
    fn title_period_strip_is_case_insensitive_on_match_case_preserving_on_write() {
        // Lowercase / uppercase variations match the allowlist but
        // preserve the original casing of non-period characters.
        // A pre-capitalized first word blocks utterance-initial cap
        // from re-capitalizing subsequent probes.
        let mut utts = vec![make_utt(0, &["They", "said", "dr.", "DR.", "Mr.", "PROF."])];
        apply_english_transcribe_rules(&mut utts, "eng");
        assert_eq!(
            text_of(&utts[0]),
            vec!["They", "said", "dr", "DR", "Mr", "PROF"]
        );
    }

    #[test]
    fn title_period_strip_does_not_fire_on_decimal() {
        // Probe: `_DECIMAL_CASES` is locked POST_STRICTLY_WORSE.
        // Closed allowlist means `3.14` never matches. The
        // pre-capitalized first word blocks the utterance-initial
        // cap rule from touching the decimal context.
        let mut utts = vec![make_utt(0, &["Pi", "is", "3.14", "."])];
        apply_english_transcribe_rules(&mut utts, "eng");
        assert_eq!(text_of(&utts[0]), vec!["Pi", "is", "3.14", "."]);
    }

    #[test]
    fn title_period_strip_does_not_fire_on_utterance_final_period() {
        // SENTENCE_PERIOD: the period at the end of an utterance is
        // a separate token, not attached to a word. The allowlist
        // does not contain a bare `.` entry, so it passes through.
        let mut utts = vec![make_utt(0, &["I", "saw", "him", "."])];
        apply_english_transcribe_rules(&mut utts, "eng");
        assert_eq!(text_of(&utts[0]), vec!["I", "saw", "him", "."]);
    }

    #[test]
    fn title_period_strip_does_not_fire_on_unrelated_word_with_period() {
        // A random word with a period that isn't in the allowlist
        // must pass through unmodified. Belt-and-suspenders for the
        // closed-set design.
        let mut utts = vec![make_utt(0, &["Hello.", "world", "."])];
        apply_english_transcribe_rules(&mut utts, "eng");
        assert_eq!(text_of(&utts[0]), vec!["Hello.", "world", "."]);
    }

    #[test]
    fn utterance_initial_cap_capitalizes_first_word() {
        let mut utts = vec![make_utt(0, &["hello", "world", "."])];
        apply_english_transcribe_rules(&mut utts, "eng");
        assert_eq!(text_of(&utts[0]), vec!["Hello", "world", "."]);
    }

    #[test]
    fn utterance_initial_cap_is_idempotent() {
        let mut utts = vec![make_utt(0, &["Hello", "world", "."])];
        apply_english_transcribe_rules(&mut utts, "eng");
        assert_eq!(text_of(&utts[0]), vec!["Hello", "world", "."]);
    }

    #[test]
    fn utterance_initial_cap_skips_chat_markers() {
        let mut utts = vec![make_utt(0, &["xxx", "said", "something", "."])];
        apply_english_transcribe_rules(&mut utts, "eng");
        // `xxx` stays lowercase; the rule walks past it to the
        // first capitalizable word.
        assert_eq!(text_of(&utts[0]), vec!["xxx", "Said", "something", "."]);
    }

    #[test]
    fn utterance_initial_cap_skips_fillers_and_fragments() {
        let mut utts = vec![make_utt(0, &["&-um", "&+go", "&~uh", "hello", "."])];
        apply_english_transcribe_rules(&mut utts, "eng");
        // The `&`-prefixed family is skipped; `hello` gets the cap.
        assert_eq!(
            text_of(&utts[0]),
            vec!["&-um", "&+go", "&~uh", "Hello", "."]
        );
    }

    #[test]
    fn utterance_initial_cap_runs_after_i_cap() {
        // Bare `i` at utterance-initial position goes through I-cap
        // first (becomes `I`), so utterance-initial cap is a no-op.
        let mut utts = vec![make_utt(0, &["i", "went", "home", "."])];
        apply_english_transcribe_rules(&mut utts, "eng");
        assert_eq!(text_of(&utts[0]), vec!["I", "went", "home", "."]);
    }

    #[test]
    fn combined_rules_fire_in_correct_order() {
        // Full cooperation: bare `i` capitalizes, `Dr.` strips,
        // first word uppercases.
        let mut utts = vec![make_utt(0, &["i", "saw", "Dr.", "Smith", "today", "."])];
        apply_english_transcribe_rules(&mut utts, "eng");
        assert_eq!(
            text_of(&utts[0]),
            vec!["I", "saw", "Dr", "Smith", "today", "."]
        );
    }

    /// Specifically verifies that period-strip, running at the
    /// `finalize_words_to_chunks` seam (BEFORE retokenize), catches
    /// `Dr.` so retokenize doesn't see a trailing period and
    /// therefore doesn't split the utterance in half.
    #[test]
    fn period_strip_prevents_retokenize_mid_utterance_split() {
        use crate::asr_postprocess::{
            AsrElement, AsrElementKind, AsrMonologue, AsrOutput, AsrRawText, AsrTimestampSecs,
            process_raw_asr,
        };
        let elements: Vec<AsrElement> = [
            ("I", 0.0, 0.1, AsrElementKind::Text),
            ("said", 0.1, 0.4, AsrElementKind::Text),
            ("Dr.", 0.4, 0.7, AsrElementKind::Text),
            ("Smith", 0.7, 1.0, AsrElementKind::Text),
            ("arrived", 1.0, 1.4, AsrElementKind::Text),
            (".", 1.4, 1.5, AsrElementKind::Punctuation),
        ]
        .iter()
        .map(|(t, s, e, k)| AsrElement {
            value: AsrRawText::new(*t),
            ts: AsrTimestampSecs(*s),
            end_ts: AsrTimestampSecs(*e),
            kind: *k,
        })
        .collect();
        let out = AsrOutput {
            monologues: vec![AsrMonologue {
                speaker: SpeakerIndex(0),
                elements,
            }],
        };
        let utts = process_raw_asr(&out, "eng");
        // One utterance, not two: period-strip caught `Dr.`.
        assert_eq!(
            utts.len(),
            1,
            "expected 1 utterance, got {}: {:?}",
            utts.len(),
            utts.iter()
                .map(|u| u
                    .words
                    .iter()
                    .map(|w| w.text.as_str().to_string())
                    .collect::<Vec<_>>())
                .collect::<Vec<_>>()
        );
        let words: Vec<&str> = utts[0].words.iter().map(|w| w.text.as_str()).collect();
        assert_eq!(words, vec!["I", "said", "Dr", "Smith", "arrived", "."]);
    }

    #[test]
    fn combined_rules_fire_per_utterance() {
        let mut utts = vec![
            make_utt(0, &["hello", "world", "."]),
            make_utt(0, &["i", "said", "Dr.", "Smith", "."]),
        ];
        apply_english_transcribe_rules(&mut utts, "eng");
        assert_eq!(text_of(&utts[0]), vec!["Hello", "world", "."]);
        assert_eq!(text_of(&utts[1]), vec!["I", "said", "Dr", "Smith", "."]);
    }
}
