//! Utterance-level language metadata computation.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Language_Switching>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#SecondLanguage_Marker_Single>

use super::super::Utterance;
use crate::model::language_metadata::WordLanguages;
use crate::validation::word::language::LanguageResolution;
use crate::{
    LanguageCode, LanguageMetadata, LanguageSource, UtteranceLanguage, UtteranceLanguageMetadata,
    WordLanguageInfo, WordLanguageMarker,
};

/// Add one alignable word's language record and advance the alignable index.
///
/// `alignable_index` tracks only alignable lexical items (not pauses, punctuation,
/// events, etc.) so indices match tier-alignment domains used elsewhere.
fn add_word_language_info(
    metadata: &mut LanguageMetadata,
    alignable_index: &mut usize,
    word: &crate::model::Word,
    tier_language: Option<&LanguageCode>,
    declared_languages: &[LanguageCode],
    utterance_language: &UtteranceLanguage,
) {
    use crate::validation::resolve_word_language;

    let outcome = resolve_word_language(word, tier_language, declared_languages);
    metadata.add_word(WordLanguageInfo::new(
        *alignable_index,
        resolution_to_metadata_languages(&outcome.resolution),
        resolve_word_language_source(word.lang.as_ref(), &outcome.resolution, utterance_language),
    ));

    *alignable_index += 1;
}

/// Traverse bracketed group content and add language records for nested words.
fn add_group_language_metadata(
    items: &[crate::model::BracketedItem],
    metadata: &mut LanguageMetadata,
    alignable_index: &mut usize,
    tier_language: Option<&LanguageCode>,
    declared_languages: &[LanguageCode],
    utterance_language: &UtteranceLanguage,
) {
    for item in items {
        match item {
            crate::model::BracketedItem::Word(word) => add_word_language_info(
                metadata,
                alignable_index,
                word,
                tier_language,
                declared_languages,
                utterance_language,
            ),
            crate::model::BracketedItem::AnnotatedWord(annotated) => add_word_language_info(
                metadata,
                alignable_index,
                &annotated.inner,
                tier_language,
                declared_languages,
                utterance_language,
            ),
            crate::model::BracketedItem::ReplacedWord(replaced) => add_word_language_info(
                metadata,
                alignable_index,
                &replaced.word,
                tier_language,
                declared_languages,
                utterance_language,
            ),
            crate::model::BracketedItem::AnnotatedGroup(annotated_group) => {
                add_group_language_metadata(
                    &annotated_group.inner.content.content,
                    metadata,
                    alignable_index,
                    tier_language,
                    declared_languages,
                    utterance_language,
                );
            }
            _ => {}
        }
    }
}

/// Convert validation-layer language resolution into persisted metadata representation.
fn resolution_to_metadata_languages(resolution: &LanguageResolution) -> WordLanguages {
    match resolution {
        LanguageResolution::Single(code) => WordLanguages::Single(code.clone()),
        LanguageResolution::Multiple(codes) => WordLanguages::Multiple(codes.clone()),
        LanguageResolution::Ambiguous(codes) => WordLanguages::Ambiguous(codes.clone()),
        LanguageResolution::Unresolved => WordLanguages::Unresolved,
    }
}

/// Map a word-level language marker to persisted `LanguageSource`.
fn source_from_word_marker(marker: &WordLanguageMarker) -> LanguageSource {
    match marker {
        WordLanguageMarker::Shortcut => LanguageSource::WordShortcut,
        WordLanguageMarker::Explicit(_)
        | WordLanguageMarker::Multiple(_)
        | WordLanguageMarker::Ambiguous(_) => LanguageSource::WordExplicit,
    }
}

/// Resolve `WordLanguageInfo.source`, preserving unrecoverable state explicitly.
fn resolve_word_language_source(
    marker: Option<&WordLanguageMarker>,
    resolution: &LanguageResolution,
    utterance_language: &UtteranceLanguage,
) -> LanguageSource {
    if matches!(resolution, LanguageResolution::Unresolved) {
        return LanguageSource::Unresolved;
    }

    if let Some(marker) = marker {
        source_from_word_marker(marker)
    } else {
        utterance_language.source()
    }
}

impl Utterance {
    /// Compute and store language metadata for all alignable words in this utterance.
    ///
    /// Baseline language is resolved first (`UtteranceLanguage`), then each
    /// alignable word resolves effective language using:
    /// - file default from `@Languages`
    /// - utterance-scoped override (`[- code]`)
    /// - word-level markers (`@s`, `@s:code`, ambiguous/multiple forms)
    ///
    /// # Parameters
    /// - `default_language`: primary language from `@Languages`
    /// - `declared_languages`: full ordered `@Languages` list for disambiguation
    pub fn compute_language_metadata(
        &mut self,
        default_language: Option<&LanguageCode>,
        declared_languages: &[LanguageCode],
    ) {
        use crate::model::UtteranceContent;

        // Determine utterance baseline language state.
        self.utterance_language = if let Some(code) = self.main.content.language_code.as_ref() {
            UtteranceLanguage::ResolvedTierScoped { code: code.clone() }
        } else if let Some(code) = default_language {
            UtteranceLanguage::ResolvedDefault { code: code.clone() }
        } else {
            UtteranceLanguage::Unresolved
        };

        let tier_language = self.utterance_language.code();
        let mut metadata = LanguageMetadata::new(tier_language.cloned());
        let mut alignable_index = 0;

        // Iterate through main tier content and resolve language for each alignable word
        for content_item in self.main.content.content.iter() {
            match content_item {
                UtteranceContent::Word(word) => add_word_language_info(
                    &mut metadata,
                    &mut alignable_index,
                    word,
                    tier_language,
                    declared_languages,
                    &self.utterance_language,
                ),
                UtteranceContent::AnnotatedWord(annotated_word) => add_word_language_info(
                    &mut metadata,
                    &mut alignable_index,
                    &annotated_word.inner,
                    tier_language,
                    declared_languages,
                    &self.utterance_language,
                ),
                UtteranceContent::ReplacedWord(replaced_word) => add_word_language_info(
                    &mut metadata,
                    &mut alignable_index,
                    &replaced_word.word,
                    tier_language,
                    declared_languages,
                    &self.utterance_language,
                ),
                UtteranceContent::Group(group) => add_group_language_metadata(
                    &group.content.content,
                    &mut metadata,
                    &mut alignable_index,
                    tier_language,
                    declared_languages,
                    &self.utterance_language,
                ),
                UtteranceContent::AnnotatedGroup(annotated_group) => add_group_language_metadata(
                    &annotated_group.inner.content.content,
                    &mut metadata,
                    &mut alignable_index,
                    tier_language,
                    declared_languages,
                    &self.utterance_language,
                ),
                // Non-alignable content doesn't increment index
                _ => {}
            }
        }

        self.language_metadata = UtteranceLanguageMetadata::computed(metadata);
    }
}
