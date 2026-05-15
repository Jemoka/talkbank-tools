//! Regression tests for `%mor`/`%gra` DOT graph generation.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use super::{DependencyGraphResponse, build_dependency_graph_response, generate_dot_graph};
use crate::backend::{LspBackendError, ParseState, TierName};

/// Tests missing alignment metadata.
#[test]
fn test_missing_alignment_metadata() {
    let main_tier = talkbank_model::model::MainTier::new("CHI", vec![], None);
    let mor_tier = talkbank_model::model::MorTier::new_mor(
        vec![],
        talkbank_model::Terminator::Period {
            span: talkbank_model::Span::DUMMY,
        },
    );
    let gra_tier = talkbank_model::model::GraTier::new_gra(vec![]);

    let utterance = talkbank_model::model::Utterance::new(main_tier)
        .with_mor(mor_tier)
        .with_gra(gra_tier);

    let result = generate_dot_graph(&utterance, ParseState::Clean);
    assert!(
        matches!(result, Err(LspBackendError::AlignmentMetadataMissing)),
        "expected AlignmentMetadataMissing, got {result:?}",
    );
}

/// Tests missing gra alignment.
#[test]
fn test_missing_gra_alignment() {
    let main_tier = talkbank_model::model::MainTier::new("CHI", vec![], None);
    let mor_tier = talkbank_model::model::MorTier::new_mor(
        vec![],
        talkbank_model::Terminator::Period {
            span: talkbank_model::Span::DUMMY,
        },
    );
    let gra_tier = talkbank_model::model::GraTier::new_gra(vec![]);

    let mut utterance = talkbank_model::model::Utterance::new(main_tier)
        .with_mor(mor_tier)
        .with_gra(gra_tier);

    utterance.alignments = Some(talkbank_model::model::AlignmentSet::new(
        talkbank_model::model::AlignmentUnits::default(),
    ));

    let result = generate_dot_graph(&utterance, ParseState::Clean);
    assert!(
        matches!(
            result,
            Err(LspBackendError::TierAlignmentMissing {
                tier: TierName::Gra,
            })
        ),
        "expected TierAlignmentMissing {{ tier: Gra }}, got {result:?}",
    );
}

/// On `ParseState::StaleBaseline`, the DOT header gains a muted
/// top-left `stale baseline` label so the viewer can tell the
/// dependency graph was computed against the last successful parse
/// (KIB-013). On `ParseState::Clean` the same label must be absent.
#[test]
fn stale_baseline_emits_muted_label_attrs() -> Result<(), String> {
    let main_tier = talkbank_model::model::MainTier::new("CHI", vec![], None);
    let word = talkbank_model::model::MorWord::new("n", "cat");
    let mor = talkbank_model::model::Mor::new(word);
    let mor_tier = talkbank_model::model::MorTier::new_mor(
        vec![mor],
        talkbank_model::Terminator::Period {
            span: talkbank_model::Span::DUMMY,
        },
    );
    let gra_tier = talkbank_model::model::GraTier::new_gra(vec![
        talkbank_model::model::GrammaticalRelation::new(1, 0, "ROOT"),
    ]);

    use talkbank_model::alignment::{GraAlignment, GraAlignmentPair};
    let gra_alignment = GraAlignment::new().with_pair(GraAlignmentPair::from_raw(Some(0), Some(0)));
    let mut alignment_metadata =
        talkbank_model::model::AlignmentSet::new(talkbank_model::model::AlignmentUnits::default());
    alignment_metadata.gra = Some(gra_alignment);

    let mut utterance = talkbank_model::model::Utterance::new(main_tier)
        .with_mor(mor_tier)
        .with_gra(gra_tier);
    utterance.alignments = Some(alignment_metadata);

    let clean =
        generate_dot_graph(&utterance, ParseState::Clean).map_err(|err| format!("clean: {err}"))?;
    assert!(
        !clean.contains("stale baseline"),
        "Clean parse state must not emit the stale-baseline marker"
    );

    let stale = generate_dot_graph(&utterance, ParseState::StaleBaseline)
        .map_err(|err| format!("stale: {err}"))?;
    assert!(
        stale.contains("label=\"stale baseline\""),
        "StaleBaseline must emit the marker label; got:\n{stale}"
    );
    assert!(
        stale.contains("fontcolor=\"#888888\""),
        "marker must use the muted fontcolor"
    );
    assert!(
        stale.contains("fontname=\"Courier\""),
        "marker must use Courier to signal meta-information"
    );
    Ok(())
}

/// Tests dot format structure with alignment.
#[test]
fn test_dot_format_structure_with_alignment() -> Result<(), String> {
    let main_tier = talkbank_model::model::MainTier::new("CHI", vec![], None);

    let word = talkbank_model::model::MorWord::new("n", "cat");
    let mor = talkbank_model::model::Mor::new(word);
    let mor_tier = talkbank_model::model::MorTier::new_mor(
        vec![mor],
        talkbank_model::Terminator::Period {
            span: talkbank_model::Span::DUMMY,
        },
    );

    let gra_tier = talkbank_model::model::GraTier::new_gra(vec![
        talkbank_model::model::GrammaticalRelation::new(1, 0, "ROOT"),
    ]);

    use talkbank_model::alignment::{GraAlignment, GraAlignmentPair};

    let gra_alignment = GraAlignment::new().with_pair(GraAlignmentPair::from_raw(Some(0), Some(0)));

    let mut alignment_metadata =
        talkbank_model::model::AlignmentSet::new(talkbank_model::model::AlignmentUnits::default());
    alignment_metadata.gra = Some(gra_alignment);

    let mut utterance = talkbank_model::model::Utterance::new(main_tier)
        .with_mor(mor_tier)
        .with_gra(gra_tier);
    utterance.alignments = Some(alignment_metadata);

    let dot = generate_dot_graph(&utterance, ParseState::Clean)
        .map_err(|err| format!("Failed to generate graph: {err}"))?;

    assert!(dot.contains("digraph utterance"));
    assert!(dot.contains("rankdir=LR"));
    assert!(dot.contains("cat"));
    assert!(
        dot.contains("1 -> 0"),
        "Expected edge from node 1 to ROOT (0)"
    );
    assert!(
        dot.contains("label=\"ROOT\""),
        "Expected ROOT label on edge"
    );
    Ok(())
}

/// A request for an utterance with no `%mor` tier returns a typed `Unavailable`
/// variant, so the extension can distinguish "no graph to render" from actual
/// DOT syntax. The previous string-valued API collapsed both into one field and
/// caused the Graphviz renderer to choke on the reason text.
#[test]
fn response_is_unavailable_when_no_mor_tier() {
    let main_tier = talkbank_model::model::MainTier::new("CHI", vec![], None);
    let utterance = talkbank_model::model::Utterance::new(main_tier);

    let response = build_dependency_graph_response(&utterance, ParseState::Clean);

    match response {
        DependencyGraphResponse::Unavailable { reason } => {
            assert!(
                reason.contains("%mor"),
                "reason should mention the missing %mor tier; got: {reason}",
            );
        }
        DependencyGraphResponse::Dot { .. } => {
            panic!("expected Unavailable variant when %mor tier is missing");
        }
    }
}

/// A fully aligned utterance produces a `Dot` variant carrying the Graphviz
/// source — never a bare string conflated with error text.
#[test]
fn response_is_dot_when_aligned() {
    let main_tier = talkbank_model::model::MainTier::new("CHI", vec![], None);
    let word = talkbank_model::model::MorWord::new("n", "cat");
    let mor = talkbank_model::model::Mor::new(word);
    let mor_tier = talkbank_model::model::MorTier::new_mor(
        vec![mor],
        talkbank_model::Terminator::Period {
            span: talkbank_model::Span::DUMMY,
        },
    );
    let gra_tier = talkbank_model::model::GraTier::new_gra(vec![
        talkbank_model::model::GrammaticalRelation::new(1, 0, "ROOT"),
    ]);

    use talkbank_model::alignment::{GraAlignment, GraAlignmentPair};
    let gra_alignment = GraAlignment::new().with_pair(GraAlignmentPair::from_raw(Some(0), Some(0)));
    let mut alignment_metadata =
        talkbank_model::model::AlignmentSet::new(talkbank_model::model::AlignmentUnits::default());
    alignment_metadata.gra = Some(gra_alignment);

    let mut utterance = talkbank_model::model::Utterance::new(main_tier)
        .with_mor(mor_tier)
        .with_gra(gra_tier);
    utterance.alignments = Some(alignment_metadata);

    match build_dependency_graph_response(&utterance, ParseState::Clean) {
        DependencyGraphResponse::Dot { source } => {
            assert!(
                source.contains("digraph"),
                "DOT source missing digraph header"
            );
        }
        DependencyGraphResponse::Unavailable { reason } => {
            panic!("expected Dot variant; got Unavailable({reason})");
        }
    }
}

/// The response serialises to a JSON discriminant object so the TS client can
/// branch on `kind` without parsing free-form text.
#[test]
fn response_serializes_with_kind_discriminant() {
    let dot = DependencyGraphResponse::Dot {
        source: "digraph {}".to_string(),
    };
    let value = serde_json::to_value(&dot).expect("dot response should serialize");
    assert_eq!(value["kind"], "dot");
    assert_eq!(value["source"], "digraph {}");

    let unavailable = DependencyGraphResponse::Unavailable {
        reason: "No %mor tier found".to_string(),
    };
    let value = serde_json::to_value(&unavailable).expect("unavailable response should serialize");
    assert_eq!(value["kind"], "unavailable");
    assert_eq!(value["reason"], "No %mor tier found");
}

/// Dependency edges must address the `%mor` **chunk** sequence, so a
/// post-clitic gets its own graph node and relations referring to it
/// connect through that node — not through the node of the next `%mor`
/// item. For `pron|it~aux|be n|cookie .` aligned with
/// `1|2|SUBJ 2|0|ROOT 3|2|OBJ 4|2|PUNCT`, node IDs follow chunk order:
///
/// | node_id | chunk_idx | label   |
/// |---------|-----------|---------|
/// | 1       | 0         | `it`    |
/// | 2       | 1         | `be`    |  ← post-clitic of `it's` item
/// | 3       | 2         | `cookie`|
/// | 4       | 3         | `.`     |
///
/// The test pins the four edges so any regression (e.g. reverting the
/// `labels.rs` walk to an items-only loop, or making `edges.rs` treat the
/// chunk index as an item index) shows up as a failed containment check.
#[test]
fn dependency_edges_with_post_clitic_connect_correct_chunks() -> Result<(), String> {
    let main_tier = talkbank_model::model::MainTier::new("CHI", vec![], None);

    let its = talkbank_model::model::Mor::new(talkbank_model::model::MorWord::new("pron", "it"))
        .with_post_clitic(talkbank_model::model::MorWord::new("aux", "be"));
    let cookie =
        talkbank_model::model::Mor::new(talkbank_model::model::MorWord::new("n", "cookie"));
    let mor_tier = talkbank_model::model::MorTier::new_mor(
        vec![its, cookie],
        talkbank_model::Terminator::Period {
            span: talkbank_model::Span::DUMMY,
        },
    );

    let gra_tier = talkbank_model::model::GraTier::new_gra(vec![
        talkbank_model::model::GrammaticalRelation::new(1, 2, "SUBJ"),
        talkbank_model::model::GrammaticalRelation::new(2, 0, "ROOT"),
        talkbank_model::model::GrammaticalRelation::new(3, 2, "OBJ"),
        talkbank_model::model::GrammaticalRelation::new(4, 2, "PUNCT"),
    ]);

    use talkbank_model::alignment::{GraAlignment, GraAlignmentPair};
    let gra_alignment = GraAlignment::new()
        .with_pair(GraAlignmentPair::from_raw(Some(0), Some(0)))
        .with_pair(GraAlignmentPair::from_raw(Some(1), Some(1)))
        .with_pair(GraAlignmentPair::from_raw(Some(2), Some(2)))
        .with_pair(GraAlignmentPair::from_raw(Some(3), Some(3)));

    let mut alignment_metadata =
        talkbank_model::model::AlignmentSet::new(talkbank_model::model::AlignmentUnits::default());
    alignment_metadata.gra = Some(gra_alignment);

    let mut utterance = talkbank_model::model::Utterance::new(main_tier)
        .with_mor(mor_tier)
        .with_gra(gra_tier);
    utterance.alignments = Some(alignment_metadata);

    let dot = generate_dot_graph(&utterance, ParseState::Clean)
        .map_err(|err| format!("generate: {err}"))?;

    // Every chunk, including the post-clitic `be`, gets its own node label.
    for lemma in ["it", "be", "cookie"] {
        assert!(
            dot.contains(lemma),
            "expected DOT to label every chunk ({lemma}); got:\n{dot}"
        );
    }

    // The critical edges: `1 -> 2 SUBJ` and `3 -> 2 OBJ` and `4 -> 2 PUNCT`
    // all terminate at node 2 (the `be` post-clitic). If edge generation
    // ever confused chunk with item indices, `2` would collapse into the
    // next item's node and these would read `-> 3` or `-> 1` instead.
    for (edge, relation) in [
        ("1 -> 2", "SUBJ"),
        ("2 -> 0", "ROOT"),
        ("3 -> 2", "OBJ"),
        ("4 -> 2", "PUNCT"),
    ] {
        let expected = format!("{edge} [label=\"{relation}\"");
        assert!(
            dot.contains(&expected),
            "expected edge `{expected}`; DOT was:\n{dot}"
        );
    }

    Ok(())
}
