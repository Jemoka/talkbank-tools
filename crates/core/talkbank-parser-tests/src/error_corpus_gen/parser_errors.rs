//! E3xx: Parser Errors
//!
//! Generates parse error corpus files for parser-level errors (E3xx range).
//! These are parse errors — tree-sitter rejects the input.

use std::fs;
use std::path::Path;

use crate::ChatFileBuilder;

use super::{GenResult, write_file};

//
// E3xx: Parser Errors (40 missing)
//

/// Generates e3xx parser errors.
pub fn generate_e3xx_parser_errors(root: &Path) -> GenResult {
    let dir = root.join("parse_errors");
    fs::create_dir_all(&dir)?;

    let mut count = 0;

    // E312: UnclosedBracket
    write_file(
        &dir.join("E312_unclosed_bracket.cha"),
        "@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n@ID:\teng|corpus|CHI|||||Child|||\n\
         @Comment:\tERROR: Bracket must be closed\n\
         @Comment:\tInvalid: 'hello [: world' - Missing ]\n\
         *CHI:\thello [: world .\n@End\n"
            .to_string(),
    )?;
    count += 1;

    // E313: UnclosedParenthesis
    write_file(
        &dir.join("E313_unclosed_parenthesis.cha"),
        "@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n@ID:\teng|corpus|CHI|||||Child|||\n\
         @Comment:\tERROR: Parenthesis must be closed\n\
         @Comment:\tInvalid: 'hello (world' - Missing )\n\
         *CHI:\thello (world .\n@End\n"
            .to_string(),
    )?;
    count += 1;

    // E314: IncompleteAnnotation
    write_file(
        &dir.join("E314_incomplete_annotation.cha"),
        "@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n@ID:\teng|corpus|CHI|||||Child|||\n\
         @Comment:\tERROR: Annotation bracket incomplete\n\
         @Comment:\tInvalid: '[' - Bare bracket without content\n\
         *CHI:\t[ .\n@End\n"
            .to_string(),
    )?;
    count += 1;

    // E315: InvalidControlCharacter
    write_file(
        &dir.join("E315_invalid_control_char.cha"),
        "@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n@ID:\teng|corpus|CHI|||||Child|||\n\
         @Comment:\tERROR: Invalid control character in unexpected location\n\
         @Comment:\tInvalid: Control char in word\n\
         *CHI:\thello\x01world .\n@End\n"
            .to_string(),
    )?;
    count += 1;

    // E323: MissingColonAfterSpeaker
    write_file(
        &dir.join("E323_missing_colon.cha"),
        "@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n@ID:\teng|corpus|CHI|||||Child|||\n\
         @Comment:\tERROR: Speaker must be followed by colon\n\
         @Comment:\tInvalid: '*CHI hello' - Missing colon\n\
         *CHI hello .\n@End\n"
            .to_string(),
    )?;
    count += 1;

    // E302: MissingNode
    write_file(
        &dir.join("E302_missing_node.cha"),
        "@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n@ID:\teng|corpus|CHI|||||Child|||\n\
         @Comment:\tExpected error: E302 (Missing required node)\n\
         @Comment:\tTrigger: Speaker code format invalid\n\
         *ch:\thello .\n@End\n"
            .to_string(),
    )?;
    count += 1;

    // E309: UnexpectedSyntax
    write_file(
        &dir.join("E309_unexpected_syntax.cha"),
        "@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n@ID:\teng|corpus|CHI|||||Child|||\n\
         @Comment:\tExpected error: E309 (Unexpected syntax)\n\
         @Comment:\tTrigger: Unexpected characters in utterance context\n\
         *CHI:\thello ## world .\n@End\n"
            .to_string(),
    )?;
    count += 1;

    // E344: InvalidContentAnnotationNesting
    write_file(
        &dir.join("E344_invalid_scoped_nesting.cha"),
        ChatFileBuilder::new()
            .speaker("CHI", "Target_Child")
            .custom_header("@Comment:\tExpected error: E344 (Invalid scoped annotation nesting)")
            .custom_header("@Comment:\tTrigger: Overlapping scoped annotations of same type")
            .utterance("CHI", "hello <world <foo> bar> .")
            .build(),
    )?;
    count += 1;

    // E346: UnmatchedContentAnnotationEnd
    write_file(
        &dir.join("E346_unmatched_scoped_end.cha"),
        ChatFileBuilder::new()
            .speaker("CHI", "Target_Child")
            .custom_header("@Comment:\tExpected error: E346 (Unmatched scoped annotation end)")
            .custom_header("@Comment:\tTrigger: Closing > without matching <")
            .utterance("CHI", "hello world> [/] .")
            .build(),
    )?;
    count += 1;

    // E348: MissingOverlapEnd
    write_file(
        &dir.join("E348_missing_overlap_end.cha"),
        ChatFileBuilder::new()
            .speaker("CHI", "Target_Child")
            .speaker("MOT", "Mother")
            .custom_header("@Comment:\tExpected error: E348 (Missing overlap end)")
            .custom_header("@Comment:\tTrigger: Overlap begin marker without matching end")
            .utterance("CHI", "hello \u{2308} world .")
            .utterance("MOT", "yes .")
            .build(),
    )?;
    count += 1;

    // E366: LongFeatureLabelMismatch
    write_file(
        &dir.join("E366_longfeature_label_mismatch.cha"),
        ChatFileBuilder::new()
            .speaker("CHI", "Target_Child")
            .custom_header("@Comment:\tExpected error: E366 (Long feature label mismatch)")
            .custom_header("@Comment:\tTrigger: Long feature begin/end labels don't match")
            .utterance(
                "CHI",
                "hello \u{2308}label1\u{2309} world \u{230a}label2\u{230b} .",
            )
            .build(),
    )?;
    count += 1;

    // E369: NonvocalLabelMismatch
    write_file(
        &dir.join("E369_nonvocal_label_mismatch.cha"),
        ChatFileBuilder::new()
            .speaker("CHI", "Target_Child")
            .custom_header("@Comment:\tExpected error: E369 (Nonvocal label mismatch)")
            .custom_header("@Comment:\tTrigger: Nonvocal begin/end labels don't match")
            .utterance("CHI", "hello &{laughing} world &{crying} .")
            .build(),
    )?;
    count += 1;

    Ok(count)
}
