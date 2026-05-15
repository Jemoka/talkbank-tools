//! Regression test: files with bare timestamps (no \x15 delimiters) must NOT
//! produce E502 (Missing @End header).
//!
//! Background: The %wor grammar change (wor_tier_body rule) caused tree-sitter
//! to wrap entire files with bare timestamps in a top-level ERROR node, making
//! @End invisible to the Rust parser. This was fixed by reverting to tier_body.
//!
//! This test ensures the regression doesn't happen again.

use talkbank_model::ParseValidateOptions;
use talkbank_model::{ErrorCode, ErrorCollector};
use talkbank_transform::parse_and_validate_streaming;

/// Files with bare timestamps (200_1590 instead of \x15200_1590\x15) and %wor tiers
/// must parse as a document with @End found.
#[test]
fn test_bare_timestamps_no_e502() {
    let content = "\
@UTF8
@Begin
@Languages:\teng
@Participants:\tPAR Participant , INV Investigator
@ID:\teng|test|PAR|||||Participant|||
@ID:\teng|test|INV|||||Investigator|||
*INV:\tgoing on in the picture . 200_1590
%wor:\tgoing 200_360 on 360_760 in 760_1060 the 1060_1130 picture 1130_1590 .
%xmor:\tpart|go-PRESP adv|on prep|in det:art|the n|picture .
%xgra:\t1|0|ROOT 2|1|JCT 3|1|JCT 4|5|DET 5|3|POBJ 6|1|PUNCT
*PAR:\tthe boy is tipping stool . 5441_10831
%wor:\tthe 5441_5531 boy 5531_5901 is 5901_7141 tipping 7421_8221 stool 8221_9051 .
%xmor:\tdet:art|the n|boy aux|be&3S part|tip-PRESP n|stool .
%xgra:\t1|2|DET 2|4|SUBJ 3|4|AUX 4|0|ROOT 5|4|OBJ 6|4|PUNCT
*PAR:\tmother is drying dishes . 20476_22156
%wor:\tmother 20476_20906 is 20906_21036 drying 21036_21606 dishes 21606_22156 .
%xmor:\tn|mother aux|be&3S part|dry-PRESP n|dish-PL .
%xgra:\t1|3|SUBJ 2|3|AUX 3|0|ROOT 4|3|OBJ 5|3|PUNCT
*PAR:\twater is overflowing the sink . 23730_26100
%wor:\twater 23730_24140 is 24140_24380 overflowing 24380_25280 the 25280_25320 sink 25320_26100 .
%xmor:\tn|water aux|be&3S over#part|flow-PRESP det:art|the n|sink .
%xgra:\t1|3|SUBJ 2|3|AUX 3|0|ROOT 4|5|DET 5|3|OBJ 6|3|PUNCT
@End
";

    let options = ParseValidateOptions::default().with_alignment();
    let errors = ErrorCollector::new();
    let chat_file = parse_and_validate_streaming(content, options, &errors).unwrap();

    // Must find @End header
    let has_end = chat_file.headers().any(|h| h.name() == "End");
    assert!(
        has_end,
        "Parser must find @End header in files with bare timestamps"
    );

    // Must find all 4 utterances
    let utt_count = chat_file.utterance_count();
    assert_eq!(utt_count, 4, "Expected 4 utterances, found {}", utt_count);

    // Must NOT produce E502
    let error_vec = errors.into_vec();
    let has_e502 = error_vec
        .iter()
        .any(|e| e.code == ErrorCode::MissingEndHeader);
    assert!(!has_e502, "E502 must not appear — file has @End");

    // E316 for bare timestamps is expected (localized, non-fatal)
    let e316_count = error_vec
        .iter()
        .filter(|e| e.code == ErrorCode::UnparsableContent)
        .count();
    assert!(e316_count > 0, "Expected E316 for bare timestamps");
}

/// Same test with a real corpus file if available.
/// Set `PITT_CORPUS_FILE` env var to a Pitt corpus .cha file to enable.
#[test]
fn test_real_pitt_file_no_e502() {
    let path = match std::env::var("PITT_CORPUS_FILE") {
        Ok(p) => p,
        Err(_) => {
            eprintln!("PITT_CORPUS_FILE not set, skipping real corpus test");
            return;
        }
    };
    if !std::path::Path::new(&path).exists() {
        eprintln!("Skipping: {} not found", path);
        return;
    }

    let content = std::fs::read_to_string(path).unwrap();
    let options = ParseValidateOptions::default().with_alignment();
    let errors = ErrorCollector::new();
    let chat_file = parse_and_validate_streaming(&content, options, &errors).unwrap();

    let has_end = chat_file.headers().any(|h| h.name() == "End");
    assert!(has_end, "Parser must find @End in real Pitt corpus file");

    let error_vec = errors.into_vec();
    let has_e502 = error_vec
        .iter()
        .any(|e| e.code == ErrorCode::MissingEndHeader);
    assert!(!has_e502, "E502 must not appear for real Pitt corpus file");
}
