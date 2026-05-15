//! Test module for real file in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

use super::helpers::{TestError, parser_suite};
use talkbank_model::ChatFile;
use talkbank_model::ErrorCollector;

/// Parses chat file or err.
fn parse_chat_file_or_err(
    parser: &super::helpers::ParserImpl,
    input: &str,
) -> Result<ChatFile, TestError> {
    let errors = ErrorCollector::new();
    let chat_file = parser
        .parse_chat_file_streaming(input, &errors)
        .ok_or_else(|| TestError::ParseErrors {
            parser: parser.name(),
            errors: talkbank_model::ParseErrors::new(),
        })?;

    let error_vec = errors.into_vec();
    if error_vec.is_empty() {
        Ok(chat_file)
    } else {
        Err(TestError::ParseErrors {
            parser: parser.name(),
            errors: talkbank_model::ParseErrors { errors: error_vec },
        })
    }
}

/// Tests parse real chat file participants.
#[test]
fn test_parse_real_chat_file_participants() -> Result<(), TestError> {
    // Resolve test file relative to workspace root; skips gracefully when
    // the java-chatter-stable repo is not cloned as a sibling.
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let workspace_root = std::path::Path::new(manifest_dir);
    let file_path = workspace_root
        .parent() // talkbank-tools/
        .unwrap_or(workspace_root)
        .parent() // talkbank/ (workspace)
        .map(|p| p.join("java-chatter-stable/testchat/good/10-03.cha"))
        .unwrap_or_else(|| std::path::PathBuf::from("nonexistent"));
    if !file_path.exists() {
        eprintln!("Skipping test - file not found: {}", file_path.display());
        return Ok(());
    }
    let file_path = file_path.to_string_lossy().to_string();

    let content = std::fs::read_to_string(&file_path).map_err(|source| TestError::ReadFile {
        path: file_path.clone(),
        source,
    })?;

    // Test BOTH parsers
    for parser in parser_suite()? {
        let chat_file = parse_chat_file_or_err(&parser, &content)?;

        assert!(
            chat_file.participant_count() > 0,
            "[{}] Real file should have participants",
            parser.name()
        );

        for participant in chat_file.all_participants() {
            assert!(
                !participant.code.is_empty(),
                "[{}] Participant code should not be empty",
                parser.name()
            );
            assert!(
                !participant.role.is_empty(),
                "[{}] Participant role should not be empty",
                parser.name()
            );
            assert!(
                !participant.id.language.is_empty(),
                "[{}] Participant language should not be empty",
                parser.name()
            );

            eprintln!(
                "[{}] Participant {}: role={}, languages={:?}, age={:?}",
                parser.name(),
                participant.code,
                participant.role,
                participant
                    .languages()
                    .0
                    .iter()
                    .map(|c| c.as_str())
                    .collect::<Vec<_>>(),
                participant.age()
            );
        }
    }

    Ok(())
}
