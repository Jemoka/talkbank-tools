use super::{
    AsrElement, AsrNormalizedText, AsrPipelineSnapshot, AsrWord, cleanup, merge_compounds, num2text,
};

/// Stages 1-3: compound merging, timed word extraction with separator strip,
/// and multi-word token splitting.
///
/// Returns words ready for number expansion. The caller is responsible for
/// expanding numbers (either via the Rust fallback tables or Python IPC)
/// before passing the words to [`super::finalize_words_to_chunks`].
pub fn prepare_words_pre_expansion(elements: &[AsrElement], lang: &str) -> Vec<AsrWord> {
    prepare_words_pre_expansion_with_snapshot(elements, lang, None)
}

/// Snapshot-aware variant of [`prepare_words_pre_expansion`].
///
/// When `snapshot` is `Some`, intermediate stage outputs
/// (`after_compound_merge`, `after_timing_extract`,
/// `after_multiword_split`) are populated for downstream trace
/// rendering. When `None`, behavior is identical to the bare variant
/// at zero capture cost.
///
/// Callers who want stage 4 (number expansion) captured must record
/// it themselves — this function returns BEFORE expansion runs.
pub fn prepare_words_pre_expansion_with_snapshot(
    elements: &[AsrElement],
    lang: &str,
    mut snapshot: Option<&mut AsrPipelineSnapshot>,
) -> Vec<AsrWord> {
    // Stage 1: compound merging
    let merged = merge_compounds(elements);
    if let Some(ref mut s) = snapshot {
        s.after_compound_merge = merged.clone();
    }

    // 2026-04-23 English title-period strip MUST fire here, before
    // `extract_timed_words` and `split_multiword_tokens` — the
    // latter splits on `.` (`normalized_split_separator`) and
    // would slice `Dr.` into `Dr` + `.` before our rule sees the
    // intact allowlisted surface. Operates on raw element text
    // (pre-tokenization). English-gated.
    let merged = cleanup::strip_english_title_periods_on_elements(merged, lang);

    // Stage 2: extract words with ms timings, filter pauses
    let mut words = extract_timed_words(&merged);
    if let Some(ref mut s) = snapshot {
        s.after_timing_extract = words.clone();
    }

    // Stage 2b: strip MOR_PUNCT separators from the boundaries of each word.
    // Case is preserved — downstream CHAT consumers need uppercase "I" and
    // proper nouns intact; disfluency and retrace matching are case-insensitive
    // internally.
    words = strip_separator_words(words);

    // Stage 2c: strip stray quote marks at word boundaries. ASR providers
    // emit tokens like `"My` when transcribing quoted speech verbatim;
    // tree-sitter rejects the literal `"` so these tokens otherwise
    // tank the whole transcribe job. Silent strip rather than a CHAT
    // annotation: pure orthographic noise, no information lost.
    words = cleanup::strip_boundary_quotes(words);

    // Stage 3: split multi-word tokens with timestamp interpolation
    let words = split_multiword_tokens(words, lang);

    // Stage 3c: re-run boundary-quote strip AFTER Stage 3. Stage 3 splits
    // on whitespace and `.`/`?`/`!`/`,` — a single ASR element like
    // `Ross." said.` (period+quote glued, internal whitespace) splits
    // into `["Ross", ".", "\"", "said", "."]`. The standalone `"` part
    // bypassed Stage 2c (which ran before the split). Stripping again
    // here also catches symmetric `"hello` / `world"` shapes that
    // Stage 3 produces from `He said "hello world." said.`-style
    // multi-token ASR values.
    let words = cleanup::strip_boundary_quotes(words);

    // Stage 3b: split percent-suffix tokens (`80%` → `80`, `percent`).
    // `%` is the CHAT dep-tier sigil and cannot reach the main tier in
    // any language; this stage guarantees that property for every
    // downstream consumer, including the Python-routed number-expansion
    // path used by `transcribe`.
    let result = split_percent_suffix_words(words, lang);
    if let Some(ref mut s) = snapshot {
        s.after_multiword_split = result.clone();
    }
    result
}

/// Extract timed words from ASR elements, converting seconds to milliseconds.
///
/// Filters out pause markers (like `<pause>`) and blank values.
pub(super) fn extract_timed_words(elements: &[AsrElement]) -> Vec<AsrWord> {
    let mut words = Vec::new();
    for elem in elements {
        let value = elem.value.as_str().trim();
        if value.is_empty() {
            continue;
        }
        // Filter pause markers like <pause>, <inaudible>, etc.
        if value.starts_with('<') && value.ends_with('>') {
            continue;
        }
        let (start_ms, end_ms) = normalized_timing_range(elem.ts.as_f64(), elem.end_ts.as_f64());
        words.push(AsrWord::new(value, start_ms, end_ms));
    }
    words
}

/// Strip MOR_PUNCT separators from the boundaries of each word token, and
/// drop words that become empty after stripping.
///
/// ENDING_PUNCT (`.` `?` `!` etc.) is **not** stripped — retokenize needs
/// them for sentence boundary detection. Only MOR_PUNCT (comma, tag `„`,
/// vocative `‡`) and RTL separators (`،` `؛`) are removed.
///
/// Case is preserved: uppercase tokens from the ASR provider (pronoun
/// "I", proper nouns like "Sarah" / "Cincinnati", sentence-initial
/// capitals) survive unchanged. Downstream components that need
/// case-insensitive comparison (disfluency lookup, retrace detection)
/// lowercase at comparison time.
fn strip_separator_words(words: Vec<AsrWord>) -> Vec<AsrWord> {
    /// MOR_PUNCT separators (comma, tag, vocative) plus the Arabic /
    /// RTL comma (U+060C) and semicolon (U+061B). Hardcoded as chars
    /// rather than string slices because `trim_matches` takes
    /// `FnMut(char) -> bool`; see [`super::MOR_PUNCT`] for the string-
    /// slice equivalent used elsewhere.
    fn is_strippable(c: char) -> bool {
        matches!(
            c,
            ',' | talkbank_model::chars::TAG_MARKER
                | talkbank_model::chars::VOCATIVE_MARKER
                | '\u{060C}' // ARABIC COMMA
                | '\u{061B}' // ARABIC SEMICOLON
        )
    }

    trim_word_boundaries(words, is_strippable)
}

/// Trim characters matching `is_strip` from the boundaries of each word, and
/// drop any word whose text becomes empty.
///
/// Shared primitive behind every boundary-trim pass in
/// `prepare_words_pre_expansion` (Stage 2b separators, Stage 2c boundary
/// quotes). The text payload is rewritten only when trimming actually
/// changed the string, keeping the common no-op path allocation-free.
pub(super) fn trim_word_boundaries(
    words: Vec<AsrWord>,
    is_strip: impl Fn(char) -> bool,
) -> Vec<AsrWord> {
    words
        .into_iter()
        .filter_map(|mut word| {
            let stripped: &str = word.text.as_str().trim_matches(&is_strip);
            if stripped.is_empty() {
                return None;
            }
            if stripped.len() != word.text.as_str().len() {
                word.text = AsrNormalizedText::new(stripped);
            }
            Some(word)
        })
        .collect()
}

/// Split tokens containing spaces into multiple words with interpolated timestamps.
///
/// Also handles hyphen-prefixed words by joining them with the previous word.
pub(super) fn split_multiword_tokens(words: Vec<AsrWord>, lang: &str) -> Vec<AsrWord> {
    let mut result: Vec<AsrWord> = Vec::new();

    for word in words {
        // Join hyphen-prefixed words with previous
        if word.text.starts_with('-') && !result.is_empty() {
            // SAFETY: `!result.is_empty()` guard above ensures last_mut() succeeds.
            #[allow(clippy::unwrap_used)]
            let prev = result.last_mut().unwrap();
            prev.text.push_str(word.text.as_str());
            prev.end_ms = word.end_ms;
            continue;
        }

        result.extend(split_chunk_word(word, lang));
    }

    result
}

fn normalized_timing_range(start_s: f64, end_s: f64) -> (Option<i64>, Option<i64>) {
    if !start_s.is_finite() || !end_s.is_finite() {
        return (None, None);
    }

    let start_ms = (start_s * 1000.0).round() as i64;
    let end_ms = (end_s * 1000.0).round() as i64;
    if end_ms <= start_ms {
        (None, None)
    } else {
        (Some(start_ms), Some(end_ms))
    }
}

fn split_chunk_word(word: AsrWord, lang: &str) -> Vec<AsrWord> {
    let mut parts: Vec<(String, bool)> = Vec::new();
    let mut current = String::new();

    let flush_current = |parts: &mut Vec<(String, bool)>, current: &mut String| {
        if !current.is_empty() {
            parts.push((std::mem::take(current), false));
        }
    };

    for ch in word.text.as_str().chars() {
        if ch.is_whitespace() {
            flush_current(&mut parts, &mut current);
            continue;
        }

        if let Some(separator) = normalized_split_separator(ch) {
            flush_current(&mut parts, &mut current);
            if let Some(text) = separator {
                parts.push((text.to_string(), true));
            }
            continue;
        }

        current.push(ch);
    }
    flush_current(&mut parts, &mut current);

    let mut expanded_parts: Vec<(String, bool)> = Vec::new();
    for (text, is_separator) in parts {
        if is_separator {
            expanded_parts.push((text, true));
            continue;
        }
        expanded_parts.extend(expand_language_part(text, lang));
    }
    let parts = expanded_parts;

    if parts.len() == 1 && !parts[0].1 && parts[0].0 == word.text.as_str() {
        return vec![word];
    }

    let total_text_chars: usize = parts
        .iter()
        .filter(|(_, is_separator)| !*is_separator)
        .map(|(text, _)| text.chars().count())
        .sum();

    let mut consumed_chars = 0usize;
    let total_span = match (word.start_ms, word.end_ms) {
        (Some(start), Some(end)) if end > start && total_text_chars > 0 => {
            Some((start, end - start))
        }
        _ => None,
    };

    parts
        .into_iter()
        .map(|(text, is_separator)| {
            if is_separator {
                return AsrWord::new(text, None, None);
            }

            let timings = total_span.map(|(start, span)| {
                let part_chars = text.chars().count();
                let part_start = start + (span * consumed_chars as i64 / total_text_chars as i64);
                consumed_chars += part_chars;
                let part_end = start + (span * consumed_chars as i64 / total_text_chars as i64);
                (Some(part_start), Some(part_end))
            });

            let (start_ms, end_ms) = timings.unwrap_or((None, None));
            AsrWord::new(text, start_ms, end_ms)
        })
        .collect()
}

fn expand_language_part(text: String, lang: &str) -> Vec<(String, bool)> {
    // CANTONESE-SPECIFIC BOUNDARY: Cantonese character tokenization.
    // This stage splits pure CJK text for Cantonese (yue) only.
    // Non-Cantonese languages and mixed text pass through unchanged.
    if lang != "yue" || !should_split_cantonese_chars(&text) {
        return vec![(text, false)];
    }

    let tokens = super::cantonese::cantonese_char_tokens(&text);
    if tokens.len() <= 1 {
        return vec![(text, false)];
    }
    tokens.into_iter().map(|token| (token, false)).collect()
}

fn should_split_cantonese_chars(text: &str) -> bool {
    let mut has_cjk = false;
    for ch in text.chars() {
        if ch.is_ascii_alphabetic() || ch.is_ascii_digit() {
            return false;
        }
        if is_cjk_ideograph(ch) {
            has_cjk = true;
        }
    }
    has_cjk
}

fn is_cjk_ideograph(ch: char) -> bool {
    matches!(
        ch as u32,
        0x3400..=0x4DBF
            | 0x4E00..=0x9FFF
            | 0xF900..=0xFAFF
            | 0x20000..=0x2A6DF
            | 0x2A700..=0x2B73F
            | 0x2B740..=0x2B81F
            | 0x2B820..=0x2CEAF
            | 0x2F800..=0x2FA1F
    )
}

fn normalized_split_separator(ch: char) -> Option<Option<&'static str>> {
    match ch {
        '.' => Some(Some(".")),
        '?' | '？' | '؟' => Some(Some("?")),
        '!' | '！' => Some(Some("!")),
        ',' | '，' | '、' | '،' => Some(Some(",")),
        '¿' | '¡' => Some(None),
        '。' => Some(Some(".")),
        _ => None,
    }
}

/// Split tokens of the form `{digits}%` into two words.
///
/// ASR providers (notably Rev.AI) sometimes emit `"80%"` as a single
/// word token. `%` is the CHAT dependent-tier sigil and cannot appear
/// as main-tier word content in *any* language, so the literal token
/// must never reach `build_chat`. This stage splits the offender into
/// two `AsrWord`s:
///
/// 1. The digit group (still a single token, timing = first portion
///    of the original span).
/// 2. The language-specific percent word (timing = remaining portion).
///
/// When the language has no mapped percent word (uncommon), the `%`
/// is stripped and the digit group is emitted alone — better than
/// producing malformed CHAT. When the language permits ASCII digits
/// in word content (yue/zho/cmn/nan/hak/min/cym/vie/tha), the digit
/// group is still emitted; the language-aware `ChatWordText`
/// validation will accept it.
///
/// Purely structural: this runs before number expansion so the digit
/// group can be expanded by the existing pipeline if the language
/// supports it.
fn split_percent_suffix_words(words: Vec<AsrWord>, lang: &str) -> Vec<AsrWord> {
    let mut result: Vec<AsrWord> = Vec::with_capacity(words.len());
    for word in words {
        let Some(digit_prefix) = word.text.as_str().strip_suffix('%') else {
            result.push(word);
            continue;
        };
        if digit_prefix.is_empty() || !digit_prefix.chars().all(|c| c.is_ascii_digit()) {
            // `%` not following a pure-digit prefix — not our case.
            result.push(word);
            continue;
        }

        let (digit_start, digit_end, percent_start, percent_end) =
            match (word.start_ms, word.end_ms) {
                (Some(start), Some(end)) if end > start => {
                    // Distribute timing proportionally by text length.
                    // The digit group already has 1-N characters; the
                    // percent suffix is the final 1 character (`%`).
                    // Roughly split the total span in that ratio so
                    // downstream FA can realign if the timing matters.
                    let total_chars = word.text.as_str().chars().count() as i64;
                    let digit_chars = digit_prefix.chars().count() as i64;
                    // Guard against div-by-zero even though total_chars
                    // is non-zero when we reach here (word.text was
                    // non-empty — the `%` alone is caught above).
                    let split = start + ((end - start) * digit_chars) / total_chars.max(1);
                    (Some(start), Some(split), Some(split), Some(end))
                }
                _ => (word.start_ms, word.end_ms, None, None),
            };

        let digit_word = AsrWord::new(digit_prefix, digit_start, digit_end);
        result.push(digit_word);

        if let Some(percent_word) = num2text::language_percent_word(lang) {
            result.push(AsrWord::new(percent_word, percent_start, percent_end));
        }
        // If no mapped percent word: we've already dropped the `%` by
        // constructing digit_word without the suffix. The main-tier
        // invariant is preserved; we just lose the "percent" semantic
        // marker until the table is extended for this language.
    }
    result
}
