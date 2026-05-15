//! Feature signature classification for [`Word`] model objects.
//!
//! Coarsens each word into a [`WordFeatureSignature`] that captures which model
//! features it exercises (category, content types, form type, language marker,
//! POS, content count). Used to deduplicate similar words in the golden word
//! corpus, identify coverage gaps, and group test failures by feature class.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>

use std::collections::BTreeSet;
use talkbank_model::model::{FormType, Word, WordCategory, WordContent, WordLanguageMarker};

/// Classification of word categories for signature purposes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WordCategoryClass {
    /// Word marked as omitted (`0word`).
    Omission,
    /// Conversation-analysis omission.
    CAOmission,
    /// Non-word vocalisation (`&word`).
    Nonword,
    /// Filler word (`&-uh`).
    Filler,
    /// Phonological fragment (`&+fr`).
    PhonologicalFragment,
}

impl WordCategoryClass {
    /// Convert a `WordCategory` into the coarse signature category.
    fn from_category(cat: &WordCategory) -> Self {
        match cat {
            WordCategory::Omission => WordCategoryClass::Omission,
            WordCategory::CAOmission => WordCategoryClass::CAOmission,
            WordCategory::Nonword => WordCategoryClass::Nonword,
            WordCategory::Filler => WordCategoryClass::Filler,
            WordCategory::PhonologicalFragment => WordCategoryClass::PhonologicalFragment,
        }
    }
}

/// Content type kinds for signature classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ContentKind {
    /// Plain text span.
    Text,
    /// Shortened form (`(text)`).
    Shortening,
    /// Compound marker (`+`).
    Compound,
    /// Overlap marker (`<` / `>`).
    OverlapPoint,
    /// Conversation-analysis element.
    CAElement,
    /// Conversation-analysis delimiter.
    CADelimiter,
    /// Stress marker.
    StressMarker,
    /// Vowel or consonant lengthening (`:` in IPA).
    Lengthening,
    /// Pause within a syllable.
    SyllablePause,
    /// Start of underlined region.
    UnderlineBegin,
    /// End of underlined region.
    UnderlineEnd,
}

impl ContentKind {
    /// Convert a `WordContent` variant into the coarse content-kind class.
    fn from_content(content: &WordContent) -> Self {
        match content {
            WordContent::Text(_) => ContentKind::Text,
            WordContent::Shortening(_) => ContentKind::Shortening,
            WordContent::CompoundMarker(_) => ContentKind::Compound,
            WordContent::OverlapPoint(_) => ContentKind::OverlapPoint,
            WordContent::CAElement(_) => ContentKind::CAElement,
            WordContent::CADelimiter(_) => ContentKind::CADelimiter,
            WordContent::StressMarker(_) => ContentKind::StressMarker,
            WordContent::Lengthening(_) => ContentKind::Lengthening,
            WordContent::SyllablePause(_) => ContentKind::SyllablePause,
            WordContent::UnderlineBegin(_) => ContentKind::UnderlineBegin,
            WordContent::UnderlineEnd(_) => ContentKind::UnderlineEnd,
            WordContent::CliticBoundary(_) => ContentKind::Compound, // grouped with compound for signature purposes
        }
    }
}

/// Form type classification for signatures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FormTypeClass {
    /// Built-in single-letter form type (e.g. `@b`, `@c`, `@d`).
    SingleLetter,
    /// User-defined multi-character form type (e.g. `@si:lm`).
    UserDefined,
}

impl FormTypeClass {
    /// Convert a `FormType` into the coarse form-type class.
    fn from_form_type(form: &FormType) -> Self {
        match form {
            FormType::UserDefined { .. } => FormTypeClass::UserDefined,
            _ => FormTypeClass::SingleLetter,
        }
    }
}

/// Language marker presence classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LangPresence {
    /// No language marker present.
    None,
    /// Shortcut language marker (e.g. `[- eng]`).
    Shortcut,
    /// Explicit per-word language marker (e.g. `word@s:eng`).
    Explicit,
}

/// Content count classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ContentCountClass {
    /// Zero content items.
    Empty,
    /// Exactly one content item.
    Single,
    /// 2–5 content items.
    Few,
    /// 6 or more content items.
    Many,
}

impl ContentCountClass {
    /// Bucket a content-item count into an analysis-friendly range.
    fn from_count(count: usize) -> Self {
        match count {
            0 => ContentCountClass::Empty,
            1 => ContentCountClass::Single,
            2..=5 => ContentCountClass::Few,
            _ => ContentCountClass::Many,
        }
    }
}

/// Feature signature of a Word, capturing which model features it exercises.
///
/// Used to:
/// - Deduplicate similar words in golden word corpus
/// - Identify coverage gaps (features not tested)
/// - Group test failures by feature class
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WordFeatureSignature {
    /// Which category class, if any
    pub category: Option<WordCategoryClass>,

    /// Set of content types present
    pub content_types: BTreeSet<ContentKind>,

    /// Form type classification
    pub form_type_class: Option<FormTypeClass>,

    /// Language marker presence
    pub lang_type: LangPresence,

    /// Has POS tag
    pub has_pos: bool,

    /// Has untranscribed status
    pub has_untranscribed: bool,

    /// Content count class
    pub content_count: ContentCountClass,
}

impl WordFeatureSignature {
    /// Extract feature signature from a parsed Word.
    pub fn from_word(word: &Word) -> Self {
        let category = word.category.as_ref().map(WordCategoryClass::from_category);

        let content_types: BTreeSet<ContentKind> =
            word.content.iter().map(ContentKind::from_content).collect();

        let form_type_class = word.form_type.as_ref().map(FormTypeClass::from_form_type);

        let lang_type = match &word.lang {
            None => LangPresence::None,
            Some(WordLanguageMarker::Shortcut) => LangPresence::Shortcut,
            Some(WordLanguageMarker::Explicit(_)) => LangPresence::Explicit,
            Some(WordLanguageMarker::Multiple(_)) => LangPresence::Explicit,
            Some(WordLanguageMarker::Ambiguous(_)) => LangPresence::Explicit,
        };

        let has_pos = word.part_of_speech.is_some();

        let has_untranscribed = word.untranscribed().is_some();

        let content_count = ContentCountClass::from_count(word.content.len());

        Self {
            category,
            content_types,
            form_type_class,
            lang_type,
            has_pos,
            has_untranscribed,
            content_count,
        }
    }

    /// Returns true if this word exercises no special features (plain text only).
    pub fn is_plain_text(&self) -> bool {
        self.category.is_none()
            && self.content_types.len() == 1
            && self.content_types.contains(&ContentKind::Text)
            && self.form_type_class.is_none()
            && self.lang_type == LangPresence::None
            && !self.has_pos
            && !self.has_untranscribed
    }

    /// Returns a human-readable description of this signature.
    pub fn describe(&self) -> String {
        let mut parts = Vec::new();

        if let Some(cat) = &self.category {
            parts.push(format!("cat:{:?}", cat));
        }

        if !self.content_types.is_empty() {
            let content_str: Vec<_> = self
                .content_types
                .iter()
                .map(|k| format!("{:?}", k))
                .collect();
            parts.push(format!("content:[{}]", content_str.join(",")));
        }

        if let Some(form) = &self.form_type_class {
            parts.push(format!("form:{:?}", form));
        }

        if self.lang_type != LangPresence::None {
            parts.push(format!("lang:{:?}", self.lang_type));
        }

        if self.has_pos {
            parts.push("pos".to_string());
        }

        if self.has_untranscribed {
            parts.push("untrans".to_string());
        }

        if self.is_plain_text() {
            "plain_text".to_string()
        } else {
            parts.push(format!("count:{:?}", self.content_count));
            parts.join(", ")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests plain text signature.
    #[test]
    fn test_plain_text_signature() {
        // A word with only text content should be plain text
        let word = Word::new_unchecked("hello", "hello");

        let sig = WordFeatureSignature::from_word(&word);
        assert!(sig.is_plain_text());
        assert_eq!(sig.describe(), "plain_text");
    }

    /// Tests complex signature.
    #[test]
    fn test_complex_signature() {
        // A word with multiple features
        let mut word = Word::new_unchecked("0hel(lo)@b$n", "hello");
        word.category = Some(WordCategory::Omission);
        word.form_type = Some(FormType::B); // @b is babbling
        word.part_of_speech = Some("n".into());

        let sig = WordFeatureSignature::from_word(&word);
        assert!(!sig.is_plain_text());
        assert_eq!(sig.category, Some(WordCategoryClass::Omission));
        assert_eq!(sig.form_type_class, Some(FormTypeClass::SingleLetter));
        assert!(sig.has_pos);
    }
}
