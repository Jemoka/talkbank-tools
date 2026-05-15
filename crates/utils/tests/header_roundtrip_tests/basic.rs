//! Test module for basic in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

use super::helpers::{TestError, assert_header_roundtrip};

/// Tests utf8 header roundtrip.
#[test]
fn test_utf8_header_roundtrip() -> Result<(), TestError> {
    assert_header_roundtrip("@UTF8")?;
    Ok(())
}

/// Tests begin header roundtrip.
#[test]
fn test_begin_header_roundtrip() -> Result<(), TestError> {
    assert_header_roundtrip("@Begin")?;
    Ok(())
}

/// Tests end header roundtrip.
#[test]
fn test_end_header_roundtrip() -> Result<(), TestError> {
    assert_header_roundtrip("@End")?;
    Ok(())
}

/// Tests new episode header roundtrip.
#[test]
fn test_new_episode_header_roundtrip() -> Result<(), TestError> {
    assert_header_roundtrip("@New Episode")?;
    Ok(())
}
