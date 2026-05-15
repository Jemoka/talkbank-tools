//! CHAT parsing helpers for transform-oriented callers.
//!
//! These are thin convenience wrappers over `talkbank-parser` that keep the
//! common Batchalign call pattern available from the canonical shared crate.

use talkbank_model::model::ChatFile;
pub use talkbank_parser::TreeSitterParser;

/// Parse CHAT text leniently (tree-sitter with error recovery).
///
/// Always returns a `ChatFile` (best-effort), plus any parse warnings/errors.
///
/// Lenient parsing intentionally ignores malformed existing `%mor` / `%gra`
/// tiers: their slots stay present in the recovered AST, but their parse
/// diagnostics are not surfaced through this helper. Main-tier and header
/// parse failures still come back in `error_vec`.
pub fn parse_lenient(
    parser: &TreeSitterParser,
    chat_text: &str,
) -> (ChatFile, Vec<talkbank_model::ParseError>) {
    let errors = talkbank_model::ErrorCollector::new();
    let chat_file = parser.parse_chat_file_streaming(chat_text, &errors);
    let error_vec = errors.into_vec();
    let error_vec = error_vec
        .into_iter()
        .filter(|error| {
            generated_tier_at_offset(chat_text, error.location.span.start as usize).is_none()
        })
        .collect();
    (chat_file, error_vec)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum IgnoredGeneratedTierKind {
    Mor,
    Gra,
}

fn generated_tier_at_offset(chat_text: &str, offset: usize) -> Option<IgnoredGeneratedTierKind> {
    let mut byte_offset = 0usize;

    for raw_line in chat_text.split_inclusive('\n') {
        let line_start = byte_offset;
        let line_end = byte_offset + raw_line.len();
        let line = raw_line.trim_end_matches('\n').trim_end_matches('\r');
        let trimmed = line.trim_start();

        if line_start <= offset && offset < line_end {
            return if trimmed.starts_with("%mor") {
                Some(IgnoredGeneratedTierKind::Mor)
            } else if trimmed.starts_with("%gra") {
                Some(IgnoredGeneratedTierKind::Gra)
            } else {
                None
            };
        }

        byte_offset = line_end;
    }

    None
}

/// Parse CHAT text strictly (tree-sitter, no error recovery).
pub fn parse_strict(
    parser: &TreeSitterParser,
    chat_text: &str,
) -> Result<ChatFile, talkbank_model::ParseErrors> {
    parser.parse_chat_file(chat_text)
}

/// Check whether a parsed CHAT file has `@Options: dummy`.
///
/// Dummy files are pass-through placeholders that should not be processed by
/// any NLP pipeline.
pub fn is_dummy(chat_file: &ChatFile) -> bool {
    use talkbank_model::model::ChatOptionFlag;

    chat_file
        .options
        .iter()
        .any(|f| matches!(f, ChatOptionFlag::Unsupported(s) if s == "dummy"))
}

/// Check whether a parsed CHAT file has `@Options: NoAlign`.
///
/// Files with `NoAlign` should skip forced alignment and be output unchanged by
/// alignment-oriented commands.
pub fn is_no_align(chat_file: &ChatFile) -> bool {
    chat_file.options.iter().any(|f| f.skips_alignment())
}

/// Check whether a parsed CHAT file has `@Options: CA`.
///
/// Files with `CA` should skip morphotagging by default.
pub fn is_ca(chat_file: &ChatFile) -> bool {
    chat_file.options.iter().any(|f| f.enables_ca_mode())
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL_CHAT: &str = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n\
@ID:\teng|test|CHI||female|||Target_Child|||\n*CHI:\thello world .\n@End\n";

    const DUMMY_CHAT: &str = "@UTF8\n@Begin\n@Languages:\teng\n@Options:\tdummy\n\
@Participants:\tCHI Target_Child\n@ID:\teng|test|CHI||female|||Target_Child|||\n\
*CHI:\thello world .\n@End\n";

    const NOALIGN_CHAT: &str = "@UTF8\n@Begin\n@Languages:\teng\n@Options:\tNoAlign\n\
@Participants:\tCHI Target_Child\n@ID:\teng|test|CHI||female|||Target_Child|||\n\
*CHI:\thello world .\n@End\n";

    const BULLETS_CHAT: &str = "@UTF8\n@Begin\n@Languages:\teng\n@Options:\tbullets\n\
@Participants:\tCHI Target_Child\n@ID:\teng|test|CHI||female|||Target_Child|||\n\
*CHI:\thello world .\n@End\n";

    #[test]
    fn test_parse_strict_round_trip_basics() {
        let parser = TreeSitterParser::new().expect("tree-sitter parser");
        let chat_file = parse_strict(&parser, MINIMAL_CHAT).expect("strict parse");
        assert_eq!(chat_file.utterances().count(), 1);
    }

    #[test]
    fn test_is_dummy_with_dummy_option() {
        let parser = TreeSitterParser::new().expect("tree-sitter parser");
        let (chat_file, _) = parse_lenient(&parser, DUMMY_CHAT);
        assert!(is_dummy(&chat_file));
    }

    #[test]
    fn test_is_dummy_without_options() {
        let parser = TreeSitterParser::new().expect("tree-sitter parser");
        let (chat_file, _) = parse_lenient(&parser, MINIMAL_CHAT);
        assert!(!is_dummy(&chat_file));
    }

    #[test]
    fn test_is_dummy_with_other_options() {
        let parser = TreeSitterParser::new().expect("tree-sitter parser");
        let (chat_file, _) = parse_lenient(&parser, BULLETS_CHAT);
        assert!(!is_dummy(&chat_file));
    }

    #[test]
    fn test_is_no_align_with_noalign_option() {
        let parser = TreeSitterParser::new().expect("tree-sitter parser");
        let (chat_file, _) = parse_lenient(&parser, NOALIGN_CHAT);
        assert!(is_no_align(&chat_file));
    }

    #[test]
    fn test_is_no_align_without_options() {
        let parser = TreeSitterParser::new().expect("tree-sitter parser");
        let (chat_file, _) = parse_lenient(&parser, MINIMAL_CHAT);
        assert!(!is_no_align(&chat_file));
    }

    #[test]
    fn test_is_no_align_with_dummy_option() {
        let parser = TreeSitterParser::new().expect("tree-sitter parser");
        let (chat_file, _) = parse_lenient(&parser, DUMMY_CHAT);
        assert!(!is_no_align(&chat_file));
    }

    #[test]
    fn test_lenient_parse_ignores_malformed_gra_tier() {
        let parser = TreeSitterParser::new().expect("tree-sitter parser");
        let input = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n\
@ID:\teng|test|CHI||female|||Target_Child|||\n*CHI:\twake up .\n\
%gra:\t1|0|ROOT 2|1|compound:prt 3|1|PUNCT\n%wor:\twake up .\n@End\n";
        let (chat_file, errors) = parse_lenient(&parser, input);
        assert!(
            errors.is_empty(),
            "malformed %gra should be ignored in lenient parse"
        );
        let utterance = chat_file.utterances().next().expect("utterance");
        let tiers: Vec<_> = utterance.dependent_tiers.iter().collect();
        assert!(
            matches!(tiers.first(), Some(talkbank_model::model::DependentTier::Gra(g)) if g.relations().is_empty()),
            "malformed %gra should remain as an empty placeholder in its original slot"
        );
        assert!(
            matches!(
                tiers.get(1),
                Some(talkbank_model::model::DependentTier::Wor(_))
            ),
            "placeholder %gra must remain before existing %wor to avoid tier reordering"
        );
    }

    #[test]
    fn test_lenient_parse_ignores_malformed_mor_tier() {
        let parser = TreeSitterParser::new().expect("tree-sitter parser");
        let input = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n\
@ID:\teng|test|CHI||female|||Target_Child|||\n*CHI:\thello .\n\
%mor:\t|hello .\n@End\n";
        let (chat_file, errors) = parse_lenient(&parser, input);
        assert!(
            errors.is_empty(),
            "malformed %mor should be ignored in lenient parse"
        );
        let utterance = chat_file.utterances().next().expect("utterance");
        assert!(
            matches!(utterance.mor_tier(), Some(m) if m.items().is_empty()),
            "malformed %mor should remain as an empty placeholder tier"
        );
    }

    #[test]
    fn test_lenient_parse_still_reports_main_tier_errors() {
        let parser = TreeSitterParser::new().expect("tree-sitter parser");
        let input = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n\
@ID:\teng|test|CHI||female|||Target_Child|||\n*CHI:\thello @ .\n@End\n";
        let (_chat_file, errors) = parse_lenient(&parser, input);
        assert!(
            !errors.is_empty(),
            "lenient parse must still surface main-tier parse errors"
        );
    }
}
