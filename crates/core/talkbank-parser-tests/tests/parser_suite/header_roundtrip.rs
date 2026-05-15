//! Header fragment parity tests against the reference corpus.
//!
//! These tests ensure `parse_header()` matches the same parser's whole-file AST
//! on real headers drawn from the sacred reference corpus, including interstitial
//! `@Comment` headers attached to utterances.

use std::fs;
use std::path::PathBuf;

use talkbank_model::ErrorCollector;
use talkbank_model::ParseOutcome;
use talkbank_model::model::{SemanticEq, WriteChat};
use talkbank_model::{Header, Line};
use talkbank_parser_tests::test_error::TestError;

use super::parser_impl::parser_suite;

fn reference_file(path: &str) -> PathBuf {
    let mut full = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    full.push(path);
    full
}

fn collect_headers(file: &talkbank_model::ChatFile) -> Vec<Header> {
    let mut headers = Vec::new();
    for line in &file.lines {
        match line {
            Line::Header { header, .. } => headers.push((**header).clone()),
            Line::Utterance(utterance) => {
                headers.extend(utterance.preceding_headers.iter().cloned());
            }
        }
    }
    headers
}

#[test]
fn reference_headers_roundtrip_for_every_parser() -> Result<(), TestError> {
    let files = [
        "../../resources/corpus/reference/core/headers-comments.cha",
        "../../resources/corpus/reference/core/headers-media.cha",
        "../../resources/corpus/reference/core/headers-speaker-info.cha",
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

            for header in collect_headers(&parsed_file) {
                let header_text = header.to_chat();
                let header_errors = ErrorCollector::new();
                let reparsed = match parser.parse_header_fragment(&header_text, 0, &header_errors) {
                    ParseOutcome::Parsed(header) => header,
                    ParseOutcome::Rejected => {
                        return Err(TestError::Failure(format!(
                            "[{}] parse_header rejected `{}` from {}",
                            "tree-sitter",
                            header_text,
                            path.display()
                        )));
                    }
                };

                if !header_errors.is_empty() {
                    return Err(TestError::Failure(format!(
                        "[{}] parse_header errors for `{}` from {}: {:?}",
                        "tree-sitter",
                        header_text,
                        path.display(),
                        header_errors.to_vec()
                    )));
                }

                if !header.semantic_eq(&reparsed) {
                    return Err(TestError::Failure(format!(
                        "[{}] parse_header semantic mismatch for `{}` from {}",
                        "tree-sitter",
                        header_text,
                        path.display()
                    )));
                }
            }
        }
    }

    Ok(())
}
