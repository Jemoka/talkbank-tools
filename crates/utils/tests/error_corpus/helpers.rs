//! Test module for helpers in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

use crate::test_utils::parser_suite::{
    ParserImpl, ParserSuiteError, parser_suite as shared_parser_suite,
};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use talkbank_utils::test_error::TestError;

use serde::Deserialize;
use talkbank_model::ErrorCollector;

/// Type representing what the error corpus file is expected to trigger.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExpectedKind {
    Error,
    Warning,
}

/// Expected outcome specification for a single parser.
#[derive(Clone, Debug, Deserialize)]
pub struct ExpectedOutcome {
    kind: ExpectedKind,
    codes: Vec<String>,
}

impl ExpectedOutcome {
    /// Runs codes.
    pub(crate) fn codes(&self) -> &[String] {
        &self.codes
    }

    /// Runs description.
    pub(crate) fn description(&self) -> String {
        /// Formats code list.
        fn format_code_list(codes: &[String]) -> String {
            if codes.len() == 1 {
                codes[0].clone()
            } else {
                format!("[{}]", codes.join(", "))
            }
        }

        let kind = match self.kind {
            ExpectedKind::Error => "error",
            ExpectedKind::Warning => "warning",
        };
        if self.codes.is_empty() {
            format!("no {}s", kind)
        } else {
            format!("{} {}", kind, format_code_list(&self.codes))
        }
    }

    /// Runs matches codes.
    pub(crate) fn matches_codes(&self, actual_codes: &[String]) -> bool {
        if self.codes.is_empty() {
            actual_codes.is_empty()
        } else {
            self.codes.iter().any(|code| actual_codes.contains(code))
        }
    }
}

/// Holds parser-specific expectations with an optional default fallback.
#[derive(Clone, Debug, Deserialize)]
pub struct ExpectedOutcomes {
    tree_sitter: ExpectedOutcome,
    /// Legacy field kept for JSON compatibility; ignored at runtime.
    #[serde(default)]
    #[allow(dead_code)]
    direct: Option<ExpectedOutcome>,
    #[serde(default)]
    #[allow(dead_code)]
    divergence_note: Option<String>,
}

impl ExpectedOutcomes {
    /// Returns the expected outcome for the parser.
    pub fn for_parser(&self, parser_name: &str) -> Option<&ExpectedOutcome> {
        match parser_name {
            "tree-sitter" => Some(&self.tree_sitter),
            _ => None,
        }
    }
}

/// Error corpus expectations manifest.
#[derive(Debug, Deserialize)]
pub struct ExpectationsManifest {
    #[allow(dead_code)]
    pub version: u32,
    pub files: BTreeMap<String, ExpectedOutcomes>,
}

impl ParserImpl {
    /// Collects all errors.
    pub fn collect_all_errors(&self, content: &str) -> Vec<talkbank_model::ParseError> {
        let parse_errors = ErrorCollector::new();

        let chat_file_opt = self
            .0
            .parse_chat_file_fragment(content, 0, &parse_errors)
            .into_option();

        if let Some(mut chat_file) = chat_file_opt {
            // TEMPORARY: skip %wor alignment - semantics still being worked out
            chat_file.validate_with_alignment(&parse_errors, None);
        }

        parse_errors.into_vec()
    }
}

/// Returns both parser implementations for testing
pub fn parser_suite() -> Result<Vec<ParserImpl>, TestError> {
    shared_parser_suite().map_err(map_parser_suite_error)
}

fn map_parser_suite_error(error: ParserSuiteError) -> TestError {
    match error {
        ParserSuiteError::TreeSitterInit { source } => TestError::Failure(format!(
            "Failed to create TreeSitterParser for error corpus: {}",
            source
        )),
    }
}

/// Path to the expectations manifest.
pub fn expectations_manifest_path() -> PathBuf {
    PathBuf::from("tests/error_corpus/expectations.json")
}

/// Load error corpus expectations from the manifest.
pub fn load_expectations_manifest() -> Result<ExpectationsManifest, TestError> {
    let manifest_path = expectations_manifest_path();
    let content = fs::read_to_string(&manifest_path).map_err(|err| {
        TestError::Failure(format!(
            "Failed to read {}: {}",
            manifest_path.display(),
            err
        ))
    })?;
    serde_json::from_str(&content).map_err(|err| {
        TestError::Failure(format!("Failed to parse expectations manifest: {}", err))
    })
}

/// Convert an error corpus file path into a manifest key.
pub fn error_corpus_relative_path(path: &Path) -> Result<String, TestError> {
    let corpus_dir = Path::new("tests/error_corpus");
    let relative = relative_to(path, corpus_dir)?;
    let parts: Vec<String> = relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy().to_string())
        .collect();
    Ok(parts.join("/"))
}

/// Discovers all .cha files in error_corpus directory structure.
pub fn discover_error_files() -> Result<Vec<PathBuf>, TestError> {
    let corpus_dir = Path::new("tests/error_corpus");
    let mut files = Vec::new();

    if !corpus_dir.exists() {
        return Ok(files);
    }

    /// Runs visit dirs.
    fn visit_dirs(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), TestError> {
        if dir.is_dir() {
            let entries = fs::read_dir(dir).map_err(TestError::Io)?;
            for entry in entries {
                let entry = entry.map_err(TestError::Io)?;
                let path = entry.path();
                if path.is_dir() {
                    visit_dirs(&path, files)?;
                } else if path.extension().and_then(|s| s.to_str()) == Some("cha") {
                    files.push(path);
                }
            }
        }
        Ok(())
    }

    visit_dirs(corpus_dir, &mut files)?;
    files.sort();
    Ok(files)
}

/// Runs relative to.
fn relative_to(path: &Path, base: &Path) -> Result<PathBuf, TestError> {
    let mut path_components = path.components();
    for base_component in base.components() {
        match path_components.next() {
            Some(component) if component == base_component => {}
            _ => {
                return Err(TestError::Failure(format!(
                    "Failed to relativize {}: not under {}",
                    path.display(),
                    base.display()
                )));
            }
        }
    }

    let mut relative = PathBuf::new();
    for component in path_components {
        relative.push(component.as_os_str());
    }
    Ok(relative)
}
