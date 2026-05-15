//! Test module for typed in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

use super::helpers::{TestError, assert_header_roundtrip};

/// Tests languages header roundtrip.
#[test]
fn test_languages_header_roundtrip() -> Result<(), TestError> {
    assert_header_roundtrip("@Languages:\teng")?;
    Ok(())
}

/// Tests languages header multiple codes roundtrip.
#[test]
fn test_languages_header_multiple_codes_roundtrip() -> Result<(), TestError> {
    assert_header_roundtrip("@Languages:\teng, spa")?;
    Ok(())
}

/// Tests participants header roundtrip.
#[test]
fn test_participants_header_roundtrip() -> Result<(), TestError> {
    assert_header_roundtrip("@Participants:\tCHI Target_Child, MOT Mother")?;
    Ok(())
}

/// Tests participants header with names roundtrip.
#[test]
fn test_participants_header_with_names_roundtrip() -> Result<(), TestError> {
    assert_header_roundtrip("@Participants:\tFAT Father, CHI Alex Target_Child, MOT Mary Mother")?;
    Ok(())
}

/// Tests id header full roundtrip.
#[test]
fn test_id_header_full_roundtrip() -> Result<(), TestError> {
    assert_header_roundtrip("@ID:\teng|MacWhinney|CHI|1;04.11|male|TD||Target_Child|||")?;
    Ok(())
}

/// Tests id header minimal roundtrip.
#[test]
fn test_id_header_minimal_roundtrip() -> Result<(), TestError> {
    assert_header_roundtrip("@ID:\teng||CHI|||||Target_Child|||")?;
    Ok(())
}

/// Tests id header female roundtrip.
#[test]
fn test_id_header_female_roundtrip() -> Result<(), TestError> {
    assert_header_roundtrip("@ID:\teng||MOT||female|||Mother|||")?;
    Ok(())
}

/// Tests pid header roundtrip.
#[test]
fn test_pid_header_roundtrip() -> Result<(), TestError> {
    assert_header_roundtrip("@PID:\t11312/c-00016447-1")?;
    Ok(())
}

/// Tests media header roundtrip.
#[test]
fn test_media_header_roundtrip() -> Result<(), TestError> {
    assert_header_roundtrip("@Media:\t010411a, audio")?;
    Ok(())
}

/// Tests situation header roundtrip.
#[test]
fn test_situation_header_roundtrip() -> Result<(), TestError> {
    assert_header_roundtrip("@Situation:\tRoss giggling and laughing")?;
    Ok(())
}

/// Tests types header roundtrip.
#[test]
fn test_types_header_roundtrip() -> Result<(), TestError> {
    assert_header_roundtrip("@Types:\tlong, toyplay, TD")?;
    Ok(())
}

/// Tests tape location header roundtrip.
#[test]
fn test_tape_location_header_roundtrip() -> Result<(), TestError> {
    assert_header_roundtrip("@Tape Location:\t0:23:15")?;
    Ok(())
}
