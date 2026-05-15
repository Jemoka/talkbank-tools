//! E5xx: Header Errors
//!
//! Generates validation error corpus files for header-level errors (E5xx range).

use std::fs;
use std::path::Path;

use crate::ChatFileBuilder;

use super::{GenResult, write_file};

//
// E5xx: Header Errors (9 missing)
//

/// Generates e5xx header errors.
pub fn generate_e5xx_header_errors(root: &Path) -> GenResult {
    let dir = root.join("validation_errors");
    fs::create_dir_all(&dir)?;

    let mut count = 0;

    // E502: MissingEndHeader
    write_file(
        &dir.join("E502_missing_end_header.cha"),
        "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n\
         @ID:\teng|corpus|CHI|||||Child|||\n\
         @Comment:\tERROR: Missing @End header\n\
         *CHI:\thello world .\n"
            .to_string(),
    )?;
    count += 1;

    // E513: EmptyParticipantRole
    write_file(
        &dir.join("E513_empty_participant_role.cha"),
        "@UTF8\n@Begin\n@Languages:\teng\n\
         @Participants:\tCHI\n\
         @ID:\teng|corpus|CHI|||||Child|||\n\
         @Comment:\tERROR: Participant must have role\n\
         *CHI:\thello .\n@End\n"
            .to_string(),
    )?;
    count += 1;

    // E516: EmptyDate
    write_file(
        &dir.join("E516_empty_date.cha"),
        ChatFileBuilder::new()
            .speaker("CHI", "Target_Child")
            .custom_header("@Date:\t")
            .custom_header("@Comment:\tERROR: Date header cannot be empty")
            .utterance("CHI", "hello .")
            .build(),
    )?;
    count += 1;

    // E519: InvalidLanguageCode
    write_file(
        &dir.join("E519_invalid_language_code.cha"),
        ChatFileBuilder::new()
            .language("xyz")
            .speaker("CHI", "Target_Child")
            .custom_header("@Comment:\tERROR: Language code must be valid ISO 639-3")
            .custom_header("@Comment:\tInvalid: 'xyz' - Not a valid ISO language code")
            .utterance("CHI", "hello .")
            .build(),
    )?;
    count += 1;

    // E528: GemLabelMismatch
    write_file(
        &dir.join("E528_gem_label_mismatch.cha"),
        ChatFileBuilder::new()
            .speaker("CHI", "Target_Child")
            .custom_header("@Bg:label1")
            .custom_header("@Comment:\tERROR: Gem label must match")
            .utterance("CHI", "hello .")
            .custom_header("@Eg:label2")
            .custom_header("@Comment:\tInvalid: @Bg:label1 ... @Eg:label2 - Labels don't match")
            .build(),
    )?;
    count += 1;

    // E529: NestedBeginGem
    write_file(
        &dir.join("E529_nested_begin_gem.cha"),
        ChatFileBuilder::new()
            .speaker("CHI", "Target_Child")
            .custom_header("@Bg:outer")
            .custom_header("@Bg:inner")
            .custom_header("@Comment:\tERROR: Cannot nest @Bg without closing previous")
            .utterance("CHI", "hello .")
            .custom_header("@Eg:inner")
            .custom_header("@Eg:outer")
            .build(),
    )?;
    count += 1;

    // E530: LazyGemInsideScope
    write_file(
        &dir.join("E530_lazy_gem_inside_scope.cha"),
        ChatFileBuilder::new()
            .speaker("CHI", "Target_Child")
            .custom_header("@Bg:outer")
            .custom_header("@Bg:")
            .custom_header("@Comment:\tERROR: Lazy @Bg (no label) inside scoped gem")
            .utterance("CHI", "hello .")
            .custom_header("@Eg:")
            .custom_header("@Eg:outer")
            .build(),
    )?;
    count += 1;

    // E531: MediaFilenameMismatch
    write_file(
        &dir.join("E531_media_filename_mismatch.cha"),
        ChatFileBuilder::new()
            .speaker("CHI", "Target_Child")
            .custom_header("@Media:\tdifferent, audio")
            .custom_header("@Comment:\tERROR: Media filename must match transcript")
            .utterance("CHI", "hello .")
            .build(),
    )?;
    count += 1;

    // E532: InvalidParticipantRole
    write_file(
        &dir.join("E532_invalid_participant_role.cha"),
        "@UTF8\n@Begin\n@Languages:\teng\n\
         @Participants:\tCHI InvalidRole123\n\
         @ID:\teng|corpus|CHI|||||InvalidRole123|||\n\
         @Comment:\tERROR: Participant role must be valid format\n\
         *CHI:\thello .\n@End\n"
            .to_string(),
    )?;
    count += 1;

    // E533: EmptyOptionsHeader
    write_file(
        &dir.join("E533_empty_options_header.cha"),
        ChatFileBuilder::new()
            .speaker("CHI", "Target_Child")
            .custom_header("@Options:\t")
            .custom_header("@Comment:\tERROR: Options header cannot be empty")
            .utterance("CHI", "hello .")
            .build(),
    )?;
    count += 1;

    // E514: MissingLanguageCode
    write_file(
        &dir.join("E514_missing_language_code.cha"),
        "@UTF8\n@Begin\n@Languages:\teng\n\
         @Participants:\tCHI Child\n\
         @ID:\t|corpus|CHI|||||Child|||\n\
         @Comment:\tExpected error: E514 (Missing language code in @ID)\n\
         *CHI:\thello .\n@End\n"
            .to_string(),
    )?;
    count += 1;

    Ok(count)
}
