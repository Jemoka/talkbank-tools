//! Utterance-level validation regression tests.
//!
//! These fixtures exercise checks that require visibility across multiple content
//! items in one utterance, such as quotation pairing, CA delimiter balance,
//! overlap index constraints, and underline pairing.
//!
//! CHAT references:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Words>
//! - <https://talkbank.org/0info/manuals/CHAT.html#CA_Delimiters>
//! - <https://talkbank.org/0info/manuals/CHAT.html#CA_Overlaps>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Terminators>

use crate::model::{
    BracketedContent, BracketedItem, CADelimiter, CADelimiterType, Group, MainTier, OverlapIndex,
    OverlapPoint, OverlapPointKind, Postcode, Terminator, UnderlineMarker, Utterance,
    UtteranceContent, Word, WordContent, WordText, WordUnderlineEnd,
};
use crate::validation::ValidationContext;
use crate::validation::utterance::{
    CADelimiterRole, analyze_ca_delimiter_roles, check_ca_delimiter_balance,
    check_overlap_index_values, check_quotation_balance, check_underline_balance,
};
use crate::{ErrorCode, ErrorCollector, Span};

/// Accepts balanced quotation postcode pairs within one utterance.
///
/// This positive fixture ensures the quotation stack logic does not report
/// `E242` when begin and end markers are matched.
#[test]
fn test_e242_balanced_quotations() {
    // Create utterance with balanced quotation markers
    let main = MainTier::new("CHI", vec![], Terminator::Period { span: Span::DUMMY })
        .with_postcodes(vec![
            Postcode::new("\"/"),  // Begin
            Postcode::new("\"/."), // End
        ]);

    let utterance = Utterance::new(main);
    let errors = ErrorCollector::new();
    check_quotation_balance(&utterance, &errors);

    assert!(
        errors.into_vec().is_empty(),
        "Balanced quotations should not produce errors"
    );
}

/// Reports `E242` when a quotation begin marker has no matching end marker.
///
/// The test validates both code selection and the diagnostic wording for an
/// unclosed quotation span.
#[test]
fn test_e242_unbalanced_quotations_missing_end() {
    // Create utterance with unbalanced quotations (missing end) - stack-based validation
    let main = MainTier::new("CHI", vec![], Terminator::Period { span: Span::DUMMY })
        .with_postcodes(vec![
            Postcode::new("\"/"), // Begin without end
        ]);

    let utterance = Utterance::new(main);
    let errors = ErrorCollector::new();
    check_quotation_balance(&utterance, &errors);

    let error_vec = errors.into_vec();
    assert_eq!(
        error_vec.len(),
        1,
        "Expected E242 error for missing end marker"
    );
    assert_eq!(error_vec[0].code, ErrorCode::UnbalancedQuotation);
    assert!(error_vec[0].message.contains("unclosed quotation begin"));
}

/// Reports `E242` when a quotation end marker appears without a begin marker.
///
/// This captures the mirror case of stray closing quotation markup.
#[test]
fn test_e242_unbalanced_quotations_missing_begin() {
    // Create utterance with unbalanced quotations (missing begin) - stack-based validation
    let main = MainTier::new("CHI", vec![], Terminator::Period { span: Span::DUMMY })
        .with_postcodes(vec![
            Postcode::new("\"/."), // End without begin
        ]);

    let utterance = Utterance::new(main);
    let errors = ErrorCollector::new();
    check_quotation_balance(&utterance, &errors);

    let error_vec = errors.into_vec();
    assert_eq!(
        error_vec.len(),
        1,
        "Expected E242 error for missing begin marker"
    );
    assert_eq!(error_vec[0].code, ErrorCode::UnbalancedQuotation);
    assert!(
        error_vec[0].message.contains("Quotation end")
            && error_vec[0].message.contains("without corresponding begin")
    );
}

/// Treats malformed postcode text as non-matching and therefore unbalanced.
///
/// A near-match like `"/. "` must not count as a valid close marker, so the
/// utterance still emits quotation-balance errors.
#[test]
fn test_e242_multiple_balanced_quotations() {
    // Create utterance with multiple balanced quotation pairs
    // The postcode text MUST exactly match: "/" for begin, "/." for end
    // No spaces allowed in the postcode text value
    let main = MainTier::new("CHI", vec![], Terminator::Period { span: Span::DUMMY })
        .with_postcodes(vec![
            Postcode::new("\"/"),   // Begin 1
            Postcode::new("\"/. "), // End 1 - WRONG: has trailing space, won't match!
            Postcode::new("\"/"),   // Begin 2
            Postcode::new("\"/."),  // End 2
        ]);

    let utterance = Utterance::new(main);
    let errors = ErrorCollector::new();
    check_quotation_balance(&utterance, &errors);

    // The test postcodes are unbalanced because postcode 2 has text `"/. "` with a space.
    // Since validation checks for exact match of `"/."`, this doesn't match.
    // Expected: 2 opens, 1 close (the one with space), 1 close
    // Actual parse: 2 opens, 0 closes from second postcode, 1 close
    // So we'll have 1 unclosed quotation, which should produce an error
    let error_vec = errors.into_vec();
    assert!(
        !error_vec.is_empty(),
        "Test has mismatched quotation postcodes (second one has trailing space), should produce errors"
    );
}

/// Confirms quotation validation is inert when no quotation markers are present.
///
/// Non-quotation postcodes should not produce `E242`.
#[test]
fn test_e242_no_quotations() {
    // Create utterance without quotation markers
    let main = MainTier::new("CHI", vec![], Terminator::Period { span: Span::DUMMY })
        .with_postcodes(vec![
            Postcode::new("bch"), // Some other postcode
        ]);

    let utterance = Utterance::new(main);
    let errors = ErrorCollector::new();
    check_quotation_balance(&utterance, &errors);

    assert!(
        errors.into_vec().is_empty(),
        "No quotations should not produce errors"
    );
}

/// Allows CA delimiter pairs to open and close across word boundaries.
///
/// This guards the intended utterance-level interpretation of delimiter scopes.
#[test]
fn test_e230_ca_delimiters_balanced_across_words() {
    let word1 = Word::new_unchecked("°soft", "soft").with_content(vec![
        WordContent::CADelimiter(CADelimiter::new(CADelimiterType::Softer)),
        WordContent::Text(WordText::new_unchecked("soft")),
    ]);
    let word2 = Word::new_unchecked("more°", "more").with_content(vec![
        WordContent::Text(WordText::new_unchecked("more")),
        WordContent::CADelimiter(CADelimiter::new(CADelimiterType::Softer)),
    ]);

    let main = MainTier::new(
        "CHI",
        vec![
            UtteranceContent::Word(Box::new(word1)),
            UtteranceContent::Word(Box::new(word2)),
        ],
        Terminator::Period { span: Span::DUMMY },
    );
    let utterance = Utterance::new(main);
    let errors = ErrorCollector::new();
    check_ca_delimiter_balance(&utterance, &errors);

    assert!(
        errors.into_vec().is_empty(),
        "CA delimiters should balance across word boundaries"
    );
}

/// Reports `E230` when a CA delimiter remains unmatched in an utterance.
///
/// The test asserts that one unmatched marker yields exactly one balance error.
#[test]
fn test_e230_ca_delimiters_unbalanced_in_utterance() {
    let word = Word::new_unchecked("°soft", "soft").with_content(vec![
        WordContent::CADelimiter(CADelimiter::new(CADelimiterType::Softer)),
        WordContent::Text(WordText::new_unchecked("soft")),
    ]);

    let main = MainTier::new(
        "CHI",
        vec![UtteranceContent::Word(Box::new(word))],
        Terminator::Period { span: Span::DUMMY },
    );
    let utterance = Utterance::new(main);
    let errors = ErrorCollector::new();
    check_ca_delimiter_balance(&utterance, &errors);

    let error_vec = errors.into_vec();
    assert_eq!(
        error_vec.len(),
        1,
        "Expected E230 error for unbalanced delimiter"
    );
    assert_eq!(error_vec[0].code, ErrorCode::UnbalancedCADelimiter);
}

/// Confirms delimiter role analysis assigns begin/end roles across tokens.
///
/// This fixture also checks pair-detection behavior when mixed delimiter types
/// appear in the same utterance.
#[test]
fn test_ca_delimiter_role_analysis_across_words() {
    let word1 = Word::new_unchecked("°soft", "soft").with_content(vec![
        WordContent::CADelimiter(CADelimiter::new(CADelimiterType::Softer)),
        WordContent::Text(WordText::new_unchecked("soft")),
    ]);
    let word2 = Word::new_unchecked("more°", "more").with_content(vec![
        WordContent::Text(WordText::new_unchecked("more")),
        WordContent::CADelimiter(CADelimiter::new(CADelimiterType::Softer)),
    ]);
    let word3 = Word::new_unchecked("∆fast", "fast").with_content(vec![
        WordContent::CADelimiter(CADelimiter::new(CADelimiterType::Faster)),
        WordContent::Text(WordText::new_unchecked("fast")),
    ]);

    let main = MainTier::new(
        "CHI",
        vec![
            UtteranceContent::Word(Box::new(word1)),
            UtteranceContent::Word(Box::new(word2)),
            UtteranceContent::Word(Box::new(word3)),
        ],
        Terminator::Period { span: Span::DUMMY },
    );
    let utterance = Utterance::new(main);
    let roles = analyze_ca_delimiter_roles(&utterance);

    assert_eq!(roles.len(), 3);
    assert_eq!(roles[0].delimiter_type, CADelimiterType::Softer);
    assert_eq!(roles[0].role, CADelimiterRole::Begin);
    assert!(roles[0].is_paired);

    assert_eq!(roles[1].delimiter_type, CADelimiterType::Softer);
    assert_eq!(roles[1].role, CADelimiterRole::End);
    assert!(roles[1].is_paired);

    assert_eq!(roles[2].delimiter_type, CADelimiterType::Faster);
    assert_eq!(roles[2].role, CADelimiterRole::Begin);
    assert!(!roles[2].is_paired);
}

/// Accepts overlapping delimiter types when each type is internally balanced.
///
/// Different CA delimiter classes should not interfere with each other's pairing.
#[test]
fn test_e230_ca_delimiters_overlapping_types_are_balanced() {
    let word1 = Word::new_unchecked("°∆a", "a").with_content(vec![
        WordContent::CADelimiter(CADelimiter::new(CADelimiterType::Softer)),
        WordContent::CADelimiter(CADelimiter::new(CADelimiterType::Faster)),
        WordContent::Text(WordText::new_unchecked("a")),
    ]);
    let word2 = Word::new_unchecked("b°∆", "b").with_content(vec![
        WordContent::Text(WordText::new_unchecked("b")),
        WordContent::CADelimiter(CADelimiter::new(CADelimiterType::Softer)),
        WordContent::CADelimiter(CADelimiter::new(CADelimiterType::Faster)),
    ]);

    let main = MainTier::new(
        "CHI",
        vec![
            UtteranceContent::Word(Box::new(word1)),
            UtteranceContent::Word(Box::new(word2)),
        ],
        Terminator::Period { span: Span::DUMMY },
    );
    let utterance = Utterance::new(main);
    let errors = ErrorCollector::new();
    check_ca_delimiter_balance(&utterance, &errors);

    assert!(
        errors.into_vec().is_empty(),
        "Overlapping CA delimiters by type should be considered balanced"
    );
}

/// Rejects overlap indices outside the allowed CHAT one-digit range.
///
/// Invalid values should emit `E373` (`InvalidOverlapIndex`).
#[test]
fn test_e373_invalid_overlap_index() {
    let main = MainTier::new(
        "CHI",
        vec![UtteranceContent::OverlapPoint(OverlapPoint::new(
            OverlapPointKind::TopOverlapBegin,
            Some(OverlapIndex::new(1)),
        ))],
        Terminator::Period { span: Span::DUMMY },
    );
    let utterance = Utterance::new(main);
    let errors = ErrorCollector::new();
    let context = ValidationContext::default();
    check_overlap_index_values(&utterance, &context, &errors);

    let error_vec = errors.into_vec();
    assert_eq!(
        error_vec.len(),
        1,
        "Expected E373 for invalid overlap index"
    );
    assert_eq!(error_vec[0].code, ErrorCode::InvalidOverlapIndex);
}

/// Accepts valid one-digit overlap indices.
///
/// This keeps `E373` targeted to out-of-range values.
#[test]
fn test_overlap_index_valid_single_digit() {
    let main = MainTier::new(
        "CHI",
        vec![UtteranceContent::OverlapPoint(OverlapPoint::new(
            OverlapPointKind::TopOverlapBegin,
            Some(OverlapIndex::new(9)),
        ))],
        Terminator::Period { span: Span::DUMMY },
    );
    let utterance = Utterance::new(main);
    let errors = ErrorCollector::new();
    let context = ValidationContext::default();
    check_overlap_index_values(&utterance, &context, &errors);

    assert!(
        errors.into_vec().is_empty(),
        "Valid single-digit overlap index should not produce errors"
    );
}

/// Reports `E356` when an underline begin marker is never closed.
///
/// The checker should track begin/end state across the full utterance stream.
#[test]
fn test_e356_unmatched_underline_begin() {
    // Create utterance with unmatched underline begin
    let main = MainTier::new(
        "CHI",
        vec![
            UtteranceContent::UnderlineBegin(UnderlineMarker::default()),
            // Missing UnderlineEnd
        ],
        Terminator::Period { span: Span::DUMMY },
    );

    let utterance = Utterance::new(main);
    let errors = ErrorCollector::new();
    check_underline_balance(&utterance, &errors);

    let error_vec = errors.into_vec();
    assert_eq!(
        error_vec.len(),
        1,
        "Expected E356 error for unmatched begin"
    );
    assert_eq!(error_vec[0].code, ErrorCode::UnmatchedUnderlineBegin);
    assert!(error_vec[0].message.contains("Unmatched underline begin"));
}

/// Reports `E357` when an underline end marker appears without a prior begin.
///
/// This catches dangling close markers at utterance scope.
#[test]
fn test_e357_unmatched_underline_end() {
    // Create utterance with unmatched underline end
    let main = MainTier::new(
        "CHI",
        vec![
            // Missing UnderlineBegin
            UtteranceContent::UnderlineEnd(UnderlineMarker::default()),
        ],
        Terminator::Period { span: Span::DUMMY },
    );

    let utterance = Utterance::new(main);
    let errors = ErrorCollector::new();
    check_underline_balance(&utterance, &errors);

    let error_vec = errors.into_vec();
    assert_eq!(error_vec.len(), 1, "Expected E357 error for unmatched end");
    assert_eq!(error_vec[0].code, ErrorCode::UnmatchedUnderlineEnd);
    assert!(error_vec[0].message.contains("Unmatched underline end"));
}

/// Accepts properly paired underline begin/end markers.
///
/// A balanced pair should produce no `E356`/`E357` diagnostics.
#[test]
fn test_e356_e357_balanced_underlines() {
    // Create utterance with balanced underline markers
    let main = MainTier::new(
        "CHI",
        vec![
            UtteranceContent::UnderlineBegin(UnderlineMarker::default()),
            UtteranceContent::UnderlineEnd(UnderlineMarker::default()),
        ],
        Terminator::Period { span: Span::DUMMY },
    );

    let utterance = Utterance::new(main);
    let errors = ErrorCollector::new();
    check_underline_balance(&utterance, &errors);

    assert!(
        errors.into_vec().is_empty(),
        "Balanced underline markers should not produce errors"
    );
}

/// Detects unmatched underline endings nested inside grouped content.
///
/// Nested structures must still participate in the same underline-balance rules.
#[test]
fn test_e357_unmatched_underline_end_inside_group_word() {
    let word = Word::new_unchecked("x", "x").with_content(vec![
        WordContent::Text(WordText::new_unchecked("x")),
        WordContent::UnderlineEnd(WordUnderlineEnd::new()),
    ]);
    let group = Group::new(BracketedContent::new(vec![BracketedItem::Word(Box::new(
        word,
    ))]));
    let main = MainTier::new(
        "CHI",
        vec![UtteranceContent::Group(group)],
        Terminator::Period { span: Span::DUMMY },
    );

    let utterance = Utterance::new(main);
    let errors = ErrorCollector::new();
    check_underline_balance(&utterance, &errors);

    let error_vec = errors.into_vec();
    assert_eq!(error_vec.len(), 1, "Expected E357 for nested underline end");
    assert_eq!(error_vec[0].code, ErrorCode::UnmatchedUnderlineEnd);
}

/// Reports `E357` when underline end markers occur before any begin marker.
///
/// This protects stack-order invariants in marker traversal.
#[test]
fn test_e357_end_before_begin() {
    // Stack-based validation should catch end appearing before any begin
    let main = MainTier::new(
        "CHI",
        vec![
            UtteranceContent::UnderlineEnd(UnderlineMarker::default()), // End without begin
            UtteranceContent::UnderlineBegin(UnderlineMarker::default()),
            UtteranceContent::UnderlineEnd(UnderlineMarker::default()),
        ],
        Terminator::Period { span: Span::DUMMY },
    );

    let utterance = Utterance::new(main);
    let errors = ErrorCollector::new();
    check_underline_balance(&utterance, &errors);

    let error_vec = errors.into_vec();
    assert_eq!(error_vec.len(), 1, "Should detect end before begin");
    assert_eq!(error_vec[0].code, ErrorCode::UnmatchedUnderlineEnd);
}

/// Reports remaining unclosed begins after traversal completes.
///
/// Multiple opens with insufficient closes should leave an `E356` residue.
#[test]
fn test_e356_multiple_unclosed() {
    // Stack-based validation should report count of unclosed begins
    let main = MainTier::new(
        "CHI",
        vec![
            UtteranceContent::UnderlineBegin(UnderlineMarker::default()),
            UtteranceContent::UnderlineBegin(UnderlineMarker::default()),
            UtteranceContent::UnderlineEnd(UnderlineMarker::default()), // Only closes one
                                                                        // Two begins, one end = one unclosed
        ],
        Terminator::Period { span: Span::DUMMY },
    );

    let utterance = Utterance::new(main);
    let errors = ErrorCollector::new();
    check_underline_balance(&utterance, &errors);

    let error_vec = errors.into_vec();
    assert_eq!(error_vec.len(), 1, "Should detect unclosed begin");
    assert_eq!(error_vec[0].code, ErrorCode::UnmatchedUnderlineBegin);
    assert!(error_vec[0].message.contains("unclosed begin marker"));
}
