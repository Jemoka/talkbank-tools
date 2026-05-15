//! Inject `%xalign` and `%xrev` review tiers into CHAT files.
//!
//! `%xalign` records machine decisions (structured key=value pairs).
//! `%xrev` holds the human review marker (`[?]` for unreviewed).
//!
//! Together they make aligned CHAT files self-documenting: reviewers see
//! which utterances need attention and what the algorithm did, then fill in
//! a one-word rating by changing `[?]` to `[ok]`, `[early]`, `[late]`, etc.

use talkbank_model::Span;
use talkbank_model::model::{
    ChatFile, DependentTier, Line, NonEmptyString, UserDefinedDependentTier,
};

use super::repair::RepairDecision;
pub use talkbank_transform::decisions::ReviewLevel;

/// Inject `%xalign` and `%xrev` tiers based on repair decisions.
///
/// For each decision in `decisions`:
/// - Always adds a `%xalign` tier with the machine's reasoning.
/// - If `decision.needs_review` is true, adds a `%xrev: [?]` tier.
///
/// When `review_level` is `All`, also adds `%xalign` informational tiers
/// on bulleted utterances that had no decisions (clean alignment).
pub fn inject_review_tiers(
    chat_file: &mut ChatFile,
    decisions: &[RepairDecision],
    review_level: ReviewLevel,
) {
    if review_level == ReviewLevel::None {
        return;
    }

    // Strip existing %xalign / %xrev tiers so that re-running FA on a file
    // that already has review tiers replaces them rather than accumulating a
    // second set.  Delegates to the shared helper in `decisions.rs`.
    talkbank_transform::decisions::strip_decision_tiers(chat_file);

    // Index decisions by line_idx for O(1) lookup.
    let decision_map: std::collections::HashMap<usize, Vec<&RepairDecision>> = {
        let mut map: std::collections::HashMap<usize, Vec<&RepairDecision>> =
            std::collections::HashMap::new();
        for d in decisions {
            map.entry(d.line_idx).or_default().push(d);
        }
        map
    };

    for (line_idx, line) in chat_file.lines.iter_mut().enumerate() {
        let Line::Utterance(utt) = line else {
            continue;
        };

        if let Some(decisions_for_utt) = decision_map.get(&line_idx) {
            // This utterance had repair decisions — always add %xalign + %xrev.
            for decision in decisions_for_utt {
                utt.dependent_tiers
                    .push(make_user_tier("xalign", &decision.reason));

                if decision.needs_review {
                    utt.dependent_tiers.push(make_user_tier("xrev", "[?]"));
                }
            }
        } else if review_level == ReviewLevel::All {
            // No decisions, but --review-level=all: add informational %xalign.
            let has_bullet = utt.main.content.bullet.is_some();
            if has_bullet {
                utt.dependent_tiers
                    .push(make_user_tier("xalign", "fa_aligned no_repair_needed"));
            }
        }
    }
}

/// Construct a `DependentTier::UserDefined` with the given label and content.
/// # Safety (panic-freedom)
///
/// All call sites pass compile-time string literals that are visibly non-empty
/// (`"xalign"`, `"fa_aligned ..."`, etc.), so `NonEmptyString::new` cannot fail.
#[allow(clippy::expect_used)]
fn make_user_tier(label: &str, content: &str) -> DependentTier {
    DependentTier::UserDefined(UserDefinedDependentTier {
        label: NonEmptyString::new(label).expect("tier label must be non-empty"),
        content: NonEmptyString::new(content).expect("tier content must be non-empty"),
        span: Span::default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inject_low_confidence_only() {
        let chat_text = "\
@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Target_Child
@ID:\teng|test|CHI|2;0.0||||Target_Child|||
*CHI:\thello . \u{0015}1000_2000\u{0015}
*CHI:\tworld . \u{0015}4000_5000\u{0015}
@End
";
        let parser = talkbank_transform::parse::TreeSitterParser::new().expect("parser init");
        let (mut chat_file, _errors) = talkbank_transform::parse::parse_lenient(&parser, chat_text);

        let decisions = vec![RepairDecision {
            line_idx: 5, // first utterance line
            speaker: "CHI".to_string(),
            strategy: talkbank_transform::decisions::FaStrategy::GapFilled,
            reason: "gap_filled gap=500ms same_speaker machine=1000_2000 snapped_start=500"
                .to_string(),
            needs_review: true,
        }];

        inject_review_tiers(&mut chat_file, &decisions, ReviewLevel::LowConfidence);

        // Serialize and check for %xalign and %xrev.
        let output = talkbank_transform::serialize::to_chat_string(&chat_file);
        assert!(
            output.contains("%xalign:"),
            "should contain %xalign tier:\n{output}"
        );
        assert!(
            output.contains("%xrev:"),
            "should contain %xrev tier:\n{output}"
        );
        assert!(
            output.contains("[?]"),
            "should contain [?] marker:\n{output}"
        );
        assert!(
            output.contains("gap_filled"),
            "should contain reason:\n{output}"
        );
    }

    #[test]
    fn test_inject_none_level() {
        let chat_text = "\
@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Target_Child
@ID:\teng|test|CHI|2;0.0||||Target_Child|||
*CHI:\thello . \u{0015}1000_2000\u{0015}
@End
";
        let parser = talkbank_transform::parse::TreeSitterParser::new().expect("parser init");
        let (mut chat_file, _errors) = talkbank_transform::parse::parse_lenient(&parser, chat_text);

        let decisions = vec![RepairDecision {
            line_idx: 5,
            speaker: "CHI".to_string(),
            strategy: talkbank_transform::decisions::FaStrategy::GapFilled,
            reason: "gap_filled test".to_string(),
            needs_review: true,
        }];

        inject_review_tiers(&mut chat_file, &decisions, ReviewLevel::None);

        let output = talkbank_transform::serialize::to_chat_string(&chat_file);
        assert!(
            !output.contains("%xalign:"),
            "ReviewLevel::None should not emit %xalign:\n{output}"
        );
        assert!(
            !output.contains("%xrev:"),
            "ReviewLevel::None should not emit %xrev:\n{output}"
        );
    }
}
