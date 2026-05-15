//! CHSTRING -- string replacement using a changes file.
//!
//! Reimplements CLAN's `chstring` command, which reads a changes file
//! containing find/replace pairs (alternating lines) and applies text
//! substitutions to main-tier words. Replacements are applied to all word
//! nodes, including words inside annotated groups, replacement forms, and
//! bracketed groups.
//!
//! See the [CLAN manual](https://talkbank.org/0info/manuals/CLAN.html#_Toc220409309)
//! for the original command documentation.
//!
//! # Changes file format
//!
//! The changes file contains alternating lines of find and replace strings:
//!
//! ```text
//! find_text1
//! replace_text1
//! find_text2
//! replace_text2
//! ```
//!
//! The file must have an even number of non-empty lines. By default, CLAN
//! looks for `changes.cut` in the current directory.
//!
//! # Differences from CLAN
//!
//! - Operates on the parsed AST rather than raw text, ensuring structural
//!   integrity of the CHAT file after substitution.
//! - Does not support CLAN's regex-based pattern matching in the changes file.

use std::path::{Path, PathBuf};

use talkbank_model::{BracketedItem, ChatFile, Line, UtteranceContent, Word};

use crate::framework::{TransformCommand, TransformError};

/// A find/replace pair from the changes file.
#[derive(Debug, Clone)]
struct ChangePair {
    find: String,
    replace: String,
}

/// CHSTRING transform: apply string replacements from a changes file.
pub struct ChstringCommand {
    /// Path to the changes file containing find/replace pairs.
    pub changes_path: PathBuf,
}

impl ChstringCommand {
    /// Create a new `ChstringCommand` reading replacements from the given path.
    pub fn new(changes_path: PathBuf) -> Self {
        Self { changes_path }
    }

    /// Parse the changes file into find/replace pairs.
    fn load_changes(path: &Path) -> Result<Vec<ChangePair>, TransformError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| TransformError::Transform(format!("Cannot read changes file: {e}")))?;

        let lines: Vec<&str> = content.lines().collect();
        if !lines.len().is_multiple_of(2) {
            return Err(TransformError::Transform(
                "Changes file must have an even number of lines (find/replace pairs)".to_string(),
            ));
        }

        let pairs = lines
            .chunks(2)
            .map(|pair| ChangePair {
                find: pair[0].to_string(),
                replace: pair[1].to_string(),
            })
            .collect();

        Ok(pairs)
    }
}

impl TransformCommand for ChstringCommand {
    type Config = PathBuf;

    /// Apply all configured string substitutions across main-tier word nodes.
    fn transform(&self, file: &mut ChatFile) -> Result<(), TransformError> {
        let changes = Self::load_changes(&self.changes_path)?;
        if changes.is_empty() {
            return Ok(());
        }

        for line in file.lines.iter_mut() {
            if let Line::Utterance(utterance) = line {
                apply_changes_to_content(&mut utterance.main.content.content, &changes);
            }
        }

        Ok(())
    }
}

/// Apply string replacements to utterance content items.
fn apply_changes_to_content(items: &mut [UtteranceContent], changes: &[ChangePair]) {
    for item in items.iter_mut() {
        match item {
            UtteranceContent::Word(word) => apply_changes_to_word(word, changes),
            UtteranceContent::AnnotatedWord(annotated) => {
                apply_changes_to_word(&mut annotated.inner, changes);
            }
            UtteranceContent::ReplacedWord(replaced) => {
                apply_changes_to_word(&mut replaced.word, changes);
                for rep in replaced.replacement.words.iter_mut() {
                    apply_changes_to_word(rep, changes);
                }
            }
            UtteranceContent::Group(group) => {
                apply_changes_to_bracketed(&mut group.content.content, changes);
            }
            UtteranceContent::AnnotatedGroup(annotated) => {
                apply_changes_to_bracketed(&mut annotated.inner.content.content, changes);
            }
            _ => {}
        }
    }
}

/// Apply string replacements to bracketed items.
fn apply_changes_to_bracketed(items: &mut [BracketedItem], changes: &[ChangePair]) {
    for item in items.iter_mut() {
        match item {
            BracketedItem::Word(word) => apply_changes_to_word(word, changes),
            BracketedItem::AnnotatedWord(annotated) => {
                apply_changes_to_word(&mut annotated.inner, changes);
            }
            BracketedItem::ReplacedWord(replaced) => {
                apply_changes_to_word(&mut replaced.word, changes);
                for rep in replaced.replacement.words.iter_mut() {
                    apply_changes_to_word(rep, changes);
                }
            }
            BracketedItem::AnnotatedGroup(annotated) => {
                apply_changes_to_bracketed(&mut annotated.inner.content.content, changes);
            }
            _ => {}
        }
    }
}

/// Apply string replacements to a single word's text.
fn apply_changes_to_word(word: &mut Word, changes: &[ChangePair]) {
    let raw = word.raw_text().to_owned();
    let mut result = raw.clone();
    for change in changes {
        result = result.replace(&change.find, &change.replace);
    }
    if result != raw {
        word.replace_simple_text(result);
    }
}
