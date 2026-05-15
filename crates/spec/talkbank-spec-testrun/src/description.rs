//! Generate informative descriptions for CHAT construct examples

use talkbank_model::model::{WordCategory, WordContent};
use talkbank_model::ErrorCollector;
use talkbank_model::{ParseErrors, ParseOutcome};
use talkbank_parser::TreeSitterParser;
use thiserror::Error;

/// Enum variants for DescriptionError.
#[derive(Debug, Error)]
pub enum DescriptionError {
    #[error("Failed to parse {context}: {source}")]
    Parse {
        context: &'static str,
        #[source]
        source: ParseErrors,
    },
}

/// Parses error.
fn parse_error(context: &'static str, source: ParseErrors) -> DescriptionError {
    DescriptionError::Parse { context, source }
}

/// Generate a description for a construct based on its input and fence type
pub fn generate_description(input: &str, fence_type: &str) -> Result<String, DescriptionError> {
    match fence_type {
        "word" | "word-compound" | "word-special" => generate_word_description(input),
        "chat-file" => generate_chatfile_description(input),
        _ => Ok(format!("Example using {}", fence_type)),
    }
}

/// Generate description for word-level constructs
fn generate_word_description(input: &str) -> Result<String, DescriptionError> {
    let parser = TreeSitterParser::new().expect("TreeSitterParser should construct");
    let errors = ErrorCollector::new();
    let word = match parser.parse_word_fragment(input, 0, &errors) {
        ParseOutcome::Parsed(word) => word,
        ParseOutcome::Rejected => {
            return Err(parse_error("word", ParseErrors::from(errors.into_vec())));
        }
    };
    let mut features: Vec<String> = Vec::new();

    if let Some(category) = &word.category {
        match category {
            WordCategory::Omission | WordCategory::CAOmission => {
                features.push("omission marker".to_string());
            }
            WordCategory::Nonword => {
                features.push("nonword marker".to_string());
            }
            WordCategory::Filler => {
                features.push("filler marker".to_string());
            }
            WordCategory::PhonologicalFragment => {
                features.push("fragment marker".to_string());
            }
        }
    }

    if word.form_type.is_some() {
        features.push("form type marker".to_string());
    }

    if word.lang.is_some() {
        features.push("language marker".to_string());
    }

    if word.untranscribed().is_some() {
        return Ok("Special form placeholder word".to_string());
    }

    let compound_markers = word
        .content
        .iter()
        .filter(|item| matches!(item, WordContent::CompoundMarker(_)))
        .count();
    if compound_markers > 0 {
        features.push(format!("compound ({} parts)", compound_markers + 1));
    }

    if word
        .content
        .iter()
        .any(|item| matches!(item, WordContent::Lengthening(_)))
    {
        features.push("lengthening".to_string());
    }

    if word
        .content
        .iter()
        .any(|item| matches!(item, WordContent::OverlapPoint(_)))
    {
        features.push("overlap markers".to_string());
    }

    if word
        .content
        .iter()
        .any(|item| matches!(item, WordContent::Shortening(_)))
    {
        features.push("shortening".to_string());
    }

    if word
        .content
        .iter()
        .any(|item| matches!(item, WordContent::SyllablePause(_)))
    {
        features.push("word-internal markers".to_string());
    }

    if word
        .content
        .iter()
        .any(|item| matches!(item, WordContent::StressMarker(_)))
    {
        features.push("stress markers".to_string());
    }

    if word.content.iter().any(|item| {
        matches!(
            item,
            WordContent::CAElement(_) | WordContent::CADelimiter(_)
        )
    }) {
        features.push("CA markers".to_string());
    }

    if features.is_empty() {
        Ok("Plain word without special markers".to_string())
    } else {
        Ok(format!("Word with {}", features.join(", ")))
    }
}

/// Generate description for complete CHAT file examples
fn generate_chatfile_description(input: &str) -> Result<String, DescriptionError> {
    let parser = TreeSitterParser::new().expect("TreeSitterParser should construct");
    let chat_file = parser
        .parse_chat_file(input)
        .map_err(|err| parse_error("chat file", err))?;

    let mut has_mor = false;
    let mut has_gra = false;
    let mut has_pho = false;
    let mut has_com = false;

    for utterance in chat_file.utterances() {
        if utterance.mor().is_some() {
            has_mor = true;
        }
        if utterance.gra().is_some() {
            has_gra = true;
        }
        if utterance.pho().is_some() {
            has_pho = true;
        }
        if utterance.com().is_some() {
            has_com = true;
        }
    }

    if has_mor {
        Ok("Morphology tier example".to_string())
    } else if has_gra {
        Ok("Grammatical relations tier example".to_string())
    } else if has_pho {
        Ok("Phonology tier example".to_string())
    } else if has_com {
        Ok("Comment tier example".to_string())
    } else {
        Ok("Complete CHAT file example".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests simple word.
    #[test]
    fn test_simple_word() -> Result<(), DescriptionError> {
        let desc = generate_word_description("hello")?;
        assert_eq!(desc, "Plain word without special markers");
        Ok(())
    }

    /// Tests omission.
    #[test]
    fn test_omission() -> Result<(), DescriptionError> {
        let desc = generate_word_description("0is")?;
        assert!(desc.contains("omission marker"));
        Ok(())
    }

    /// Tests nonword.
    #[test]
    fn test_nonword() -> Result<(), DescriptionError> {
        let desc = generate_word_description("&~foo")?;
        assert!(desc.contains("nonword marker"));
        Ok(())
    }

    /// Tests compound.
    #[test]
    fn test_compound() -> Result<(), DescriptionError> {
        let desc = generate_word_description("ice+cream")?;
        assert!(desc.contains("compound (2 parts)"));
        Ok(())
    }

    /// Tests form type.
    #[test]
    fn test_form_type() -> Result<(), DescriptionError> {
        let desc = generate_word_description("foo@b")?;
        assert!(desc.contains("form type marker"));
        Ok(())
    }

    /// Tests lengthening.
    #[test]
    fn test_lengthening() -> Result<(), DescriptionError> {
        let desc = generate_word_description("a:")?;
        assert!(desc.contains("lengthening"));
        Ok(())
    }

    /// Tests special form.
    #[test]
    fn test_special_form() -> Result<(), DescriptionError> {
        let desc = generate_word_description("xxx")?;
        assert_eq!(desc, "Special form placeholder word");
        Ok(())
    }

    /// Tests multiple features.
    #[test]
    fn test_multiple_features() -> Result<(), DescriptionError> {
        let desc = generate_word_description("0ice+cream@b")?;
        assert!(desc.contains("omission marker"));
        assert!(desc.contains("compound"));
        assert!(desc.contains("form type marker"));
        Ok(())
    }
}
