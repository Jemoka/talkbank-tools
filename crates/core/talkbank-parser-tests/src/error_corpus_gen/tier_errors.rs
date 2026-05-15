//! E4xx and E6xx: Dependent Tier and Tier Validation Errors
//!
//! Generates corpus files for dependent tier errors (E4xx) and
//! tier validation errors (E6xx).

use std::fs;
use std::path::Path;

use crate::ChatFileBuilder;

use super::{GenResult, write_file};

//
// E4xx: Dependent Tier Errors (1 missing)
//

/// Generates e4xx dependent tier errors.
pub fn generate_e4xx_dependent_tier_errors(root: &Path) -> GenResult {
    let dir = root.join("validation_errors");
    fs::create_dir_all(&dir)?;

    let mut count = 0;

    // E404: OrphanedDependentTier
    write_file(
        &dir.join("E404_orphaned_dependent_tier.cha"),
        ChatFileBuilder::new()
            .speaker("CHI", "Target_Child")
            .custom_header("@Comment:\tERROR: Dependent tier without preceding main tier")
            .custom_header("@Comment:\tInvalid: %mor without *CHI:")
            .custom_header("%mor:\tpro|I v|want n|cookie .")
            .build(),
    )?;
    count += 1;

    Ok(count)
}

//
// E6xx: Tier Validation Errors (4 missing)
//

/// Generates e6xx tier errors.
pub fn generate_e6xx_tier_errors(root: &Path) -> GenResult {
    let dir = root.join("validation_errors");
    fs::create_dir_all(&dir)?;

    let mut count = 0;

    // E600: TierValidationError
    write_file(
        &dir.join("E600_tier_validation_error.cha"),
        ChatFileBuilder::new()
            .speaker("CHI", "Target_Child")
            .custom_header("@Comment:\tERROR: Generic tier validation failure")
            .utterance("CHI", "I want cookie .")
            .dependent_tier("mor", "invalid|mor|format")
            .build(),
    )?;
    count += 1;

    // E601: InvalidDependentTier
    write_file(
        &dir.join("E601_invalid_dependent_tier.cha"),
        ChatFileBuilder::new()
            .speaker("CHI", "Target_Child")
            .custom_header("@Comment:\tERROR: Dependent tier semantically invalid")
            .utterance("CHI", "hello .")
            .dependent_tier("mor", "|||")
            .build(),
    )?;
    count += 1;

    // E602: MalformedTierHeader
    write_file(
        &dir.join("E602_malformed_tier_header.cha"),
        "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n\
         @ID:\teng|corpus|CHI|||||Child|||\n\
         @Comment:\tERROR: Tier header malformed\n\
         *CHI:\thello .\n\
         %mor\tpro|I .\n\
         @End\n"
            .to_string(),
    )?;
    count += 1;

    // E604: GraWithoutMor
    write_file(
        &dir.join("E604_gra_without_mor.cha"),
        ChatFileBuilder::new()
            .speaker("CHI", "Target_Child")
            .custom_header("@Comment:\tERROR: %gra requires %mor to be present")
            .utterance("CHI", "I want cookie .")
            .dependent_tier("gra", "1|2|SUBJ 2|0|ROOT 3|2|OBJ 4|2|PUNCT")
            .build(),
    )?;
    count += 1;

    Ok(count)
}
