//! E2xx: Word Errors
//!
//! Generates validation error corpus files for word-level errors (E2xx range).
//! These are validation errors — the parser succeeds, but validation catches the issue.

use std::fs;
use std::path::Path;

use crate::ChatFileBuilder;

use super::{GenResult, write_file};

//
// E2xx: Word Errors (20 missing)
//

/// Generates e2xx word errors.
pub fn generate_e2xx_word_errors(root: &Path) -> GenResult {
    let dir = root.join("validation_errors");
    fs::create_dir_all(&dir)?;

    let mut count = 0;

    // E230: UnbalancedCADelimiter
    write_file(
        &dir.join("E230_unbalanced_ca_delimiter.cha"),
        ChatFileBuilder::new()
            .speaker("CHI", "Target_Child")
            .custom_header("@Comment:\tERROR: Compound delimiter (∆) must be balanced")
            .custom_header("@Comment:\tInvalid: 'hello∆world' - Missing closing ∆")
            .utterance("CHI", "hello∆world .")
            .build(),
    )?;
    count += 1;

    // E231: UnbalancedShortening
    write_file(
        &dir.join("E231_unbalanced_shortening.cha"),
        ChatFileBuilder::new()
            .speaker("CHI", "Target_Child")
            .custom_header("@Comment:\tERROR: Shortening parenthesis must be balanced")
            .custom_header("@Comment:\tInvalid: 'hel(lo' - Missing closing parenthesis")
            .utterance("CHI", "hel(lo .")
            .build(),
    )?;
    count += 1;

    // E232: InvalidCompoundMarkerPosition
    write_file(
        &dir.join("E232_compound_marker_at_start.cha"),
        ChatFileBuilder::new()
            .speaker("CHI", "Target_Child")
            .custom_header("@Comment:\tERROR: Compound marker (+) cannot be at word start")
            .custom_header("@Comment:\tInvalid: '+hello' - Compound marker at start")
            .utterance("CHI", "+hello .")
            .build(),
    )?;
    count += 1;

    // E233: EmptyCompoundPart
    write_file(
        &dir.join("E233_empty_compound_part.cha"),
        ChatFileBuilder::new()
            .speaker("CHI", "Target_Child")
            .custom_header("@Comment:\tERROR: Compound marker requires text after it")
            .custom_header("@Comment:\tInvalid: 'hello+' - Nothing after compound marker")
            .utterance("CHI", "hello+ .")
            .build(),
    )?;
    count += 1;

    // E242: UnbalancedQuotation
    write_file(
        &dir.join("E242_unbalanced_quotation.cha"),
        ChatFileBuilder::new()
            .speaker("CHI", "Target_Child")
            .custom_header("@Comment:\tERROR: Quotation marks must be balanced")
            .custom_header("@Comment:\tInvalid: '\"hello' - Missing closing quote")
            .utterance("CHI", "\"hello .")
            .build(),
    )?;
    count += 1;

    // E241: IllegalUntranscribed
    write_file(
        &dir.join("E241_illegal_untranscribed.cha"),
        ChatFileBuilder::new()
            .speaker("CHI", "Target_Child")
            .custom_header("@Comment:\tERROR: Untranscribed markers must be xxx, yyy, or www")
            .custom_header("@Comment:\tInvalid: 'xx' - Only xxx, yyy, www are allowed")
            .utterance("CHI", "xx .")
            .build(),
    )?;
    count += 1;

    // E243: IllegalCharactersInWord
    write_file(
        &dir.join("E243_illegal_characters.cha"),
        ChatFileBuilder::new()
            .speaker("CHI", "Target_Child")
            .custom_header("@Comment:\tERROR: @ character only allowed for form markers")
            .custom_header("@Comment:\tInvalid: 'hell@' - @ in wrong position")
            .utterance("CHI", "hell@ .")
            .build(),
    )?;
    count += 1;

    // E244: ConsecutiveStressMarkers
    write_file(
        &dir.join("E244_consecutive_stress.cha"),
        ChatFileBuilder::new()
            .speaker("CHI", "Target_Child")
            .custom_header("@Comment:\tERROR: Stress markers cannot be consecutive")
            .custom_header("@Comment:\tInvalid: 'ˈˈhello' - Two stress marks in a row")
            .utterance("CHI", "ˈˈhello .")
            .build(),
    )?;
    count += 1;

    // E245: StressNotBeforeSpokenMaterial
    write_file(
        &dir.join("E245_stress_without_material.cha"),
        ChatFileBuilder::new()
            .speaker("CHI", "Target_Child")
            .custom_header("@Comment:\tERROR: Stress marker must precede spoken material")
            .custom_header("@Comment:\tInvalid: 'ˈ' - Stress without following text")
            .utterance("CHI", "ˈ .")
            .build(),
    )?;
    count += 1;

    // E246: LengtheningNotAfterSpokenMaterial
    write_file(
        &dir.join("E246_lengthening_before_material.cha"),
        ChatFileBuilder::new()
            .speaker("CHI", "Target_Child")
            .custom_header("@Comment:\tERROR: Lengthening (:) must follow spoken material")
            .custom_header("@Comment:\tInvalid: ':hello' - Lengthening before text")
            .utterance("CHI", ":hello .")
            .build(),
    )?;
    count += 1;

    // E247: MultiplePrimaryStress
    write_file(
        &dir.join("E247_multiple_primary_stress.cha"),
        ChatFileBuilder::new()
            .speaker("CHI", "Target_Child")
            .custom_header("@Comment:\tERROR: Word can only have one primary stress")
            .custom_header("@Comment:\tInvalid: 'ˈheˈllo' - Two primary stress markers")
            .utterance("CHI", "ˈheˈllo .")
            .build(),
    )?;
    count += 1;

    // E250: SecondaryStressWithoutPrimary
    write_file(
        &dir.join("E250_secondary_without_primary.cha"),
        ChatFileBuilder::new()
            .speaker("CHI", "Target_Child")
            .custom_header("@Comment:\tERROR: Secondary stress requires primary stress")
            .custom_header("@Comment:\tInvalid: 'ˌhello' - Secondary stress without primary")
            .utterance("CHI", "ˌhello .")
            .build(),
    )?;
    count += 1;

    // E252: SyllablePauseNotBetweenSpokenMaterial
    write_file(
        &dir.join("E252_pause_not_between_syllables.cha"),
        ChatFileBuilder::new()
            .speaker("CHI", "Target_Child")
            .custom_header("@Comment:\tERROR: Syllable pause (^) must be between syllables")
            .custom_header("@Comment:\tInvalid: '^hello' - Pause not between material")
            .utterance("CHI", "^hello .")
            .build(),
    )?;
    count += 1;

    // E248: TertiaryLanguageNeedsExplicitCode
    write_file(
        &dir.join("E248_tertiary_without_code.cha"),
        ChatFileBuilder::new()
            .language("eng")
            .custom_header("@Languages:\teng, spa")
            .speaker("CHI", "Target_Child")
            .custom_header("@Comment:\tERROR: Tertiary language (@t) requires explicit code")
            .custom_header("@Comment:\tInvalid: 'hello@t' - Must specify language code")
            .utterance("CHI", "hello@t .")
            .build(),
    )?;
    count += 1;

    // E249: MissingLanguageContext
    write_file(
        &dir.join("E249_missing_language_context.cha"),
        ChatFileBuilder::new()
            .speaker("CHI", "Target_Child")
            .custom_header("@Comment:\tERROR: Language marker without context")
            .custom_header("@Comment:\tInvalid: Language marker usage without declaration")
            .utterance("CHI", "hello@s .")
            .build(),
    )?;
    count += 1;

    // E203: InvalidFormType
    write_file(
        &dir.join("E203_invalid_form_type.cha"),
        ChatFileBuilder::new()
            .speaker("CHI", "Target_Child")
            .custom_header("@Comment:\tERROR: Form type must be valid (b, c, d, etc.)")
            .custom_header("@Comment:\tInvalid: 'hello@invalid' - Unknown form type")
            .utterance("CHI", "hello@xyz .")
            .build(),
    )?;
    count += 1;

    // E214: EmptyAnnotatedContentAnnotations
    write_file(
        &dir.join("E214_empty_scoped_annotation.cha"),
        ChatFileBuilder::new()
            .speaker("CHI", "Target_Child")
            .custom_header("@Comment:\tERROR: Scoped annotation cannot be empty")
            .custom_header("@Comment:\tInvalid: 'hello [*]' - Empty error annotation")
            .utterance("CHI", "hello [*] .")
            .build(),
    )?;
    count += 1;

    // E208: EmptyReplacement
    write_file(
        &dir.join("E208_empty_replacement.cha"),
        ChatFileBuilder::new()
            .speaker("CHI", "Target_Child")
            .custom_header("@Comment:\tExpected error: E208 (Empty replacement)")
            .custom_header("@Comment:\tTrigger: Replacement with empty target")
            .utterance("CHI", "hello [: ] .")
            .build(),
    )?;
    count += 1;

    // E209: EmptySpokenContent
    write_file(
        &dir.join("E209_empty_spoken_content.cha"),
        ChatFileBuilder::new()
            .speaker("CHI", "Target_Child")
            .custom_header("@Comment:\tExpected error: E209 (Empty spoken content)")
            .custom_header("@Comment:\tTrigger: Word with form marker but no spoken text")
            .utterance("CHI", "@l .")
            .build(),
    )?;
    count += 1;

    // E251: EmptyWordContentText
    write_file(
        &dir.join("E251_empty_word_content_text.cha"),
        ChatFileBuilder::new()
            .speaker("CHI", "Target_Child")
            .custom_header("@Comment:\tExpected error: E251 (Empty word content text)")
            .custom_header("@Comment:\tTrigger: Word with annotations but empty text")
            .utterance("CHI", "@s:eng .")
            .build(),
    )?;
    count += 1;

    // E258: ConsecutiveCommas
    write_file(
        &dir.join("E258_consecutive_commas.cha"),
        ChatFileBuilder::new()
            .speaker("CHI", "Target_Child")
            .custom_header("@Comment:\tExpected error: E258 (Consecutive commas)")
            .custom_header("@Comment:\tTrigger: Two comma separators in a row")
            .utterance("CHI", "hello ,, world .")
            .build(),
    )?;
    count += 1;

    // E253: EmptyWordContent
    write_file(
        &dir.join("E253_empty_word_content.cha"),
        ChatFileBuilder::new()
            .speaker("CHI", "Target_Child")
            .custom_header("@Comment:\tERROR: Word must have content")
            .custom_header("@Comment:\tInvalid: Empty word element")
            .utterance("CHI", "  .")
            .build(),
    )?;
    count += 1;

    Ok(count)
}
