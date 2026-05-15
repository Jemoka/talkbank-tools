//! Semantic word filtering for analysis commands.
//!
//! CLAN's original analysis commands exclude certain "words" from counting by
//! checking raw string prefixes: `0`, `&`, `+`, `-`, `#`. We use typed AST
//! fields instead, representing each of these categories as distinct types:
//!
//! | CLAN text pattern | Semantic intent | AST representation |
//! |---|---|---|
//! | `word[0] == '#'` | Skip pauses | `Pause` (not a `Word` at all) |
//! | `word[0] == '+'` | Skip terminators | `Terminator` (separate AST level) |
//! | `word == "xxx"` | Skip unintelligible | `Word { untranscribed: Some(Unintelligible) }` |
//! | `word == "yyy"` | Skip phonetic coding | `Word { untranscribed: Some(Phonetic) }` |
//! | `word == "www"` | Skip untranscribable | `Word { untranscribed: Some(Untranscribed) }` |
//! | `word[0] == '0'` | Skip omitted words | `Word { category: Some(Omission) }` |
//! | `word[0] == '&'` | Skip fillers/nonwords | `Word { category: Some(Filler\|Nonword\|Fragment) }` |
//! | `word[0] == '-'` | (unclear) | Not a meaningful CHAT category |
//!
//! Pauses, terminators, events, and actions are already separate AST node
//! types that our tree walk never visits. The only filtering needed is on
//! `Word` nodes that carry semantic annotations indicating they are not
//! countable lexical items.

use talkbank_model::{BracketedItem, Utterance, UtteranceContent, Word, WordCategory};

/// Determine whether a word contributes lexical material to analysis counts.
///
/// A word is **not countable** if it represents:
/// - Untranscribed material (`xxx`, `yyy`, `www`) — unintelligible or
///   deliberately omitted speech that has no lexical content
/// - Omitted words (`0is`, `0det`) — words the speaker should have produced
///   but didn't; they describe an absence, not a presence
/// - Fillers (`&-um`, `&-uh`) — non-lexical vocalizations used for turn-holding
/// - Nonwords (`&~gaga`) — babbling or invented sounds with no lexical status
/// - Phonological fragments (`&+fr`) — incomplete word attempts
///
/// These correspond to CLAN's default exclusions, but expressed through the
/// type system rather than string prefix matching.
///
/// # What is already excluded by tree structure
///
/// The following are separate AST node types that the tree walk never reaches:
/// - **Pauses** (`Pause`) — CLAN's `#` prefix check
/// - **Events** (`Event`) — CLAN's `&=` prefix check (e.g., `&=laughs`)
/// - **Actions** (`Action`) — standalone `0`
/// - **Terminators** (`Terminator`) — CLAN's `+` prefix check
///
/// # Postcondition
///
/// If this returns `true`, the word has genuine lexical content suitable
/// for frequency counting, MLU computation, and other analyses.
pub fn is_countable_word(word: &Word) -> bool {
    // Untranscribed material has no lexical content
    if word.untranscribed().is_some() {
        return false;
    }

    // Omissions, fillers, nonwords, and fragments are not lexical items
    if let Some(ref category) = word.category
        && !is_countable_category(category)
    {
        return false;
    }

    // Defensive: empty cleaned_text means no lexical content.
    // The model currently prevents constructing empty words, but this
    // guard ensures correctness if that invariant ever relaxes.
    if word.cleaned_text().is_empty() {
        return false;
    }

    true
}

/// Determine whether this word category remains countable for analysis.
///
/// Only `CAOmission` is countable among categories — it represents uncertain
/// but present speech in CA transcription, unlike standard omissions which
/// represent absent speech.
fn is_countable_category(category: &WordCategory) -> bool {
    match category {
        // Standard omission: word was NOT produced (e.g., "0is" = missing copula)
        WordCategory::Omission => false,
        // Filler: non-lexical vocalization (e.g., "&-um")
        WordCategory::Filler => false,
        // Nonword: babbling with no lexical status (e.g., "&~gaga")
        WordCategory::Nonword => false,
        // Fragment: incomplete word attempt (e.g., "&+fr")
        WordCategory::PhonologicalFragment => false,
        // CA omission: uncertain but present speech in CA mode — countable
        // because the transcriber heard something and attempted to transcribe it
        WordCategory::CAOmission => true,
    }
}

/// Iterator over all countable words in utterance main-tier content.
///
/// Walks the `UtteranceContent` + `BracketedItem` tree recursively, yielding
/// each [`Word`] that passes [`is_countable_word`]. The caller receives
/// `&Word` references and decides how to use them (e.g., to extract
/// [`cleaned_text()`][Word::cleaned_text] for frequency keys).
///
/// Internally collects into a `Vec<&Word>` before iterating; this keeps the
/// borrow checker happy across the two-level tree and is negligible for the
/// 10–50 word utterances typical in CHAT.
///
/// # Usage
///
/// ```ignore
/// for word in countable_words(&utterance.main.content.content) {
///     let key = NormalizedWord::from_word(word);
///     // ...
/// }
/// ```
pub fn countable_words(content: &[UtteranceContent]) -> impl Iterator<Item = &Word> {
    let mut words: Vec<&Word> = Vec::new();
    collect_countable(content, &mut words, false);
    words.into_iter()
}

/// Convenience wrapper: iterate countable words in an utterance's main tier.
///
/// Equivalent to `countable_words(&utterance.main.content.content)`.
pub fn countable_words_in_utterance(utterance: &Utterance) -> impl Iterator<Item = &Word> {
    countable_words(&utterance.main.content.content)
}

/// Like [`countable_words`], but with retracings included.
///
/// When `include_retracings` is true, `ReplacedWord` yields BOTH the original
/// (retraced) word AND the replacement (corrected) words. This corresponds to
/// CLAN's `+r6` flag which counts retraced material.
pub fn countable_words_with_retracings(
    content: &[UtteranceContent],
    include_retracings: bool,
) -> impl Iterator<Item = &Word> {
    let mut words: Vec<&Word> = Vec::new();
    collect_countable(content, &mut words, include_retracings);
    words.into_iter()
}

/// Like [`countable_words_in_utterance`], but with retracings control.
pub fn countable_words_in_utterance_with_retracings(
    utterance: &Utterance,
    include_retracings: bool,
) -> impl Iterator<Item = &Word> {
    countable_words_with_retracings(&utterance.main.content.content, include_retracings)
}

/// Recursively collect countable words from main-tier content into `out`.
///
/// When `include_retracings` is true, `ReplacedWord` yields both the original
/// and replacement words. Otherwise, only the replacement (corrected form) is
/// counted, matching CLAN's default behavior.
///
/// # Invariant
///
/// Every word appended to `out` satisfies `is_countable_word(word) == true`.
fn collect_countable<'a>(
    content: &'a [UtteranceContent],
    out: &mut Vec<&'a Word>,
    include_retracings: bool,
) {
    for item in content {
        match item {
            UtteranceContent::Word(word)
                if is_countable_word(word) => {
                    out.push(word);
                }
            UtteranceContent::AnnotatedWord(annotated)
                if is_countable_word(&annotated.inner) => {
                    out.push(&annotated.inner);
                }
            UtteranceContent::ReplacedWord(replaced) => {
                if include_retracings {
                    // With retracings: count both original and replacement
                    if is_countable_word(&replaced.word) {
                        out.push(&replaced.word);
                    }
                    for w in &replaced.replacement.words {
                        if is_countable_word(w) {
                            out.push(w);
                        }
                    }
                } else if !replaced.replacement.words.is_empty() {
                    // Default: count replacement (corrected form), not original
                    for w in &replaced.replacement.words {
                        if is_countable_word(w) {
                            out.push(w);
                        }
                    }
                } else if is_countable_word(&replaced.word) {
                    out.push(&replaced.word);
                }
            }
            UtteranceContent::Group(group) => {
                collect_countable_bracketed(&group.content.content, out, include_retracings);
            }
            UtteranceContent::AnnotatedGroup(annotated) => {
                collect_countable_bracketed(
                    &annotated.inner.content.content,
                    out,
                    include_retracings,
                );
            }
            UtteranceContent::Retrace(retrace)
                // Retrace targets are excluded by default. When include_retracings
                // is set (CLAN's +r6 flag), count the retraced words too.
                if include_retracings => {
                    collect_countable_bracketed(&retrace.content.content, out, include_retracings);
                }
            UtteranceContent::PhoGroup(group) => {
                collect_countable_bracketed(&group.content.content, out, include_retracings);
            }
            UtteranceContent::SinGroup(group) => {
                collect_countable_bracketed(&group.content.content, out, include_retracings);
            }
            UtteranceContent::Quotation(group) => {
                collect_countable_bracketed(&group.content.content, out, include_retracings);
            }
            _ => {}
        }
    }
}

/// Recursively collect countable words from bracketed (nested) content.
fn collect_countable_bracketed<'a>(
    items: &'a [BracketedItem],
    out: &mut Vec<&'a Word>,
    include_retracings: bool,
) {
    for item in items {
        match item {
            BracketedItem::Word(word) if is_countable_word(word) => {
                out.push(word);
            }
            BracketedItem::AnnotatedWord(annotated) if is_countable_word(&annotated.inner) => {
                out.push(&annotated.inner);
            }
            BracketedItem::ReplacedWord(replaced) => {
                if include_retracings {
                    if is_countable_word(&replaced.word) {
                        out.push(&replaced.word);
                    }
                    for w in &replaced.replacement.words {
                        if is_countable_word(w) {
                            out.push(w);
                        }
                    }
                } else if !replaced.replacement.words.is_empty() {
                    for w in &replaced.replacement.words {
                        if is_countable_word(w) {
                            out.push(w);
                        }
                    }
                } else if is_countable_word(&replaced.word) {
                    out.push(&replaced.word);
                }
            }
            BracketedItem::AnnotatedGroup(annotated) => {
                collect_countable_bracketed(
                    &annotated.inner.content.content,
                    out,
                    include_retracings,
                );
            }
            BracketedItem::Retrace(retrace) if include_retracings => {
                collect_countable_bracketed(&retrace.content.content, out, include_retracings);
            }
            BracketedItem::PhoGroup(group) => {
                collect_countable_bracketed(&group.content.content, out, include_retracings);
            }
            BracketedItem::SinGroup(group) => {
                collect_countable_bracketed(&group.content.content, out, include_retracings);
            }
            BracketedItem::Quotation(group) => {
                collect_countable_bracketed(&group.content.content, out, include_retracings);
            }
            _ => {}
        }
    }
}

/// Check whether utterance content contains any countable lexical word.
///
/// This is used by MLU to exclude utterances that consist entirely of
/// untranscribed material (e.g., `*CHI: xxx .`) from the utterance count.
/// Such utterances would otherwise deflate MLU by adding zero-morpheme
/// utterances to the denominator.
///
/// # Precondition
///
/// `content` should be the main tier content of an utterance.
pub fn has_countable_words(content: &[talkbank_model::UtteranceContent]) -> bool {
    use talkbank_model::UtteranceContent;
    for item in content {
        match item {
            UtteranceContent::Word(word) if is_countable_word(word) => {
                return true;
            }
            UtteranceContent::AnnotatedWord(annotated) if is_countable_word(&annotated.inner) => {
                return true;
            }
            UtteranceContent::ReplacedWord(replaced) => {
                // Replacements represent corrected forms — they are countable
                if !replaced.replacement.words.is_empty() {
                    for w in &replaced.replacement.words {
                        if is_countable_word(w) {
                            return true;
                        }
                    }
                } else if is_countable_word(&replaced.word) {
                    return true;
                }
            }
            UtteranceContent::Group(group)
                if has_countable_words_bracketed(&group.content.content) =>
            {
                return true;
            }
            UtteranceContent::AnnotatedGroup(annotated)
                if has_countable_words_bracketed(&annotated.inner.content.content) =>
            {
                return true;
            }
            UtteranceContent::PhoGroup(group)
                if has_countable_words_bracketed(&group.content.content) =>
            {
                return true;
            }
            UtteranceContent::SinGroup(group)
                if has_countable_words_bracketed(&group.content.content) =>
            {
                return true;
            }
            UtteranceContent::Quotation(group)
                if has_countable_words_bracketed(&group.content.content) =>
            {
                return true;
            }
            // Non-word content (events, pauses, actions, etc.) doesn't count
            _ => {}
        }
    }
    false
}

/// Check whether bracketed content contains any countable words.
fn has_countable_words_bracketed(items: &[talkbank_model::BracketedItem]) -> bool {
    use talkbank_model::BracketedItem;
    for item in items {
        match item {
            BracketedItem::Word(word) if is_countable_word(word) => {
                return true;
            }
            BracketedItem::AnnotatedWord(annotated) if is_countable_word(&annotated.inner) => {
                return true;
            }
            BracketedItem::ReplacedWord(replaced) => {
                if !replaced.replacement.words.is_empty() {
                    for w in &replaced.replacement.words {
                        if is_countable_word(w) {
                            return true;
                        }
                    }
                } else if is_countable_word(&replaced.word) {
                    return true;
                }
            }
            BracketedItem::AnnotatedGroup(annotated)
                if has_countable_words_bracketed(&annotated.inner.content.content) =>
            {
                return true;
            }
            BracketedItem::PhoGroup(group)
                if has_countable_words_bracketed(&group.content.content) =>
            {
                return true;
            }
            BracketedItem::SinGroup(group)
                if has_countable_words_bracketed(&group.content.content) =>
            {
                return true;
            }
            BracketedItem::Quotation(group)
                if has_countable_words_bracketed(&group.content.content) =>
            {
                return true;
            }
            _ => {}
        }
    }
    false
}

/// Match a word against a CLAN `+s` search pattern (both should be lowercased).
///
/// CLAN uses exact word matching by default. Wildcards (`*`) match
/// zero or more characters:
/// - `cookie` matches only "cookie" (exact)
/// - `cook*` matches "cookie", "cookies", "cook" (prefix)
/// - `*ing` matches "going", "running" (suffix)
/// - `*ook*` matches "cookie", "book" (contains)
pub fn word_pattern_matches(word: &str, pattern: &str) -> bool {
    if !pattern.contains('*') {
        return word == pattern;
    }

    let parts: Vec<&str> = pattern.split('*').collect();

    if parts.len() == 2 {
        let (prefix, suffix) = (parts[0], parts[1]);
        if prefix.is_empty() && suffix.is_empty() {
            return true; // "*" matches everything
        }
        if prefix.is_empty() {
            return word.ends_with(suffix);
        }
        if suffix.is_empty() {
            return word.starts_with(prefix);
        }
        return word.starts_with(prefix)
            && word.ends_with(suffix)
            && word.len() >= prefix.len() + suffix.len();
    }

    // General multi-wildcard: segments must appear in order
    let mut pos = 0;
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        if i == 0 {
            if !word[pos..].starts_with(part) {
                return false;
            }
            pos += part.len();
        } else if i == parts.len() - 1 {
            if !word[pos..].ends_with(part) {
                return false;
            }
        } else {
            match word[pos..].find(part) {
                Some(found) => pos += found + part.len(),
                None => return false,
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Plain lexical words should be countable.
    #[test]
    fn simple_word_is_countable() {
        let word = Word::simple("dog");
        assert!(is_countable_word(&word));
    }

    /// Untranscribed tokens (`xxx/yyy/www`) should be excluded.
    #[test]
    fn untranscribed_words_are_not_countable() {
        let xxx = Word::simple("xxx");
        let yyy = Word::simple("yyy");
        let www = Word::simple("www");

        assert!(!is_countable_word(&xxx));
        assert!(!is_countable_word(&yyy));
        assert!(!is_countable_word(&www));
    }

    /// Omission/filler/nonword/fragment categories should be excluded.
    #[test]
    fn omissions_fillers_nonwords_fragments_not_countable() {
        let omission = Word::simple("is").with_category(WordCategory::Omission);
        let filler = Word::simple("um").with_category(WordCategory::Filler);
        let nonword = Word::simple("gaga").with_category(WordCategory::Nonword);
        let fragment = Word::simple("fr").with_category(WordCategory::PhonologicalFragment);

        assert!(!is_countable_word(&omission));
        assert!(!is_countable_word(&filler));
        assert!(!is_countable_word(&nonword));
        assert!(!is_countable_word(&fragment));
    }

    /// CA omissions represent present-but-uncertain speech and remain countable.
    #[test]
    fn ca_omission_is_countable() {
        // CA omissions represent uncertain but present speech
        let ca = Word::simple("word").with_category(WordCategory::CAOmission);
        assert!(is_countable_word(&ca));
    }

    /// `has_countable_words` should differentiate lexical from non-lexical input.
    #[test]
    fn has_countable_words_detects_lexical_content() {
        use talkbank_model::UtteranceContent;

        // Utterance with a normal word has countable content
        let word = Word::simple("dog");
        let content = vec![UtteranceContent::Word(Box::new(word))];
        assert!(has_countable_words(&content));

        // Utterance with only untranscribed material has no countable content
        let xxx = Word::simple("xxx");
        let content = vec![UtteranceContent::Word(Box::new(xxx))];
        assert!(!has_countable_words(&content));
    }
}
