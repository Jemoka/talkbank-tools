//! Rewrite whole-utterance `@s` runs into utterance-level `[- LANG]` precodes.

use talkbank_model::alignment::helpers::{
    TierDomain, WordItem, WordItemMut, walk_words, walk_words_mut,
};
use talkbank_model::model::{
    ChatFile, Header, LanguageCode, Line, MainTier, ReplacedWord, Word, WordLanguageMarker,
};
use talkbank_model::validation::{LanguageResolution, ValidationState};

/// Rewrite summary for one CHAT file.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FixSRewriteStats {
    /// Number of utterances rewritten from per-word `@s` markers to `[- LANG]`.
    pub rewritten_utterances: usize,
    /// Number of language codes appended to `@Languages` from explicit `@s:LANG`.
    pub appended_language_codes: usize,
}

impl FixSRewriteStats {
    /// Returns `true` when no utterance rewrite was needed.
    pub fn is_empty(self) -> bool {
        self.rewritten_utterances == 0 && self.appended_language_codes == 0
    }
}

/// Rewrite whole-utterance language switches in one CHAT file in place.
///
/// Uses the same `%mor`-bearing detection semantics as E255:
/// if all lexical content in an utterance resolves to one language override,
/// set the utterance precode and clear per-word markers that resolve to that
/// same target language. Also appends any explicit `@s:LANG` codes missing from
/// the file's `@Languages` header.
pub fn rewrite_whole_utterance_language_switches<S: ValidationState>(
    chat_file: &mut ChatFile<S>,
) -> FixSRewriteStats {
    let appended_language_codes = append_missing_explicit_language_declarations(chat_file);
    let default_language = chat_file.languages.first().cloned();
    let declared_languages = chat_file.languages.iter().cloned().collect::<Vec<_>>();
    let mut stats = FixSRewriteStats {
        appended_language_codes,
        ..FixSRewriteStats::default()
    };

    for line in &mut chat_file.lines {
        let Line::Utterance(utterance) = line else {
            continue;
        };

        if rewrite_main_tier_language_switch(
            &mut utterance.main,
            default_language.as_ref(),
            &declared_languages,
        ) {
            stats.rewritten_utterances += 1;
        }
    }

    stats
}

fn append_missing_explicit_language_declarations<S: ValidationState>(
    chat_file: &mut ChatFile<S>,
) -> usize {
    let missing = collect_missing_explicit_language_codes(chat_file);
    if missing.is_empty() {
        return 0;
    }

    let Some(header_idx) = chat_file.lines.iter().rposition(|line| {
        matches!(
            line,
            Line::Header { header, .. } if matches!(header.as_ref(), Header::Languages { .. })
        )
    }) else {
        return 0;
    };

    chat_file.languages.extend(missing.iter().cloned());
    if let Line::Header { header, .. } = &mut chat_file.lines[header_idx]
        && let Header::Languages { codes } = header.as_mut()
    {
        codes.extend(missing.iter().cloned());
    }

    missing.len()
}

fn collect_missing_explicit_language_codes<S: ValidationState>(
    chat_file: &ChatFile<S>,
) -> Vec<LanguageCode> {
    let mut known = chat_file.languages.iter().cloned().collect::<Vec<_>>();
    let mut missing = Vec::new();

    for line in &chat_file.lines {
        let Line::Utterance(utterance) = line else {
            continue;
        };
        collect_missing_explicit_languages_from_main_tier(
            &utterance.main,
            &mut known,
            &mut missing,
        );
    }

    missing
}

fn collect_missing_explicit_languages_from_main_tier(
    main_tier: &MainTier,
    known: &mut Vec<LanguageCode>,
    missing: &mut Vec<LanguageCode>,
) {
    walk_words(
        &main_tier.content.content,
        Some(TierDomain::Mor),
        &mut |item| match item {
            WordItem::Word(word) => record_missing_explicit_language(word, known, missing),
            WordItem::ReplacedWord(replaced) => {
                record_missing_explicit_languages_in_replaced_word(replaced, known, missing);
            }
            WordItem::Separator(_) => {}
        },
    );
}

fn record_missing_explicit_languages_in_replaced_word(
    replaced: &ReplacedWord,
    known: &mut Vec<LanguageCode>,
    missing: &mut Vec<LanguageCode>,
) {
    record_missing_explicit_language(&replaced.word, known, missing);
    for word in &replaced.replacement.words {
        record_missing_explicit_language(word, known, missing);
    }
}

fn record_missing_explicit_language(
    word: &Word,
    known: &mut Vec<LanguageCode>,
    missing: &mut Vec<LanguageCode>,
) {
    let Some(WordLanguageMarker::Explicit(code)) = word.lang.as_ref() else {
        return;
    };
    if known.contains(code) {
        return;
    }

    known.push(code.clone());
    missing.push(code.clone());
}

fn rewrite_main_tier_language_switch(
    main_tier: &mut MainTier,
    default_language: Option<&LanguageCode>,
    declared_languages: &[LanguageCode],
) -> bool {
    let Some(target_language) =
        main_tier.whole_utterance_language_switch_target(default_language, declared_languages)
    else {
        return false;
    };

    let original_tier_language = main_tier
        .content
        .language_code
        .as_ref()
        .or(default_language)
        .cloned();
    let mut cleared_any_word_marker = false;

    // Walk EVERY main-tier word — regular words AND fillers (`&~`,
    // `&-`, `&+`) AND nonwords. Domain-filtering to MOR here would
    // skip fillers, leaving any `@s` shortcut on a filler in place;
    // that shortcut would then resolve against the new tier-language
    // (set by the precode below) and FLIP its meaning. The predicate
    // has already verified that every word's `@s` resolves to
    // `target_language`, so it is safe — and necessary — to clear
    // every `@s` marker that resolves there.
    walk_words_mut(
        &mut main_tier.content.content,
        None,
        &mut |item| match item {
            WordItemMut::Word(word) => {
                cleared_any_word_marker |= clear_matching_word_language_marker(
                    word,
                    original_tier_language.as_ref(),
                    declared_languages,
                    &target_language,
                );
            }
            WordItemMut::ReplacedWord(replaced) => {
                cleared_any_word_marker |= clear_matching_replaced_word_language_markers(
                    replaced,
                    original_tier_language.as_ref(),
                    declared_languages,
                    &target_language,
                );
            }
            WordItemMut::Separator(_) => {}
        },
    );

    let language_changed = main_tier.content.language_code.as_ref() != Some(&target_language);
    if language_changed {
        main_tier.content.language_code = Some(target_language);
    }

    language_changed || cleared_any_word_marker
}

fn clear_matching_word_language_marker(
    word: &mut Word,
    tier_language: Option<&LanguageCode>,
    declared_languages: &[LanguageCode],
    target_language: &LanguageCode,
) -> bool {
    let Some(_) = word.lang.as_ref() else {
        return false;
    };

    let outcome = talkbank_model::resolve_word_language(word, tier_language, declared_languages);
    if outcome.resolution == LanguageResolution::Single(target_language.clone()) {
        word.lang = None;
        true
    } else {
        false
    }
}

fn clear_matching_replaced_word_language_markers(
    replaced: &mut ReplacedWord,
    tier_language: Option<&LanguageCode>,
    declared_languages: &[LanguageCode],
    target_language: &LanguageCode,
) -> bool {
    let mut cleared_any_word_marker = clear_matching_word_language_marker(
        &mut replaced.word,
        tier_language,
        declared_languages,
        target_language,
    );
    for word in &mut replaced.replacement.words {
        cleared_any_word_marker |= clear_matching_word_language_marker(
            word,
            tier_language,
            declared_languages,
            target_language,
        );
    }
    cleared_any_word_marker
}

#[cfg(test)]
mod tests {
    use super::{FixSRewriteStats, rewrite_whole_utterance_language_switches};
    use talkbank_model::model::WriteChat;
    use talkbank_parser::TreeSitterParser;

    fn rewrite(chat: &str) -> (String, FixSRewriteStats) {
        let parser = TreeSitterParser::new().expect("parser");
        let mut parsed = parser.parse_chat_file(chat).expect("parse chat");
        let stats = rewrite_whole_utterance_language_switches(&mut parsed);
        (parsed.to_chat_string(), stats)
    }

    #[test]
    fn rewrites_whole_utterance_shortcuts_to_precode() {
        let input = "@UTF8
@Begin
@Languages:\teng, spa
@Participants:\tCHI Target_Child
@ID:\teng|corpus|CHI|||||Target_Child|||
*CHI:\thola@s amiga@s .
@End
";
        let expected = "@UTF8
@Begin
@Languages:\teng, spa
@Participants:\tCHI Target_Child
@ID:\teng|corpus|CHI|||||Target_Child|||
*CHI:\t[- spa] hola amiga .
@End
";

        let (rewritten, stats) = rewrite(input);
        assert_eq!(
            stats,
            FixSRewriteStats {
                rewritten_utterances: 1,
                appended_language_codes: 0,
            }
        );
        assert_eq!(rewritten, expected);
    }

    #[test]
    fn rewrites_existing_precode_when_shortcuts_resolve_to_other_language() {
        let input = "@UTF8
@Begin
@Languages:\teng, spa
@Participants:\tCHI Target_Child
@ID:\teng|corpus|CHI|||||Target_Child|||
*CHI:\t[- spa] hello@s there@s .
@End
";
        let expected = "@UTF8
@Begin
@Languages:\teng, spa
@Participants:\tCHI Target_Child
@ID:\teng|corpus|CHI|||||Target_Child|||
*CHI:\t[- eng] hello there .
@End
";

        let (rewritten, stats) = rewrite(input);
        assert_eq!(
            stats,
            FixSRewriteStats {
                rewritten_utterances: 1,
                appended_language_codes: 0,
            }
        );
        assert_eq!(rewritten, expected);
    }

    #[test]
    fn leaves_mixed_tagged_and_untagged_utterance_unchanged() {
        let input = "@UTF8
@Begin
@Languages:\teng, spa
@Participants:\tCHI Target_Child
@ID:\teng|corpus|CHI|||||Target_Child|||
*CHI:\thola@s friend .
@End
";

        let (rewritten, stats) = rewrite(input);
        assert!(stats.is_empty());
        assert_eq!(rewritten, input);
    }

    #[test]
    fn appends_missing_explicit_word_language_to_languages_header() {
        let input = "@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Target_Child
@ID:\teng|corpus|CHI|||||Target_Child|||
*CHI:\thola@s:spa friend .
@End
";
        let expected = "@UTF8
@Begin
@Languages:\teng, spa
@Participants:\tCHI Target_Child
@ID:\teng|corpus|CHI|||||Target_Child|||
*CHI:\thola@s:spa friend .
@End
";

        let (rewritten, stats) = rewrite(input);
        assert_eq!(
            stats,
            FixSRewriteStats {
                rewritten_utterances: 0,
                appended_language_codes: 1,
            }
        );
        assert_eq!(rewritten, expected);
    }

    #[test]
    fn appends_missing_explicit_original_replaced_word_language_to_languages_header() {
        let input = "@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Target_Child
@ID:\teng|corpus|CHI|||||Target_Child|||
*CHI:\thola@s:spa [: hello] friend .
@End
";
        let expected = "@UTF8
@Begin
@Languages:\teng, spa
@Participants:\tCHI Target_Child
@ID:\teng|corpus|CHI|||||Target_Child|||
*CHI:\thola@s:spa [: hello] friend .
@End
";

        let (rewritten, stats) = rewrite(input);
        assert_eq!(
            stats,
            FixSRewriteStats {
                rewritten_utterances: 0,
                appended_language_codes: 1,
            }
        );
        assert_eq!(rewritten, expected);
    }

    /// RED → GREEN regression — when fix-s adds a `[- LANG]` precode,
    /// EVERY `@s`-marked word in the main tier (including fillers,
    /// nonwords, phonological fragments) must have its marker cleared.
    /// Otherwise the marker's resolution flips:
    ///
    /// - Pre-rewrite (tier-default `spa`, declared `[spa, eng]`):
    ///   `&~orin@s` shortcut resolves to "the OTHER declared
    ///   language" = `eng`.
    /// - Post-rewrite with `[- eng]` precode but unchanged filler:
    ///   tier is now `eng`, shortcut resolves to "the OTHER
    ///   declared language" = `spa`.
    /// - The filler flipped from `eng` to `spa` despite being the
    ///   same source text.
    ///
    /// The bug was that fix-s's clear pass walked
    /// `walk_words_mut(... Some(TierDomain::Mor) ...)`, which skips
    /// fillers/nonwords. Widening to a domain-agnostic walk fixes
    /// it because the predicate already verified every word
    /// (including fillers) resolves to the target language, so
    /// clearing all `@s` markers is safe.
    #[test]
    fn filler_with_at_s_shortcut_is_cleared_to_avoid_resolution_flip() {
        let input = "@UTF8
@Begin
@Languages:\tspa, eng
@Participants:\tCHI Target_Child
@ID:\tspa|corpus|CHI|||||Target_Child|||
*CHI:\thello@s &~orin@s .
@End
";
        let expected = "@UTF8
@Begin
@Languages:\tspa, eng
@Participants:\tCHI Target_Child
@ID:\tspa|corpus|CHI|||||Target_Child|||
*CHI:\t[- eng] hello &~orin .
@End
";
        let (rewritten, stats) = rewrite(input);
        assert_eq!(
            stats,
            FixSRewriteStats {
                rewritten_utterances: 1,
                appended_language_codes: 0,
            }
        );
        assert_eq!(rewritten, expected);
    }

    /// RED → GREEN regression — same flip-prevention rule for
    /// `&-`-style filler and `&+`-style phonological fragment with
    /// `@s` shortcut markers.
    #[test]
    fn dash_and_plus_form_fillers_clear_their_at_s_shortcut() {
        let input = "@UTF8
@Begin
@Languages:\tspa, eng
@Participants:\tCHI Target_Child
@ID:\tspa|corpus|CHI|||||Target_Child|||
*CHI:\thello@s &-um@s &+w@s .
@End
";
        let expected = "@UTF8
@Begin
@Languages:\tspa, eng
@Participants:\tCHI Target_Child
@ID:\tspa|corpus|CHI|||||Target_Child|||
*CHI:\t[- eng] hello &-um &+w .
@End
";
        let (rewritten, stats) = rewrite(input);
        assert_eq!(
            stats,
            FixSRewriteStats {
                rewritten_utterances: 1,
                appended_language_codes: 0,
            }
        );
        assert_eq!(rewritten, expected);
    }
}
