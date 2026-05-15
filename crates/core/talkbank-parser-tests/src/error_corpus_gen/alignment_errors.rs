//! E7xx: Alignment/Temporal Errors
//!
//! Generates validation error corpus files for alignment and temporal errors
//! (E7xx range), including bullet timestamp monotonicity, GRA index validation,
//! and speaker overlap rules.

use std::fs;
use std::path::Path;

use crate::ChatFileBuilder;

use super::{GenResult, write_file};

//
// E7xx: Alignment/Temporal Errors (7 missing)
//

/// Generates e7xx alignment errors.
pub fn generate_e7xx_alignment_errors(root: &Path) -> GenResult {
    let dir = root.join("validation_errors");
    fs::create_dir_all(&dir)?;

    let mut count = 0;

    // E701: TierBeginTimeNotMonotonic
    write_file(
        &dir.join("E701_bullet_not_monotonic.cha"),
        ChatFileBuilder::new()
            .speaker("CHI", "Target_Child")
            .custom_header("@Comment:\tERROR: Bullet timestamps must be monotonically increasing")
            .utterance_with_timing("CHI", "hello .", 1000, 2000)
            .utterance_with_timing("CHI", "world .", 500, 1500)
            .build(),
    )?;
    count += 1;

    // E712: GraInvalidWordIndex
    write_file(
        &dir.join("E712_gra_invalid_word_index.cha"),
        ChatFileBuilder::new()
            .speaker("CHI", "Target_Child")
            .custom_header("@Comment:\tERROR: Grammar relation word index out of bounds")
            .utterance("CHI", "I want .")
            .dependent_tier("mor", "pro|I v|want .")
            .dependent_tier("gra", "1|2|SUBJ 5|0|ROOT 3|2|PUNCT")
            .build(),
    )?;
    count += 1;

    // E713: GraInvalidHeadIndex
    write_file(
        &dir.join("E713_gra_invalid_head_index.cha"),
        ChatFileBuilder::new()
            .speaker("CHI", "Target_Child")
            .custom_header("@Comment:\tERROR: Grammar relation head index out of bounds")
            .utterance("CHI", "I want .")
            .dependent_tier("mor", "pro|I v|want .")
            .dependent_tier("gra", "1|2|SUBJ 2|10|ROOT 3|2|PUNCT")
            .build(),
    )?;
    count += 1;

    // E704: SpeakerSelfOverlap
    write_file(
        &dir.join("E704_speaker_self_overlap.cha"),
        ChatFileBuilder::new()
            .speaker("CHI", "Target_Child")
            .custom_header("@Comment:\tERROR: Speaker cannot overlap with themselves")
            .utterance("CHI", "hello \u{2308} world \u{2309} .")
            .utterance("CHI", "\u{230a} testing \u{230b} .")
            .build(),
    )?;
    count += 1;

    // E700: UnexpectedTierNode
    write_file(
        &dir.join("E700_unexpected_tier_node.cha"),
        ChatFileBuilder::new()
            .speaker("CHI", "Target_Child")
            .custom_header("@Comment:\tExpected error: E700 (Unexpected tier node)")
            .custom_header("@Comment:\tTrigger: Tier body contains unexpected node type")
            .utterance("CHI", "hello .")
            .dependent_tier("mor", "pro|I v|want\x01 .")
            .build(),
    )?;
    count += 1;

    // E703: UnexpectedMorphologyNode
    write_file(
        &dir.join("E703_unexpected_mor_node.cha"),
        ChatFileBuilder::new()
            .speaker("CHI", "Target_Child")
            .custom_header("@Comment:\tExpected error: E703 (Unexpected morphology node)")
            .custom_header("@Comment:\tTrigger: Invalid morphology format")
            .utterance("CHI", "hello .")
            .dependent_tier("mor", "||||| .")
            .build(),
    )?;
    count += 1;

    // E709: InvalidGrammarIndex
    write_file(
        &dir.join("E709_invalid_grammar_index.cha"),
        ChatFileBuilder::new()
            .speaker("CHI", "Target_Child")
            .custom_header("@Comment:\tExpected error: E709 (Invalid grammar index)")
            .custom_header("@Comment:\tTrigger: GRA relation has non-numeric index")
            .utterance("CHI", "hello .")
            .dependent_tier("mor", "co|hello .")
            .dependent_tier("gra", "abc|0|ROOT .")
            .build(),
    )?;
    count += 1;

    // E721: GraNonSequentialIndex
    write_file(
        &dir.join("E721_gra_non_sequential.cha"),
        ChatFileBuilder::new()
            .speaker("CHI", "Target_Child")
            .custom_header("@Comment:\tExpected error: E721 (GRA non-sequential index)")
            .custom_header("@Comment:\tTrigger: GRA indices not in sequential order")
            .utterance("CHI", "I want cookie .")
            .dependent_tier("mor", "pro|I v|want n|cookie .")
            .dependent_tier("gra", "1|2|SUBJ 3|2|OBJ 2|0|ROOT .")
            .build(),
    )?;
    count += 1;

    // E722: GraNoRoot
    write_file(
        &dir.join("E722_gra_no_root.cha"),
        ChatFileBuilder::new()
            .speaker("CHI", "Target_Child")
            .custom_header("@Comment:\tExpected error: E722 (GRA has no ROOT)")
            .custom_header("@Comment:\tTrigger: No relation with head=0 (ROOT)")
            .utterance("CHI", "I want .")
            .dependent_tier("mor", "pro|I v|want .")
            .dependent_tier("gra", "1|2|SUBJ 2|1|OBJ .")
            .build(),
    )?;
    count += 1;

    // E723: GraMultipleRoots
    write_file(
        &dir.join("E723_gra_multiple_roots.cha"),
        ChatFileBuilder::new()
            .speaker("CHI", "Target_Child")
            .custom_header("@Comment:\tExpected error: E723 (GRA has multiple ROOTs)")
            .custom_header("@Comment:\tTrigger: Multiple relations with head=0 (ROOT)")
            .utterance("CHI", "I want .")
            .dependent_tier("mor", "pro|I v|want .")
            .dependent_tier("gra", "1|0|ROOT 2|0|ROOT .")
            .build(),
    )?;
    count += 1;

    Ok(count)
}
