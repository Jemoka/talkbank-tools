//! Regression test for E245 / empty-cleaned-text parser panic.
//!
//! Bug: when a word contains only a stress marker (ˈ), the cleaned text
//! becomes empty and `Word::new_unchecked(raw, "")` used to panic inside
//! `NonEmptyString::new_unchecked` via its debug-assert.
//!
//! Expected behavior: the parser must not panic. It must emit E245
//! (`StressNotBeforeSpokenMaterial`) and reject the word.

use talkbank_model::ErrorCollector;
use talkbank_parser::TreeSitterParser;

/// Parsing a word that is only a stress marker must not panic, and the
/// parser must emit E245.
#[test]
fn lone_stress_marker_emits_e245_without_panic() {
    let input = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\tˈ .\n@End\n";

    let parser = TreeSitterParser::new().expect("grammar loads");
    let errors = ErrorCollector::new();
    // This call used to panic inside NonEmptyString::new_unchecked before the fix.
    let _file = parser.parse_chat_file_streaming(input, &errors);

    let collected = errors.into_vec();
    let has_e245 = collected.iter().any(|e| e.code.as_str() == "E245");
    assert!(
        has_e245,
        "Expected E245 for lone stress marker, got: {:#?}",
        collected
            .iter()
            .map(|e| (e.code.as_str(), &e.message))
            .collect::<Vec<_>>()
    );
}
