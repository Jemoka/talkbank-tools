//! Test module for basic in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

use talkbank_model::Header;

use super::helpers::{TestError, parser_suite};

/// Tests parse utf8 header.
#[test]
fn test_parse_utf8_header() -> Result<(), TestError> {
    for parser in parser_suite()? {
        let header = parser.parse_header("@UTF8")?;
        assert_eq!(header.name(), "UTF8", "[{}] expected UTF8", parser.name());
        assert!(
            matches!(header, Header::Utf8),
            "[{}] expected Utf8 variant",
            parser.name()
        );
    }

    Ok(())
}

/// Tests parse begin header.
#[test]
fn test_parse_begin_header() -> Result<(), TestError> {
    for parser in parser_suite()? {
        let header = parser.parse_header("@Begin")?;
        assert_eq!(header.name(), "Begin", "[{}] expected Begin", parser.name());
        assert!(
            matches!(header, Header::Begin),
            "[{}] expected Begin variant",
            parser.name()
        );
    }

    Ok(())
}

/// Tests parse end header.
#[test]
fn test_parse_end_header() -> Result<(), TestError> {
    for parser in parser_suite()? {
        let header = parser.parse_header("@End")?;
        assert_eq!(header.name(), "End", "[{}] expected End", parser.name());
        assert!(
            matches!(header, Header::End),
            "[{}] expected End variant",
            parser.name()
        );
    }

    Ok(())
}

/// Tests parse new episode header.
#[test]
fn test_parse_new_episode_header() -> Result<(), TestError> {
    for parser in parser_suite()? {
        let header = parser.parse_header("@New Episode")?;
        assert_eq!(
            header.name(),
            "New Episode",
            "[{}] expected New Episode",
            parser.name()
        );
        assert!(
            matches!(header, Header::NewEpisode),
            "[{}] expected NewEpisode variant",
            parser.name()
        );
    }

    Ok(())
}
