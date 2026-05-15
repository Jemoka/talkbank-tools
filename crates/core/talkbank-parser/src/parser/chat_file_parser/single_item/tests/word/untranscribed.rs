//! Test module for untranscribed in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

use crate::model::content::word::UntranscribedStatus;
use talkbank_model::ParseErrors;

use super::helpers::parse_word;

/// Verifies canonical untranscribed tokens map to the expected status variants.
#[test]
fn untranscribed_markers_are_tagged() -> Result<(), ParseErrors> {
    let cases = [
        ("xxx", UntranscribedStatus::Unintelligible),
        ("yyy", UntranscribedStatus::Phonetic),
        ("www", UntranscribedStatus::Untranscribed),
    ];

    for (token, status) in cases {
        let expected = status;
        let word = parse_word(token)?;
        assert_eq!(
            word.untranscribed(),
            Some(expected),
            "expected {} to carry {:?}",
            token,
            expected
        );
    }

    let normal = parse_word("hello")?;
    assert_eq!(normal.untranscribed(), None);
    Ok(())
}

/// Verifies prosodic lengthening does not change untranscribed-token classification.
#[test]
fn untranscribed_with_lengthening() -> Result<(), ParseErrors> {
    let word = parse_word("xxx:")?;
    assert_eq!(
        word.untranscribed(),
        Some(UntranscribedStatus::Unintelligible),
        "xxx: should be detected as untranscribed (lengthening is prosodic, not lexical)"
    );
    assert_eq!(word.cleaned_text(), "xxx");
    Ok(())
}
