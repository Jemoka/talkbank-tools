//! Create new minimal valid CHAT files.
//!
//! Generates a transcript containing exactly the required headers (`@UTF8`, `@Begin`,
//! `@End`, `@Languages`, `@Participants`, `@ID`) so the output is immediately parseable
//! by `chatter validate`. An optional utterance line can be included as a starting point.
//!
//! File generation delegates to [`MinimalChatFile`](talkbank_parser_tests::MinimalChatFile)
//! from the parser-tests crate, ensuring the template stays in sync with the parser's own
//! test fixtures.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>

use std::fs;
use std::io::{self, Write};
use std::path::Path;
use talkbank_parser_tests::MinimalChatFile;

/// Create a minimal valid CHAT file that conforms to the File Format and Header sections of the manual.
///
/// The generated transcript always contains the required `@UTF8`, `@Begin`, `@End`, `@Languages`, `@Participants`, and `@ID` headers.
/// These headers establish the file encoding, participant list, and utterance metadata that the manual describes before any tiers are parsed.
///
/// Generates a valid CHAT file with:
/// - Required `@UTF8` header
/// - Required `@Begin` and `@End` markers
/// - Required `@Languages`, `@Participants`, and `@ID` headers
/// - Optional utterance line
///
/// # Arguments
///
/// * `output` - Output file path (None = print to stdout)
/// * `speaker` - Speaker code (e.g., "CHI", "MOT")
/// * `language` - ISO 639-3 language code (e.g., "eng", "spa")
/// * `role` - Participant role (e.g., "Target_Child", "Mother")
/// * `corpus` - Corpus identifier (e.g., "corpus", "mydata")
/// * `utterance` - Optional utterance content
///
/// When an utterance is provided, it becomes the first utterance line (Main Tier) in the file so callers can start
/// from a ready-to-parse transcript. The generated structure follows the CHAT manual’s canonical ordering so downstream
/// validation/alignment tools see predictable headers.
pub fn create_new_file(
    output: Option<&Path>,
    speaker: &str,
    language: &str,
    role: &str,
    corpus: &str,
    utterance: Option<&str>,
) {
    // Build the CHAT file content
    let mut builder = MinimalChatFile::new()
        .speaker(speaker)
        .language(language)
        .role(role)
        .corpus(corpus);

    if let Some(utterance) = utterance {
        builder = builder.utterance(utterance);
    }

    let content = builder.to_string();

    // Write to output
    match output {
        Some(path) => {
            if let Err(e) = fs::write(path, &content) {
                eprintln!("Error writing file: {}", e);
                std::process::exit(1);
            }
            eprintln!("✓ Created {}", path.display());
        }
        None => {
            // Print to stdout
            if let Err(e) = io::stdout().write_all(content.as_bytes()) {
                eprintln!("Error writing to stdout: {}", e);
                std::process::exit(1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use talkbank_model::ParseErrors;
    use talkbank_model::model::{
        ChatFile, CorpusName, Header, LanguageCode, Line, ParticipantRole, SpeakerCode,
    };
    use talkbank_parser::TreeSitterParser;
    use tempfile::tempdir;
    use thiserror::Error;

    /// Error cases used by `new_file` integration-style tests.
    #[derive(Debug, Error)]
    enum TestError {
        #[error("Tempdir error")]
        TempDir { source: std::io::Error },
        #[error("IO error")]
        Io { source: std::io::Error },
        #[error("Parse error")]
        Parse { source: ParseErrors },
        #[error("Missing header: {header:?}")]
        MissingHeader { header: RequiredHeader },
        #[error("Missing participant entry")]
        MissingParticipant,
        #[error("Missing ID header")]
        MissingIdHeader,
        #[error("Missing utterance")]
        MissingUtterance,
    }

    /// Required headers that must appear in a minimal valid CHAT file.
    #[derive(Debug, Clone, Copy)]
    enum RequiredHeader {
        Utf8,
        Begin,
        End,
        Languages,
        Participants,
        Id,
    }

    /// Creates a minimal file and checks that all required headers are present.
    #[test]
    fn creates_valid_file_with_defaults() -> Result<(), TestError> {
        let dir = tempdir().map_err(|source| TestError::TempDir { source })?;
        let path = dir.path().join("test.cha");

        create_new_file(Some(&path), "CHI", "eng", "Target_Child", "corpus", None);

        let chat_file = parse_file(&path)?;
        require_header(&chat_file, RequiredHeader::Utf8)?;
        require_header(&chat_file, RequiredHeader::Begin)?;
        require_header(&chat_file, RequiredHeader::Languages)?;
        require_header(&chat_file, RequiredHeader::Participants)?;
        require_header(&chat_file, RequiredHeader::Id)?;
        require_header(&chat_file, RequiredHeader::End)?;

        require_language(&chat_file, LanguageCode::new("eng"))?;
        require_participant(&chat_file, "CHI", "Target_Child")?;
        require_id_header(&chat_file, "eng", "corpus", "CHI", "Target_Child")?;
        Ok(())
    }

    /// Creates a file with caller-supplied metadata and verifies round-trip parse values.
    #[test]
    fn creates_file_with_custom_params() -> Result<(), TestError> {
        let dir = tempdir().map_err(|source| TestError::TempDir { source })?;
        let path = dir.path().join("test.cha");

        create_new_file(
            Some(&path),
            "MOT",
            "spa",
            "Mother",
            "mydata",
            Some("hola mundo ."),
        );

        let chat_file = parse_file(&path)?;
        require_language(&chat_file, LanguageCode::new("spa"))?;
        require_participant(&chat_file, "MOT", "Mother")?;
        require_id_header(&chat_file, "spa", "mydata", "MOT", "Mother")?;
        require_utterance(&chat_file, "MOT", "hola mundo .")?;
        Ok(())
    }

    /// Parses file.
    fn parse_file(path: &Path) -> Result<ChatFile, TestError> {
        let content = fs::read_to_string(path).map_err(|source| TestError::Io { source })?;
        let parser = TreeSitterParser::new().expect("grammar loads");
        parser
            .parse_chat_file(&content)
            .map_err(|source| TestError::Parse { source })
    }

    /// Assert that a required header appears in the parsed file.
    fn require_header(chat_file: &ChatFile, required: RequiredHeader) -> Result<(), TestError> {
        let found = chat_file.lines.iter().any(|line| {
            let Line::Header { header, .. } = line else {
                return false;
            };
            matches!(
                (required, header.as_ref()),
                (RequiredHeader::Utf8, Header::Utf8)
                    | (RequiredHeader::Begin, Header::Begin)
                    | (RequiredHeader::End, Header::End)
                    | (RequiredHeader::Languages, Header::Languages { .. })
                    | (RequiredHeader::Participants, Header::Participants { .. })
                    | (RequiredHeader::Id, Header::ID(_))
            )
        });

        if found {
            Ok(())
        } else {
            Err(TestError::MissingHeader { header: required })
        }
    }

    /// Assert that the `` header includes the expected code.
    fn require_language(chat_file: &ChatFile, expected: LanguageCode) -> Result<(), TestError> {
        let found = chat_file.lines.iter().any(|line| match line {
            Line::Header { header, .. } => {
                matches!(header.as_ref(), Header::Languages { codes } if codes.contains(&expected))
            }
            _ => false,
        });

        if found {
            Ok(())
        } else {
            Err(TestError::MissingHeader {
                header: RequiredHeader::Languages,
            })
        }
    }

    /// Assert that `` includes the expected speaker and role.
    fn require_participant(
        chat_file: &ChatFile,
        speaker: &str,
        role: &str,
    ) -> Result<(), TestError> {
        let expected_speaker = SpeakerCode::new(speaker);
        let expected_role = ParticipantRole::new(role);
        let found = chat_file.lines.iter().any(|line| match line {
            Line::Header { header, .. } => match header.as_ref() {
                Header::Participants { entries } => entries.iter().any(|entry| {
                    entry.speaker_code == expected_speaker && entry.role == expected_role
                }),
                _ => false,
            },
            _ => false,
        });

        if found {
            Ok(())
        } else {
            Err(TestError::MissingParticipant)
        }
    }

    /// Assert that an `` line matches the expected participant metadata.
    fn require_id_header(
        chat_file: &ChatFile,
        language: &str,
        corpus: &str,
        speaker: &str,
        role: &str,
    ) -> Result<(), TestError> {
        let expected_language = LanguageCode::new(language);
        let expected_corpus = CorpusName::new(corpus);
        let expected_speaker = SpeakerCode::new(speaker);
        let expected_role = ParticipantRole::new(role);

        let found = chat_file.lines.iter().any(|line| match line {
            Line::Header { header, .. } => match header.as_ref() {
                Header::ID(id) => {
                    id.language.contains(&expected_language)
                        && id.corpus.as_ref() == Some(&expected_corpus)
                        && id.speaker == expected_speaker
                        && id.role == expected_role
                }
                _ => false,
            },
            _ => false,
        });

        if found {
            Ok(())
        } else {
            Err(TestError::MissingIdHeader)
        }
    }

    /// Assert that the generated file contains the expected first utterance.
    fn require_utterance(
        chat_file: &ChatFile,
        speaker: &str,
        content: &str,
    ) -> Result<(), TestError> {
        let expected_speaker = SpeakerCode::new(speaker);
        let found = chat_file.lines.iter().any(|line| match line {
            Line::Utterance(utterance) => {
                utterance.main.speaker == expected_speaker
                    && utterance.main.content.to_content_string() == content
            }
            _ => false,
        });

        if found {
            Ok(())
        } else {
            Err(TestError::MissingUtterance)
        }
    }
}
