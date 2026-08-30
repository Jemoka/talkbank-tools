use super::{AsrNormalizedText, AsrWord, WordKind, expand_number};

/// Expand digit strings to word form in all words.
///
/// Some expansions turn one input word into multi-word text (e.g.
/// `"100"` → `"one hundred"` via num2words, `"2001-2002"` →
/// `"two thousand one two thousand two"` via the dash-split branch).
/// Such outputs must be re-split into separate `AsrWord`s — a
/// `ChatWordText` holds one token on the main tier, and whitespace
/// inside a single word makes the fragment parser reject it as two
/// tokens glued together. Timing is distributed proportionally by
/// text length so downstream FA can realign if needed.
pub(super) fn expand_numbers_in_words(words: Vec<AsrWord>, lang: &str) -> Vec<AsrWord> {
    words
        .into_iter()
        .flat_map(|w| {
            let expanded = expand_number(w.text.as_str(), lang);
            if !expanded.contains(char::is_whitespace) {
                return vec![AsrWord {
                    text: AsrNormalizedText::new(expanded),
                    ..w
                }];
            }
            split_expanded_text_into_words(&expanded, w.start_ms, w.end_ms, w.kind)
        })
        .collect()
}

/// Replace every `AsrWord` whose text contains whitespace with a
/// sequence of single-token `AsrWord`s, preserving overall timing.
///
/// Number expansion (`expand_number`) can turn one input token into multi-word text
/// (`"100"` → `"one hundred"`, `"2001-2002"` → `"two thousand one two
/// thousand two"`). A single `AsrWord` cannot carry whitespace —
/// `ChatWordText` holds one token on the main tier, and the fragment
/// parser rejects whitespace inside a word. This pass normalises such
/// outputs after expansion, distributing timing proportionally by
/// character count.
pub fn split_words_with_whitespace(words: &mut Vec<AsrWord>) {
    if !words
        .iter()
        .any(|w| w.text.as_str().contains(char::is_whitespace))
    {
        return;
    }
    let taken = std::mem::take(words);
    words.reserve(taken.len());
    for w in taken {
        if w.text.as_str().contains(char::is_whitespace) {
            words.extend(split_expanded_text_into_words(
                w.text.as_str(),
                w.start_ms,
                w.end_ms,
                w.kind,
            ));
        } else {
            words.push(w);
        }
    }
}

/// Distribute a whitespace-separated expansion across several
/// `AsrWord`s, proportioning timing by text length.
///
/// Called only on the post-expansion path — input comes from
/// `expand_number`, which is deterministic given the original token,
/// so no need to re-run any normalization on the split parts.
fn split_expanded_text_into_words(
    expanded: &str,
    start_ms: Option<i64>,
    end_ms: Option<i64>,
    kind: WordKind,
) -> Vec<AsrWord> {
    let parts: Vec<&str> = expanded.split_whitespace().collect();
    if parts.is_empty() {
        return Vec::new();
    }

    let total_chars: i64 = parts.iter().map(|p| p.chars().count() as i64).sum();
    let span = match (start_ms, end_ms) {
        (Some(s), Some(e)) if e > s && total_chars > 0 => Some((s, e - s)),
        _ => None,
    };

    let mut consumed: i64 = 0;
    parts
        .into_iter()
        .map(|part| {
            let part_chars = part.chars().count() as i64;
            let (ps, pe) = match span {
                Some((s, dur)) => {
                    let start = s + (dur * consumed) / total_chars.max(1);
                    consumed += part_chars;
                    let end = s + (dur * consumed) / total_chars.max(1);
                    (Some(start), Some(end))
                }
                None => (None, None),
            };
            AsrWord {
                text: AsrNormalizedText::new(part),
                start_ms: ps,
                end_ms: pe,
                kind,
            }
        })
        .collect()
}
