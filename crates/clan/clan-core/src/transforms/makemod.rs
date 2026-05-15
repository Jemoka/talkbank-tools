//! MAKEMOD -- generate `%mod` tier from pronunciation lexicon lookup.
//!
//! Reimplements CLAN's MAKEMOD command, which looks up each countable word on
//! main tiers in a pronunciation lexicon (CMU dictionary format) and generates
//! a `%mod` dependent tier with the phonemic transcription. Words not found
//! in the lexicon are marked with `???`.
//!
//! # External data
//!
//! Requires a CMU-format lexicon file (default: `cmulex.cut` from the CLAN
//! `lib/` directory). Format: `WORD  phoneme1 phoneme2 ...` (one entry per
//! line). Lines starting with `#` or `%` are treated as comments. Words with
//! `(N)` suffix (variant number like `READ(2)`) are treated as pronunciation
//! alternatives for the base word.
//!
//! # Differences from CLAN
//!
//! - Operates on the typed AST rather than raw text line scanning.
//! - Uses the framework transform pipeline (parse → transform → serialize).
//! - Extracts countable words via the shared `countable_words()` utility
//!   instead of ad-hoc word tokenization on raw main-tier text.
//! - Generated `%mod:` tier is stored as a user-defined dependent tier in
//!   the AST rather than appended as a raw text line.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use talkbank_model::{ChatFile, DependentTier, Line, NonEmptyString, UserDefinedDependentTier};

use crate::framework::word_filter::countable_words;
use crate::framework::{TransformCommand, TransformError};

/// Configuration for the MAKEMOD command.
pub struct MakemodConfig {
    /// Path to the pronunciation lexicon file.
    pub lexicon_path: PathBuf,
    /// Show all alternative pronunciations (default: first only).
    pub all_alternatives: bool,
}

/// MAKEMOD transform: add %mod tier from pronunciation lexicon.
pub struct MakemodCommand {
    lexicon: HashMap<String, Vec<String>>,
    all_alternatives: bool,
}

impl MakemodCommand {
    /// Create a new MAKEMOD command, loading the lexicon from file.
    pub fn new(config: MakemodConfig) -> Result<Self, TransformError> {
        let lexicon = load_lexicon(&config.lexicon_path)?;
        Ok(Self {
            lexicon,
            all_alternatives: config.all_alternatives,
        })
    }
}

impl TransformCommand for MakemodCommand {
    type Config = MakemodConfig;

    /// Look up each countable word in the lexicon and append a `%mod` tier.
    fn transform(&self, file: &mut ChatFile) -> Result<(), TransformError> {
        for line in file.lines.iter_mut() {
            if let Line::Utterance(utt) = line {
                let mut mod_parts: Vec<String> = Vec::new();

                for word in countable_words(&utt.main.content.content) {
                    let upper = word.cleaned_text().to_uppercase();
                    if let Some(pronunciations) = self.lexicon.get(&upper) {
                        if self.all_alternatives && pronunciations.len() > 1 {
                            mod_parts.push(pronunciations.join("^"));
                        } else if let Some(first) = pronunciations.first() {
                            mod_parts.push(first.clone());
                        }
                    } else {
                        mod_parts.push("???".to_owned());
                    }
                }

                if !mod_parts.is_empty() {
                    let mod_text = mod_parts.join(" ");
                    if let (Some(label), Some(content)) =
                        (NonEmptyString::new("mod"), NonEmptyString::new(&mod_text))
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
        }
        Ok(())
    }
}

/// Load a CMU-format pronunciation lexicon.
///
/// Format: `WORD  phoneme1 phoneme2 ...`
/// Lines starting with `#` or `%` are comments.
/// Words with `(N)` suffix (variant number) are treated as alternatives.
fn load_lexicon(path: &Path) -> Result<HashMap<String, Vec<String>>, TransformError> {
    let content = std::fs::read_to_string(path).map_err(|e| {
        TransformError::Transform(format!(
            "Cannot read lexicon file '{}': {e}",
            path.display()
        ))
    })?;

    let mut lexicon: HashMap<String, Vec<String>> = HashMap::new();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('%') {
            continue;
        }

        let mut parts = line.splitn(2, |c: char| c.is_whitespace());
        let word = match parts.next() {
            Some(w) => w,
            None => continue,
        };
        let pronunciation = match parts.next() {
            Some(p) => p.trim(),
            None => continue,
        };

        // Strip variant number suffix like "(2)" from "WORD(2)"
        let base_word = if let Some(idx) = word.find('(') {
            &word[..idx]
        } else {
            word
        };

        let upper = base_word.to_uppercase();
        lexicon
            .entry(upper)
            .or_default()
            .push(pronunciation.to_owned());
    }

    Ok(lexicon)
}
