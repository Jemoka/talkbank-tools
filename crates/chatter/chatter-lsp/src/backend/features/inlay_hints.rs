//! Inlay hint annotations for alignment mismatches near `%mor`/`%gra`.
//!
//! Generates inline annotations that the editor renders alongside the source
//! text, surfacing alignment count mismatches (e.g. 3 main-tier words vs 4
//! `%mor` items) without requiring the user to hover or run a separate command.
//! Each hint carries a tooltip explaining the mismatch.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use talkbank_model::alignment::traits::{IndexPair, TierAlignmentResult};
use tower_lsp::lsp_types::*;

use crate::backend::utils::LineIndex;

/// Inlay hint label — compact side-left and side-right counts with a role name.
struct MismatchCounts {
    /// Count of pairs where the source side is present. For `%mor` alignment
    /// this is the number of main-tier alignable items; for `%gra` alignment
    /// it is the number of `%mor` chunks (clitic-expanded).
    source: usize,
    /// Count of pairs where the target side is present: `%mor` items or `%gra`
    /// relations respectively.
    target: usize,
}

impl MismatchCounts {
    /// Derive counts from the canonical alignment pairs. The alignment
    /// accumulator records one pair per index on whichever side is longer, with
    /// `None` on the short side — so presence counts give the original tier
    /// cardinalities without re-tokenising the tier.
    fn from_pairs<P: IndexPair>(pairs: &[P]) -> Self {
        let source = pairs.iter().filter(|p| p.source().is_some()).count();
        let target = pairs.iter().filter(|p| p.target().is_some()).count();
        Self { source, target }
    }
}

/// Produce one inlay hint per utterance whose dependent-tier alignment is not
/// error-free, using the canonical `talkbank-model` alignment result as the
/// single source of truth for what counts as a mismatch.
///
/// The LSP never does its own tier counting: the canonical validator already
/// understands clitic expansion (`it's` → two `%gra` indices but one joined
/// `%mor` token), terminator matching, and other domain subtleties, and it
/// stores its verdict on `utterance.alignments`. If the verdict is
/// `is_error_free()`, no hint is emitted — otherwise the hint surfaces the
/// canonical pair counts that the model itself used.
pub fn generate_alignment_hints(
    chat_file: &talkbank_model::model::ChatFile,
    text: &str,
    range: Range,
) -> Vec<InlayHint> {
    let mut hints = Vec::new();
    let index = LineIndex::new(text);

    for utterance in chat_file.utterances() {
        let main_line = index
            .offset_to_position(text, utterance.main.span.start)
            .line;
        if main_line < range.start.line || main_line > range.end.line {
            continue;
        }

        let Some(alignments) = utterance.alignments.as_ref() else {
            continue;
        };

        if let Some(mor_result) = alignments.mor.as_ref()
            && !mor_result.is_error_free()
        {
            let counts = MismatchCounts::from_pairs(mor_result.pairs());
            hints.push(alignment_hint(
                index.offset_to_position(text, utterance.main.span.end),
                format!(
                    " [alignment: {} main ↔ {} mor]",
                    counts.source, counts.target
                ),
                "Main tier and %mor tier do not align. See the diagnostic for details.",
            ));
        }

        if let Some(gra_result) = alignments.gra.as_ref()
            && !gra_result.is_error_free()
            && let Some(gra_tier) = utterance.gra_tier()
        {
            let gra_line = index.offset_to_position(text, gra_tier.span.start).line;
            if gra_line < range.start.line || gra_line > range.end.line {
                continue;
            }
            let counts = MismatchCounts::from_pairs(gra_result.pairs());
            hints.push(alignment_hint(
                index.offset_to_position(text, gra_tier.span.end),
                format!(
                    " [alignment: {} gra ↔ {} mor]",
                    counts.target, counts.source
                ),
                "%gra relations and %mor chunks do not align. See the diagnostic for details.",
            ));
        }
    }

    hints
}

/// Build an inlay hint at `position` with the given inline label and tooltip.
/// Centralised so both the `%mor` and `%gra` branches emit visually identical
/// hints.
fn alignment_hint(position: Position, label: String, tooltip: &str) -> InlayHint {
    InlayHint {
        position,
        label: InlayHintLabel::String(label),
        kind: Some(InlayHintKind::PARAMETER),
        text_edits: None,
        tooltip: Some(InlayHintTooltip::String(tooltip.to_string())),
        padding_left: Some(true),
        padding_right: None,
        data: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_model::model::Line;
    use talkbank_parser::TreeSitterParser;

    fn parse_and_align(input: &str) -> talkbank_model::model::ChatFile {
        let parser = TreeSitterParser::new().unwrap();
        let mut chat_file = parser.parse_chat_file(input).unwrap();
        for line in &mut chat_file.lines {
            if let Line::Utterance(utterance) = line {
                utterance.compute_alignments_default();
            }
        }
        chat_file
    }

    fn full_range() -> Range {
        Range {
            start: Position {
                line: 0,
                character: 0,
            },
            end: Position {
                line: 1000,
                character: 0,
            },
        }
    }

    #[test]
    fn no_hints_when_aligned() {
        let input = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n@ID:\teng|corpus|CHI|||||Child|||\n*CHI:\thello world .\n%mor:\tn|hello n|world .\n@End\n";
        let chat_file = parse_and_align(input);
        let hints = generate_alignment_hints(&chat_file, input, full_range());
        // When counts match, no hint should be generated
        assert!(hints.is_empty());
    }

    #[test]
    fn hint_on_mor_count_mismatch() {
        // 3 main-tier words but only 2 mor items → mismatch
        let input = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n@ID:\teng|corpus|CHI|||||Child|||\n*CHI:\thello big world .\n%mor:\tn|hello n|world .\n@End\n";
        let chat_file = parse_and_align(input);
        let hints = generate_alignment_hints(&chat_file, input, full_range());
        assert_eq!(hints.len(), 1);
        if let InlayHintLabel::String(label) = &hints[0].label {
            assert!(label.contains("3 main"));
            assert!(label.contains("2 mor"));
        } else {
            panic!("Expected string label");
        }
    }

    /// Clitic forms like `it's` expand to two `%gra` indices while remaining a
    /// single joined token in `%mor` (`pron|it~aux|be-...`). The canonical
    /// alignment validator in `talkbank-model` accepts this as correctly
    /// aligned; the LSP inlay-hint counter must agree, or users see spurious
    /// `[alignment: N gra ↔ M mor]` hints on valid reference-corpus files
    /// (regression surfaced on `resources/corpus/reference/tiers/mor-gra.cha`, utterance 1).
    #[test]
    fn no_hint_for_clitic_expansion_between_gra_and_mor() {
        let input = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n@ID:\teng|corpus|CHI|3;00.||||Child|||\n*CHI:\tit's I want cookies .\n%mor:\tpron|it~aux|be-Fin-Ind-Pres-S3 pron|I-Prs-Nom-S1 verb|want-Fin-Ind-Pres noun|cookie-Plur .\n%gra:\t1|4|NSUBJ 2|1|AUX 3|4|NSUBJ 4|0|ROOT 5|4|OBJ 6|4|PUNCT\n@End\n";
        let chat_file = parse_and_align(input);
        let hints = generate_alignment_hints(&chat_file, input, full_range());
        let gra_mor_hints: Vec<_> = hints
            .iter()
            .filter_map(|h| match &h.label {
                InlayHintLabel::String(s) if s.contains("gra") && s.contains("mor") => {
                    Some(s.as_str())
                }
                _ => None,
            })
            .collect();
        assert!(
            gra_mor_hints.is_empty(),
            "clitic expansion is semantically aligned; expected no gra↔mor hint, got: {:?}",
            gra_mor_hints,
        );
    }

    #[test]
    fn hints_filtered_by_range() {
        let input = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n@ID:\teng|corpus|CHI|||||Child|||\n*CHI:\thello world .\n%mor:\tn|hello n|world adj|big .\n@End\n";
        let chat_file = parse_and_align(input);
        // Request only lines 0-1 — utterance is further down
        let narrow = Range {
            start: Position {
                line: 0,
                character: 0,
            },
            end: Position {
                line: 1,
                character: 0,
            },
        };
        let hints = generate_alignment_hints(&chat_file, input, narrow);
        assert!(hints.is_empty()); // Utterance is outside the requested range
    }
}
