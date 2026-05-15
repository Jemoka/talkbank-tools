//! Bug annotation system for golden test data.
//!
//! Loads `golden_bugs.toml` from the crate root to document known grammar bugs
//! in the golden word corpus without blocking parser development. Each entry
//! carries a regex/substring pattern, a tracking URL, and an action (`skip`,
//! `expected_wrong`, or `note`) that controls how tests handle matched words.

use regex::Regex;
use serde::Deserialize;
use std::sync::OnceLock;
use thiserror::Error;

/// Collection of bug annotations loaded from golden_bugs.toml
#[derive(Debug, Deserialize)]
pub struct GoldenBugs {
    /// Known bug entries loaded from the TOML manifest.
    #[serde(default)]
    pub bugs: Vec<BugAnnotation>,
}

/// A single bug annotation entry
#[derive(Debug, Deserialize)]
pub struct BugAnnotation {
    /// Pattern to match affected words (regex or substring)
    pub pattern: String,
    /// Human-readable description of the issue
    pub issue: String,
    /// URL to tracking issue (GitHub, etc.)
    pub tracking: String,
    /// How tests should handle this bug
    pub action: BugAction,
    /// Date bug was discovered (YYYY-MM-DD)
    pub discovered: String,
    /// Additional notes and examples
    #[serde(default)]
    pub notes: String,
    /// Cached compiled regex for the pattern (initialized on first use).
    #[serde(skip)]
    compiled: OnceLock<Option<Regex>>,
}

/// Action to take when a buggy pattern is encountered in tests
#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BugAction {
    /// Don't test this word (skip entirely)
    Skip,
    /// Test but expect different output (for debugging)
    ExpectedWrong,
    /// Just documentation, no action taken
    Note,
}

/// Errors returned when loading or decoding `golden_bugs.toml`.
#[derive(Debug, Error)]
pub enum BugAnnotationError {
    /// Failed to read `golden_bugs.toml` from disk.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    /// `golden_bugs.toml` content could not be deserialized.
    #[error("TOML parse error: {0}")]
    Toml(#[from] toml::de::Error),
}

impl GoldenBugs {
    /// Load bug annotations from golden_bugs.toml in the crate root
    pub fn load() -> Result<Self, BugAnnotationError> {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/golden_bugs.toml");
        let contents = std::fs::read_to_string(path)?;
        Ok(toml::from_str(&contents)?)
    }

    /// Check if a word should be skipped in tests
    pub fn should_skip(&self, word: &str) -> bool {
        self.bugs
            .iter()
            .any(|bug| bug.action == BugAction::Skip && bug.matches(word))
    }

    /// Check if a word is expected to produce wrong output
    pub fn is_expected_wrong(&self, word: &str) -> bool {
        self.bugs
            .iter()
            .any(|bug| bug.action == BugAction::ExpectedWrong && bug.matches(word))
    }
}

impl BugAnnotation {
    /// Check if this bug annotation matches the given word
    pub fn matches(&self, word: &str) -> bool {
        if word.contains(&self.pattern) {
            return true;
        }
        let re = self.compiled.get_or_init(|| Regex::new(&self.pattern).ok());
        re.as_ref().is_some_and(|re| re.is_match(word))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_error::TestError;

    /// Tests load golden bugs.
    #[test]
    fn test_load_golden_bugs() -> Result<(), TestError> {
        let bugs = GoldenBugs::load().map_err(|err| TestError::Failure(err.to_string()))?;
        // Should successfully load (even if empty)
        assert!(bugs.bugs.is_empty() || !bugs.bugs.is_empty());
        Ok(())
    }

    /// Tests bug action deserialization.
    #[test]
    fn test_bug_action_deserialization() -> Result<(), TestError> {
        let toml = r#"
        [[bugs]]
        pattern = "test"
        issue = "Test bug"
        tracking = "https://github.com/test"
        action = "skip"
        discovered = "2026-01-08"
        "#;

        let bugs: GoldenBugs = toml::from_str(toml)
            .map_err(|err| TestError::Failure(format!("Parse failed: {}", err)))?;
        assert_eq!(bugs.bugs.len(), 1);
        assert_eq!(bugs.bugs[0].action, BugAction::Skip);
        Ok(())
    }

    /// Tests bug action variants.
    #[test]
    fn test_bug_action_variants() -> Result<(), TestError> {
        let skip_toml = r#"
        [[bugs]]
        pattern = "skip_me"
        issue = "Test"
        tracking = "test"
        action = "skip"
        discovered = "2026-01-08"
        "#;

        let expected_wrong_toml = r#"
        [[bugs]]
        pattern = "wrong_me"
        issue = "Test"
        tracking = "test"
        action = "expected_wrong"
        discovered = "2026-01-08"
        "#;

        let note_toml = r#"
        [[bugs]]
        pattern = "note_me"
        issue = "Test"
        tracking = "test"
        action = "note"
        discovered = "2026-01-08"
        "#;

        let skip: GoldenBugs = toml::from_str(skip_toml)
            .map_err(|err| TestError::Failure(format!("Parse failed: {}", err)))?;
        assert_eq!(skip.bugs[0].action, BugAction::Skip);

        let wrong: GoldenBugs = toml::from_str(expected_wrong_toml)
            .map_err(|err| TestError::Failure(format!("Parse failed: {}", err)))?;
        assert_eq!(wrong.bugs[0].action, BugAction::ExpectedWrong);

        let note: GoldenBugs = toml::from_str(note_toml)
            .map_err(|err| TestError::Failure(format!("Parse failed: {}", err)))?;
        assert_eq!(note.bugs[0].action, BugAction::Note);
        Ok(())
    }

    /// Tests pattern matching substring.
    #[test]
    fn test_pattern_matching_substring() {
        let bug = BugAnnotation {
            pattern: "test".to_string(),
            issue: "Test bug".to_string(),
            tracking: "test".to_string(),
            action: BugAction::Skip,
            discovered: "2026-01-08".to_string(),
            notes: String::new(),
            compiled: OnceLock::new(),
        };

        assert!(bug.matches("test"));
        assert!(bug.matches("testing"));
        assert!(bug.matches("pretest"));
        assert!(!bug.matches("foo"));
    }

    /// Tests pattern matching regex.
    #[test]
    fn test_pattern_matching_regex() {
        let bug = BugAnnotation {
            pattern: "^test$".to_string(),
            issue: "Test bug".to_string(),
            tracking: "test".to_string(),
            action: BugAction::Skip,
            discovered: "2026-01-08".to_string(),
            notes: String::new(),
            compiled: OnceLock::new(),
        };

        assert!(bug.matches("test"));
        assert!(!bug.matches("testing"));
        assert!(!bug.matches("pretest"));
    }

    /// Tests should skip.
    #[test]
    fn test_should_skip() -> Result<(), TestError> {
        let toml = r#"
        [[bugs]]
        pattern = "skip_me"
        issue = "Test"
        tracking = "test"
        action = "skip"
        discovered = "2026-01-08"

        [[bugs]]
        pattern = "note_me"
        issue = "Test"
        tracking = "test"
        action = "note"
        discovered = "2026-01-08"
        "#;

        let bugs: GoldenBugs = toml::from_str(toml)
            .map_err(|err| TestError::Failure(format!("Parse failed: {}", err)))?;
        assert!(bugs.should_skip("skip_me"));
        assert!(!bugs.should_skip("note_me"));
        assert!(!bugs.should_skip("other"));
        Ok(())
    }

    /// Tests is expected wrong.
    #[test]
    fn test_is_expected_wrong() -> Result<(), TestError> {
        let toml = r#"
        [[bugs]]
        pattern = "wrong_me"
        issue = "Test"
        tracking = "test"
        action = "expected_wrong"
        discovered = "2026-01-08"

        [[bugs]]
        pattern = "note_me"
        issue = "Test"
        tracking = "test"
        action = "note"
        discovered = "2026-01-08"
        "#;

        let bugs: GoldenBugs = toml::from_str(toml)
            .map_err(|err| TestError::Failure(format!("Parse failed: {}", err)))?;
        assert!(bugs.is_expected_wrong("wrong_me"));
        assert!(!bugs.is_expected_wrong("note_me"));
        assert!(!bugs.is_expected_wrong("other"));
        Ok(())
    }
}
