//! ORT -- orthographic conversion via dictionary lookup.
//!
//! Reimplements CLAN's CONVORT command, which applies orthographic conversion
//! rules from a dictionary file to main-tier words. When a word is modified,
//! the original main-tier text is preserved on a `%ort:` dependent tier for
//! reference.
//!
//! # External data
//!
//! Requires an orthographic conversion dictionary (default: `ort.cut`).
//! Format: `from_word  to_word` (one pair per line, tab or space separated).
//! Lines starting with `#` or `;` are treated as comments. Lookups are
//! case-insensitive.
//!
//! # Differences from CLAN
//!
//! - Operates on the typed AST rather than raw text line scanning.
//! - Uses the framework transform pipeline (parse → transform → serialize).
//! - Modifies `Word` surface forms in the AST and serializes the original
//!   main tier to `%ort:` via `WriteChat`, instead of duplicating raw text
//!   lines and applying string substitutions.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use talkbank_model::{ChatFile, DependentTier, Line, NonEmptyString, UserDefinedDependentTier};

use crate::framework::{TransformCommand, TransformError};

/// Configuration for the ORT command.
pub struct OrtConfig {
    /// Path to the orthographic conversion dictionary.
    pub dictionary_path: PathBuf,
}

/// ORT transform: apply orthographic conversion.
pub struct OrtCommand {
    /// Word → replacement mapping.
    dictionary: HashMap<String, String>,
}

impl OrtCommand {
    /// Create a new ORT command, loading the dictionary from file.
    pub fn new(config: OrtConfig) -> Result<Self, TransformError> {
        let dictionary = load_dictionary(&config.dictionary_path)?;
        Ok(Self { dictionary })
    }
}

impl TransformCommand for OrtCommand {
    type Config = OrtConfig;

    /// Apply dictionary-based word substitutions and preserve originals on `%ort:`.
    fn transform(&self, file: &mut ChatFile) -> Result<(), TransformError> {
        for line in file.lines.iter_mut() {
            if let Line::Utterance(utt) = line {
                let original_content = utt.main.content.to_content_string();

                // Apply word substitutions on main tier content
                let mut modified = false;
                for item in utt.main.content.content.iter_mut() {
                    if let talkbank_model::UtteranceContent::Word(word) = item {
                        let raw = word.raw_text().to_owned();
                        let lower = raw.to_lowercase();
                        if let Some(replacement) = self.dictionary.get(&lower) {
                            word.replace_simple_text(replacement.clone());
                            modified = true;
                        }
                    }
                }

                // Add %ort tier with original text if modifications were made
                if modified
                    && let (Some(label), Some(content)) = (
                        NonEmptyString::new("ort"),
                        NonEmptyString::new(original_content.trim()),
                    )
                {
                    utt.dependent_tiers.push(DependentTier::UserDefined(
                        UserDefinedDependentTier {
                            label,
                            content,
                            span: talkbank_model::Span::DUMMY,
                        },
                    ));
                }
            }
        }
        Ok(())
    }
}

/// Load an orthographic conversion dictionary.
///
/// Format: `from_word  to_word` (one pair per line).
/// Lines starting with `#` or `;` are comments.
fn load_dictionary(path: &Path) -> Result<HashMap<String, String>, TransformError> {
    let content = std::fs::read_to_string(path).map_err(|e| {
        TransformError::Transform(format!(
            "Cannot read dictionary file '{}': {e}",
            path.display()
        ))
    })?;

    let mut dict = HashMap::new();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }

        let mut parts = line.splitn(2, |c: char| c.is_whitespace());
        let from = match parts.next() {
            Some(w) => w.to_lowercase(),
            None => continue,
        };
        let to = match parts.next() {
            Some(w) => w.trim().to_owned(),
            None => continue,
        };

        dict.insert(from, to);
    }

    Ok(dict)
}
