//! Unified Offset Parameter Tests
//!
//! Comprehensive tests for offset parameter functionality across ALL parser implementations.
//! These tests verify that parsers correctly handle the offset parameter, which indicates
//! where a fragment starts in a larger document.
//!
//! **Testing Strategy:**
//! - Run on TreeSitterParser directly
//! - Verify spans are document-absolute (include offset)
//! - Verify error locations are offset-adjusted
//! - Verify roundtrip stability with offsets
//! - Verify UTF-8 multi-byte character handling
//!
//! All tests use insta snapshots for easy review and maintenance.

use talkbank_model::ErrorCollector;
use talkbank_model::{SemanticEq, WriteChat};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::test_error::TestError;

// ============================================================================
// Parser Setup
// ============================================================================

/// Builds the parser used for offset assertions.
fn make_parser() -> Result<TreeSitterParser, TestError> {
    TreeSitterParser::new().map_err(|err| TestError::ParserInit(err.to_string()))
}

// ============================================================================
// Word Parsing with Offsets
// ============================================================================

/// Tests parse word offset zero.
#[test]
fn test_parse_word_offset_zero() -> Result<(), TestError> {
    let parser = make_parser()?;
    let errors = ErrorCollector::new();
    let word = parser
        .parse_word_fragment("hello", 0, &errors)
        .into_option();

    insta::with_settings!({
        snapshot_suffix => "TreeSitter"
    }, {
        insta::assert_debug_snapshot!((word, errors.into_vec()));
    });
    Ok(())
}

/// Tests parse word with offset.
#[test]
fn test_parse_word_with_offset() -> Result<(), TestError> {
    let parser = make_parser()?;
    let errors = ErrorCollector::new();
    let word = parser
        .parse_word_fragment("world", 1000, &errors)
        .into_option();

    insta::with_settings!({
        snapshot_suffix => "TreeSitter"
    }, {
        insta::assert_debug_snapshot!((word, errors.into_vec()));
    });
    Ok(())
}

/// Tests parse word complex with offset.
#[test]
fn test_parse_word_complex_with_offset() -> Result<(), TestError> {
    let parser = make_parser()?;
    let errors = ErrorCollector::new();
    let word = parser
        .parse_word_fragment("dog@c", 500, &errors)
        .into_option();

    insta::with_settings!({
        snapshot_suffix => "TreeSitter"
    }, {
        insta::assert_debug_snapshot!((word, errors.into_vec()));
    });
    Ok(())
}

// ============================================================================
// Main Tier Parsing with Offsets
// ============================================================================

/// Tests parse main tier offset zero.
#[test]
fn test_parse_main_tier_offset_zero() -> Result<(), TestError> {
    let parser = make_parser()?;
    let errors = ErrorCollector::new();
    let main = parser
        .parse_main_tier_fragment("*CHI:\thello .", 0, &errors)
        .into_option();

    insta::with_settings!({
        snapshot_suffix => "TreeSitter"
    }, {
        insta::assert_debug_snapshot!((main, errors.into_vec()));
    });
    Ok(())
}

/// Tests parse main tier with offset.
#[test]
fn test_parse_main_tier_with_offset() -> Result<(), TestError> {
    let parser = make_parser()?;
    let errors = ErrorCollector::new();
    let main = parser
        .parse_main_tier_fragment("*CHI:\thello world .", 200, &errors)
        .into_option();

    insta::with_settings!({
        snapshot_suffix => "TreeSitter"
    }, {
        insta::assert_debug_snapshot!((main, errors.into_vec()));
    });
    Ok(())
}

// ============================================================================
// Dependent Tier Parsing with Offsets
// ============================================================================

/// Tests parse mor tier offset zero.
#[test]
fn test_parse_mor_tier_offset_zero() -> Result<(), TestError> {
    let parser = make_parser()?;
    let errors = ErrorCollector::new();
    let mor = parser
        .parse_mor_tier_fragment("pro|I v|want .", 0, &errors)
        .into_option();

    insta::with_settings!({
        snapshot_suffix => "TreeSitter"
    }, {
        insta::assert_debug_snapshot!((mor, errors.into_vec()));
    });
    Ok(())
}

/// Tests parse mor tier with offset.
#[test]
fn test_parse_mor_tier_with_offset() -> Result<(), TestError> {
    let parser = make_parser()?;
    let errors = ErrorCollector::new();
    let mor = parser
        .parse_mor_tier_fragment("pro|I v|want .", 300, &errors)
        .into_option();

    insta::with_settings!({
        snapshot_suffix => "TreeSitter"
    }, {
        insta::assert_debug_snapshot!((mor, errors.into_vec()));
    });
    Ok(())
}

// ============================================================================
// Error Offset Tests
// ============================================================================

/// Tests error offset in word.
#[test]
fn test_error_offset_in_word() -> Result<(), TestError> {
    let parser = make_parser()?;
    let errors = ErrorCollector::new();
    let word = parser.parse_word_fragment("xx", 100, &errors).into_option();

    // Verify error spans are offset-adjusted
    let error_vec = errors.into_vec();
    if let Some(first) = error_vec.first() {
        if first.location.span.start < 100 {
            return Err(TestError::Failure(format!(
                "Error span should be offset-adjusted: got start={}, expected >=100",
                first.location.span.start
            )));
        }
    }

    insta::with_settings!({
        snapshot_suffix => "TreeSitter"
    }, {
        insta::assert_debug_snapshot!((word, error_vec));
    });
    Ok(())
}

/// Tests error offset in main tier.
#[test]
fn test_error_offset_in_main_tier() -> Result<(), TestError> {
    let parser = make_parser()?;
    let errors = ErrorCollector::new();
    let main = parser
        .parse_main_tier_fragment("*CHI:\txx .", 500, &errors)
        .into_option();

    // Verify error spans are offset-adjusted
    let error_vec = errors.into_vec();
    if let Some(first) = error_vec.first() {
        if first.location.span.start < 500 {
            return Err(TestError::Failure(format!(
                "Error span should be offset-adjusted: got start={}, expected >=500",
                first.location.span.start
            )));
        }
    }

    insta::with_settings!({
        snapshot_suffix => "TreeSitter"
    }, {
        insta::assert_debug_snapshot!((main, error_vec));
    });
    Ok(())
}

// ============================================================================
// Roundtrip with Offsets
// ============================================================================

/// Tests roundtrip with offset.
#[test]
fn test_roundtrip_with_offset() -> Result<(), TestError> {
    let parser = make_parser()?;
    let input = "*CHI:\thello world .";
    let offset = 100;

    // Parse with offset
    let errors1 = ErrorCollector::new();
    let main1 = parser
        .parse_main_tier_fragment(input, offset, &errors1)
        .into_option()
        .ok_or_else(|| TestError::Failure("Should parse successfully".to_string()))?;

    // Serialize back to string
    let mut buf = String::new();
    main1.write_chat(&mut buf)?;
    let serialized = buf;

    // Parse again with same offset
    let errors2 = ErrorCollector::new();
    let main2 = parser
        .parse_main_tier_fragment(&serialized, offset, &errors2)
        .into_option()
        .ok_or_else(|| TestError::Failure("Should parse successfully on roundtrip".to_string()))?;

    // Verify semantic equality
    let is_semantically_equal = main1.semantic_eq(&main2);

    insta::with_settings!({
        snapshot_suffix => "TreeSitter"
    }, {
        insta::assert_debug_snapshot!((is_semantically_equal, main1, main2));
    });
    Ok(())
}

// ============================================================================
// Multiple Fragments with Different Offsets
// ============================================================================

/// Tests multiple fragments different offsets.
#[test]
fn test_multiple_fragments_different_offsets() -> Result<(), TestError> {
    let parser = make_parser()?;
    let fragments = [("hello", 0), ("world", 100), ("foo", 200), ("bar", 300)];

    let results: Vec<_> = fragments
        .iter()
        .map(|(input, offset)| {
            let errors = ErrorCollector::new();
            let word = parser
                .parse_word_fragment(input, *offset, &errors)
                .into_option();
            (input, offset, word, errors.into_vec())
        })
        .collect();

    insta::with_settings!({
        snapshot_suffix => "TreeSitter"
    }, {
        insta::assert_debug_snapshot!(results);
    });
    Ok(())
}

// ============================================================================
// UTF-8 Multi-byte Characters with Offset
// ============================================================================

/// Tests offset with multibyte utf8.
#[test]
fn test_offset_with_multibyte_utf8() -> Result<(), TestError> {
    let parser = make_parser()?;
    let errors = ErrorCollector::new();

    let input = "hello😊world";
    let offset = 1000;

    let word = parser
        .parse_word_fragment(input, offset, &errors)
        .into_option();

    insta::with_settings!({
        snapshot_suffix => "TreeSitter"
    }, {
        insta::assert_debug_snapshot!((input.len(), word, errors.into_vec()));
    });
    Ok(())
}

/// Tests offset with chinese characters.
#[test]
fn test_offset_with_chinese_characters() -> Result<(), TestError> {
    let parser = make_parser()?;
    let errors = ErrorCollector::new();

    let input = "你好";
    let offset = 500;

    let word = parser
        .parse_word_fragment(input, offset, &errors)
        .into_option();

    insta::with_settings!({
        snapshot_suffix => "TreeSitter"
    }, {
        insta::assert_debug_snapshot!((input.len(), word, errors.into_vec()));
    });
    Ok(())
}

// ============================================================================
// Fragment Parsing with Offsets
// ============================================================================

/// Tests word parsing with offset.
#[test]
fn test_parser_equivalence_word_with_offset() -> Result<(), TestError> {
    let parser = make_parser()?;
    let input = "hello@s";
    let offset = 100;

    let errors = ErrorCollector::new();
    let word = parser
        .parse_word_fragment(input, offset, &errors)
        .into_option();

    insta::assert_debug_snapshot!(word);

    Ok(())
}

/// Tests main tier parsing with offset.
#[test]
fn test_parser_equivalence_main_tier_with_offset() -> Result<(), TestError> {
    let parser = make_parser()?;
    let input = "*CHI:\thello world .";
    let offset = 200;

    let errors = ErrorCollector::new();
    let main = parser
        .parse_main_tier_fragment(input, offset, &errors)
        .into_option();

    insta::assert_debug_snapshot!(main);

    Ok(())
}

/// Tests mor tier parsing with offset.
#[test]
fn test_parser_equivalence_mor_tier_with_offset() -> Result<(), TestError> {
    let parser = make_parser()?;
    let input = "pro|I v|want .";
    let offset = 300;

    let errors = ErrorCollector::new();
    let mor = parser
        .parse_mor_tier_fragment(input, offset, &errors)
        .into_option();

    insta::assert_debug_snapshot!(mor);

    Ok(())
}
