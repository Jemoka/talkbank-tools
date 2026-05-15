//! Legacy main-tier and utterance fragment parity tests against the reference corpus.
//!
//! These tests ensure `parse_main_tier()` and `parse_utterance()` match the same
//! parser's whole-file AST on real utterances drawn from the sacred reference corpus.
//!
//! This is still useful for auditing synthetic fragment behavior, but it should
//! not be treated as the long-term oracle for direct-parser fragment semantics.

use std::fs;
use std::path::PathBuf;

use talkbank_model::ChatOptionFlag;
use talkbank_model::ErrorCollector;
use talkbank_model::model::{Line, SemanticEq};
use talkbank_model::{FragmentSemanticContext, ParseOutcome};
use talkbank_parser_tests::test_error::TestError;

use super::parser_impl::parser_suite;

fn reference_root() -> PathBuf {
    let mut full = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    full.push("../../resources/corpus/reference");
    full
}

fn reference_file(path: &str) -> PathBuf {
    let mut full = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    full.push(path);
    full
}

fn collect_reference_files() -> Result<Vec<PathBuf>, TestError> {
    let mut files = Vec::new();
    for entry in walkdir::WalkDir::new(reference_root()) {
        let entry = entry.map_err(|err| TestError::Failure(format!("walkdir failure: {err}")))?;
        if entry.file_type().is_file() && entry.path().extension().is_some_and(|ext| ext == "cha") {
            files.push(entry.into_path());
        }
    }
    files.sort();
    Ok(files)
}

fn relative_display(path: &std::path::Path) -> String {
    let root = reference_root();
    path.strip_prefix(&root)
        .unwrap_or(path)
        .display()
        .to_string()
}

fn source_uses_file_context_options(source: &str) -> bool {
    source.lines().any(|line| line.starts_with("@Options:\t"))
}

#[test]
fn reference_main_tiers_roundtrip_for_every_parser() -> Result<(), TestError> {
    let files = collect_reference_files()?;

    for parser in parser_suite()? {
        for path in &files {
            let source = fs::read_to_string(path).map_err(|err| {
                TestError::Failure(format!("failed to read {}: {err}", path.display()))
            })?;

            // `parse_main_tier()` is a context-free fragment API. Files with
            // `@Options:` can change whole-file main-tier semantics (for example
            // CA omission normalization), so they are audited separately rather
            // than being treated as parity failures here.
            if source_uses_file_context_options(&source) {
                continue;
            }

            let file_errors = ErrorCollector::new();
            let parsed_file = match parser.parse_chat_file_fragment(&source, 0, &file_errors) {
                ParseOutcome::Parsed(file) => file,
                ParseOutcome::Rejected => {
                    return Err(TestError::Failure(format!(
                        "[{}] rejected whole-file parse for {}",
                        "tree-sitter",
                        path.display()
                    )));
                }
            };

            if !file_errors.is_empty() {
                return Err(TestError::Failure(format!(
                    "[{}] whole-file parse errors for {}: {:?}",
                    "tree-sitter",
                    path.display(),
                    file_errors.to_vec()
                )));
            }

            for utterance in parsed_file.utterances() {
                let main_line = utterance.main.to_chat();
                let main_errors = ErrorCollector::new();
                let reparsed = match parser.parse_main_tier_fragment(&main_line, 0, &main_errors) {
                    ParseOutcome::Parsed(main) => main,
                    ParseOutcome::Rejected => {
                        return Err(TestError::Failure(format!(
                            "[{}] parse_main_tier rejected `{}` from {}",
                            "tree-sitter",
                            main_line,
                            relative_display(path)
                        )));
                    }
                };

                if !main_errors.is_empty() {
                    return Err(TestError::Failure(format!(
                        "[{}] parse_main_tier errors for `{}` from {}: {:?}",
                        "tree-sitter",
                        main_line,
                        relative_display(path),
                        main_errors.to_vec()
                    )));
                }

                if !utterance.main.semantic_eq(&reparsed) {
                    return Err(TestError::Failure(format!(
                        "[{}] parse_main_tier semantic mismatch for `{}` from {}",
                        "tree-sitter",
                        main_line,
                        relative_display(path)
                    )));
                }
            }
        }
    }

    Ok(())
}

#[test]
fn reference_utterances_roundtrip_for_every_parser() -> Result<(), TestError> {
    let files = [
        "../../resources/corpus/reference/core/basic-conversation.cha",
        "../../resources/corpus/reference/core/headers-comments.cha",
        "../../resources/corpus/reference/content/media-bullets.cha",
        "../../resources/corpus/reference/tiers/multi-tier-utterance.cha",
        "../../resources/corpus/reference/tiers/mor-gra.cha",
    ];

    for parser in parser_suite()? {
        for relative_path in files {
            let path = reference_file(relative_path);
            let source = fs::read_to_string(&path).map_err(|err| {
                TestError::Failure(format!("failed to read {}: {err}", path.display()))
            })?;

            let file_errors = ErrorCollector::new();
            let parsed_file = match parser.parse_chat_file_fragment(&source, 0, &file_errors) {
                ParseOutcome::Parsed(file) => file,
                ParseOutcome::Rejected => {
                    return Err(TestError::Failure(format!(
                        "[{}] rejected whole-file parse for {}",
                        "tree-sitter",
                        path.display()
                    )));
                }
            };

            if !file_errors.is_empty() {
                return Err(TestError::Failure(format!(
                    "[{}] whole-file parse errors for {}: {:?}",
                    "tree-sitter",
                    path.display(),
                    file_errors.to_vec()
                )));
            }

            for line in &parsed_file.lines {
                let Line::Utterance(utterance) = line else {
                    continue;
                };

                let utterance_text = utterance.to_chat();
                let utterance_errors = ErrorCollector::new();
                let reparsed =
                    match parser.parse_utterance_fragment(&utterance_text, 0, &utterance_errors) {
                        ParseOutcome::Parsed(utterance) => utterance,
                        ParseOutcome::Rejected => {
                            return Err(TestError::Failure(format!(
                                "[{}] parse_utterance rejected `{}` from {}",
                                "tree-sitter",
                                utterance_text,
                                path.display()
                            )));
                        }
                    };

                if !utterance_errors.is_empty() {
                    return Err(TestError::Failure(format!(
                        "[{}] parse_utterance errors for `{}` from {}: {:?}",
                        "tree-sitter",
                        utterance_text,
                        path.display(),
                        utterance_errors.to_vec()
                    )));
                }

                if !utterance.as_ref().semantic_eq(&reparsed) {
                    return Err(TestError::Failure(format!(
                        "[{}] parse_utterance semantic mismatch for `{}` from {}",
                        "tree-sitter",
                        utterance_text,
                        path.display()
                    )));
                }
            }
        }
    }

    Ok(())
}

#[test]
fn main_tier_ca_omission_requires_file_context_for_every_parser() -> Result<(), TestError> {
    let input = "*CHI:\t(word) .";

    for parser in parser_suite()? {
        let errors = ErrorCollector::new();
        let parsed = parser.parse_main_tier_fragment(input, 0, &errors);

        if !parsed.is_rejected() {
            return Err(TestError::Failure(format!(
                "[{}] parse_main_tier unexpectedly accepted CA-omission fragment without file context",
                "tree-sitter"
            )));
        }

        if errors.is_empty() {
            return Err(TestError::Failure(format!(
                "[{}] parse_main_tier rejected CA-omission fragment without reporting an error",
                "tree-sitter"
            )));
        }
    }

    Ok(())
}

#[test]
fn main_tier_ca_omission_parses_with_ca_context_for_every_parser() -> Result<(), TestError> {
    let input = "*CHI:\t(word) .";
    let context = FragmentSemanticContext::new().with_option_flag(ChatOptionFlag::Ca);

    for parser in parser_suite()? {
        let errors = ErrorCollector::new();
        let parsed = match parser.parse_main_tier_fragment_with_context(input, 0, &context, &errors)
        {
            ParseOutcome::Parsed(main) => main,
            ParseOutcome::Rejected => {
                return Err(TestError::Failure(format!(
                    "[{}] parse_main_tier_with_context rejected CA-omission fragment with CA context",
                    "tree-sitter"
                )));
            }
        };

        if !errors.is_empty() {
            return Err(TestError::Failure(format!(
                "[{}] parse_main_tier_with_context reported errors with CA context: {:?}",
                "tree-sitter",
                errors.to_vec()
            )));
        }

        let serialized = parsed.to_chat();
        if serialized != input {
            return Err(TestError::Failure(format!(
                "[{}] parse_main_tier_with_context roundtrip mismatch: expected `{}`, got `{}`",
                "tree-sitter", input, serialized
            )));
        }
    }

    Ok(())
}
