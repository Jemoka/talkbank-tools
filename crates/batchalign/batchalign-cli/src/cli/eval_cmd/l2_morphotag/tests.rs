//! Unit tests for the L2-morphotag eval analyzer.
//!
//! Tests cover:
//! - Language-marker parsing and effective-language resolution
//! - Splice-status classification
//! - Each heuristic detector in isolation
//! - End-to-end: `analyze_file` on a phrasal-verb fixture
//! - End-to-end: aggregation across two files with the same pair key
//! - The architectural win: an AST-first walker pairs `@s` correctly
//!   across a group retrace where the Python regex analyzer mis-counts.
//!
//! Fixtures are minimal hand-written CHAT strings — just enough to
//! exercise one behavior per test. No filesystem searches for "small
//! files" (per the workspace CLAUDE.md rule 19).

use std::path::{Path, PathBuf};

use talkbank_model::model::{LanguageCode, PosCategory};

use super::analysis::{analyze_file, extract_gra_deprel, extract_pos_lemma_features};
use super::heuristics::flags_for;
use super::report::aggregate_by_pair;
use super::types::{
    AtSAnalysis, AtSOccurrence, AtSStatus, FeatureSet, HeuristicFlag, LanguageMarkerKind,
    MorItemText, PairKey, SurfaceWord, classify_status,
};

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

fn langs(codes: &[&str]) -> Vec<LanguageCode> {
    codes.iter().map(|c| LanguageCode::new(*c)).collect()
}

fn write_fixture(contents: &str) -> (tempfile::TempDir, PathBuf) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let path = tmp.path().join("fixture.cha");
    std::fs::write(&path, contents).expect("write fixture");
    (tmp, path)
}

fn make_analysis(
    surface: &str,
    effective_lang: &str,
    pos: Option<&str>,
    mor_item: Option<&str>,
    features: Option<&str>,
    status: AtSStatus,
) -> AtSAnalysis {
    let occ = AtSOccurrence {
        file: PathBuf::from("test.cha"),
        pair_key: PairKey::new(format!("{effective_lang},eng")),
        marker: LanguageMarkerKind::Bare,
        effective_lang: LanguageCode::new(effective_lang),
        surface: SurfaceWord::new(surface),
        mor_position: 0,
        mor_item: mor_item.map(MorItemText::new),
        gra_item: None,
    };
    AtSAnalysis {
        occurrence: occ,
        pos: pos.map(PosCategory::new),
        lemma: Some(surface.to_string()),
        features: features.map(FeatureSet::new),
        gra_deprel: None,
        status,
        flags: Vec::new(),
    }
}

// ---------------------------------------------------------------------------
// Language-marker resolution
// ---------------------------------------------------------------------------

#[test]
fn bare_marker_resolves_to_secondary_language_in_bilingual_header() {
    let marker = LanguageMarkerKind::Bare;
    assert_eq!(
        marker.effective_language(&langs(&["deu", "eng"])),
        Some(LanguageCode::new("eng"))
    );
}

#[test]
fn bare_marker_falls_back_to_primary_in_monolingual() {
    let marker = LanguageMarkerKind::Bare;
    assert_eq!(
        marker.effective_language(&langs(&["eng"])),
        Some(LanguageCode::new("eng"))
    );
}

#[test]
fn bare_marker_returns_none_for_empty_header() {
    let marker = LanguageMarkerKind::Bare;
    assert_eq!(marker.effective_language(&[]), None);
}

#[test]
fn explicit_marker_overrides_header() {
    let marker = LanguageMarkerKind::Explicit(langs(&["spa"]));
    assert_eq!(
        marker.effective_language(&langs(&["deu", "eng"])),
        Some(LanguageCode::new("spa"))
    );
}

#[test]
fn multi_code_explicit_marker_takes_first_code() {
    let marker = LanguageMarkerKind::Explicit(langs(&["eng", "fra"]));
    assert_eq!(
        marker.effective_language(&langs(&["deu", "eng"])),
        Some(LanguageCode::new("eng"))
    );
}

// ---------------------------------------------------------------------------
// Status classification
// ---------------------------------------------------------------------------

#[test]
fn status_spliced_for_normal_mor_item() {
    let mor = MorItemText::new("verb|wake-Fin-Imp-S");
    assert_eq!(classify_status(Some(&mor)), AtSStatus::Spliced);
}

#[test]
fn status_l2xxx_for_l2_fallback() {
    let mor = MorItemText::new("L2|xxx");
    assert_eq!(classify_status(Some(&mor)), AtSStatus::L2Xxx);
}

#[test]
fn status_missing_for_absent_mor_item() {
    assert_eq!(classify_status(None), AtSStatus::MissingMor);
}

// ---------------------------------------------------------------------------
// Serialized-form helpers (kept for external callers / ports)
// ---------------------------------------------------------------------------

#[test]
fn extract_pos_lemma_features_from_plain_item() {
    let (pos, lemma, feats) = extract_pos_lemma_features("verb|wake-Fin-Imp-S");
    assert_eq!(pos.as_deref(), Some("verb"));
    assert_eq!(lemma.as_deref(), Some("wake"));
    assert_eq!(feats.as_deref(), Some("Fin-Imp-S"));
}

#[test]
fn extract_pos_lemma_features_no_features() {
    let (pos, lemma, feats) = extract_pos_lemma_features("part|up");
    assert_eq!(pos.as_deref(), Some("part"));
    assert_eq!(lemma.as_deref(), Some("up"));
    assert_eq!(feats, None);
}

#[test]
fn extract_pos_lemma_features_clitic_uses_head() {
    let (pos, lemma, _) = extract_pos_lemma_features("pron|it~aux|be");
    assert_eq!(pos.as_deref(), Some("pron"));
    assert_eq!(lemma.as_deref(), Some("it"));
}

#[test]
fn extract_pos_lemma_features_handles_l2_fallback() {
    let (pos, _, _) = extract_pos_lemma_features("L2|xxx");
    assert_eq!(pos.as_deref(), Some("L2"));
}

#[test]
fn extract_gra_deprel_parses_valid_triple() {
    let d = extract_gra_deprel("4|3|COMPOUND-PRT").expect("deprel");
    assert_eq!(d.as_str(), "COMPOUND-PRT");
    let root = extract_gra_deprel("1|0|ROOT").expect("root");
    assert_eq!(root.as_str(), "ROOT");
}

#[test]
fn extract_gra_deprel_rejects_malformed() {
    assert!(extract_gra_deprel("malformed").is_none());
    assert!(extract_gra_deprel("").is_none());
}

// ---------------------------------------------------------------------------
// Heuristics
// ---------------------------------------------------------------------------

#[test]
fn heuristic_l2xxx_short_circuits() {
    let a = make_analysis("foo", "eng", None, Some("L2|xxx"), None, AtSStatus::L2Xxx);
    assert_eq!(flags_for(&a), vec![HeuristicFlag::L2Xxx]);
}

#[test]
fn heuristic_missing_mor_short_circuits() {
    let a = make_analysis("foo", "eng", None, None, None, AtSStatus::MissingMor);
    assert_eq!(flags_for(&a), vec![HeuristicFlag::MissingMor]);
}

#[test]
fn heuristic_propn_for_function_word_eng() {
    let a = make_analysis(
        "the",
        "eng",
        Some("propn"),
        Some("propn|the"),
        None,
        AtSStatus::Spliced,
    );
    assert!(flags_for(&a).contains(&HeuristicFlag::PropnForFunctionWord));
}

#[test]
fn heuristic_propn_for_function_word_ignores_legitimate_propn() {
    let a = make_analysis(
        "London",
        "eng",
        Some("propn"),
        Some("propn|London"),
        None,
        AtSStatus::Spliced,
    );
    assert!(flags_for(&a).is_empty());
}

#[test]
fn heuristic_feature_pos_mismatch_noun_with_verb_features() {
    let a = make_analysis(
        "permit",
        "eng",
        Some("noun"),
        Some("noun|permit-Fin-Imp-S"),
        Some("Fin-Imp-S"),
        AtSStatus::Spliced,
    );
    assert!(flags_for(&a).contains(&HeuristicFlag::FeaturePosMismatch));
}

#[test]
fn heuristic_feature_pos_mismatch_verb_with_nominal_features() {
    let a = make_analysis(
        "run",
        "eng",
        Some("verb"),
        Some("verb|run-Plur"),
        Some("Plur"),
        AtSStatus::Spliced,
    );
    assert!(flags_for(&a).contains(&HeuristicFlag::FeaturePosMismatch));
}

#[test]
fn heuristic_no_flags_on_clean_spliced_noun() {
    let a = make_analysis(
        "tienda",
        "spa",
        Some("noun"),
        Some("noun|tienda"),
        None,
        AtSStatus::Spliced,
    );
    assert!(flags_for(&a).is_empty());
}

// ---------------------------------------------------------------------------
// End-to-end: phrasal-verb fixture
// ---------------------------------------------------------------------------

const PHRASAL_VERB_FIXTURE: &str = "@UTF8
@Begin
@Languages:\tdeu, eng
@Participants:\tPAR Participant
@ID:\tdeu|test|PAR|||||Participant|||
*PAR:\tich möchte wake@s up@s jetzt .
%mor:\tpron|ich-Prs-Nom-S1 aux|mögen-Fin-Sub-Past-S1 verb|wake-Fin-Imp-S part|up adv|jetzt .
%gra:\t1|3|NSUBJ 2|3|AUX 3|0|ROOT 4|3|COMPOUND-PRT 5|3|ADVMOD 6|3|PUNCT
*PAR:\tdie kinder give@s up@s immer .
%mor:\tdet|der-Def-Art-Plur noun|kinder-Neut-Plur-Nom verb|give-Fin-Imp-S part|up adv|immer .
%gra:\t1|2|DET 2|0|ROOT 3|4|ADVMOD 4|2|COMPOUND-PRT 5|2|ADVMOD 6|2|PUNCT
*PAR:\tdie zeit ist time@s out@s .
%mor:\tdet|der-Fem-Def-Art-Sing noun|zeit-Fem-Nom aux|sein-Fin-Ind-Pres-S3 noun|time adp|out .
%gra:\t1|2|DET 2|4|NSUBJ 3|4|COP 4|0|ROOT 5|4|FLAT 6|4|PUNCT
*PAR:\tsie pick@s up@s das buch .
%mor:\tpron|sie-Prs-Nom-S3 verb|pick-Fin-Imp-S part|up det|der-Neut-Def-Art-Sing noun|buchen-Neut-Acc .
%gra:\t1|2|NSUBJ 2|0|ROOT 3|2|COMPOUND-PRT 4|5|DET 5|2|OBJ 6|2|PUNCT
@End
";

#[test]
fn analyze_file_on_phrasal_verb_fixture_pairs_every_at_s_word() {
    let (_tmp, path) = write_fixture(PHRASAL_VERB_FIXTURE);
    let result = analyze_file(&path, PairKey::new("deu,eng")).expect("analyze_file");

    // 2 `@s` words per utterance × 4 utterances = 8 total.
    assert_eq!(result.analyses.len(), 8, "expected 8 @s analyses");

    // Every analysis is spliced — no L2|xxx, no missing_mor.
    for a in &result.analyses {
        assert_eq!(
            a.status,
            AtSStatus::Spliced,
            "unexpected non-spliced @s word: {:?}",
            a.occurrence.surface
        );
    }

    // The phrasal-verb heads are VERB.
    for head in ["wake", "give", "pick"] {
        let a = find_by_surface(&result.analyses, head);
        assert_eq!(
            a.pos.as_ref().map(|p| p.as_str().to_string()),
            Some("verb".to_string()),
            "{head} should be verb"
        );
    }

    // `time@s` in "time out" stays NOUN (compound-noun context).
    // In the fixture `die zeit ist time@s out@s .`, "time" is the clause
    // ROOT per the %gra row `4|0|ROOT` — it heads "zeit" (nsubj), "ist"
    // (cop), and "out" (flat). The point of the assertion is that the
    // AST walker pairs position 3 with mor_tier.items[3] / gra_tier
    // relations[3] correctly, not that the deprel is any specific label.
    let time = find_by_surface(&result.analyses, "time");
    assert_eq!(
        time.pos.as_ref().map(|p| p.as_str().to_string()),
        Some("noun".to_string())
    );
    assert_eq!(
        time.gra_deprel.as_ref().map(|d| d.as_str().to_string()),
        Some("ROOT".to_string()),
        "time@s deprel should be ROOT per fixture (4|0|ROOT)"
    );

    // `out@s` in "time out" stays ADP with FLAT deprel.
    let out = find_by_surface(&result.analyses, "out");
    assert_eq!(
        out.pos.as_ref().map(|p| p.as_str().to_string()),
        Some("adp".to_string())
    );
    assert_eq!(
        out.gra_deprel.as_ref().map(|d| d.as_str().to_string()),
        Some("FLAT".to_string())
    );

    // Every `up@s` is COMPOUND-PRT.
    let ups: Vec<_> = result
        .analyses
        .iter()
        .filter(|a| a.occurrence.surface.as_str() == "up")
        .collect();
    assert_eq!(ups.len(), 3, "expected 3 up@s occurrences");
    for u in &ups {
        assert_eq!(
            u.gra_deprel.as_ref().map(|d| d.as_str().to_string()),
            Some("COMPOUND-PRT".to_string())
        );
    }

    // No heuristic flags on this happy-path fixture.
    for a in &result.analyses {
        assert!(
            a.flags.is_empty(),
            "unexpected flag on {}: {:?}",
            a.occurrence.surface,
            a.flags
        );
    }
}

fn find_by_surface<'a>(analyses: &'a [AtSAnalysis], surface: &str) -> &'a AtSAnalysis {
    analyses
        .iter()
        .find(|a| a.occurrence.surface.as_str() == surface)
        .unwrap_or_else(|| panic!("@s word {surface:?} not found"))
}

// ---------------------------------------------------------------------------
// Aggregation
// ---------------------------------------------------------------------------

const SIMPLE_SPA_FIXTURE: &str = "@UTF8
@Begin
@Languages:\tspa, eng
@Participants:\tPAR Participant
@ID:\tspa|test|PAR|||||Participant|||
*PAR:\tyo tengo mall@s bueno .
%mor:\tpron|yo verb|tener noun|mall adj|bueno .
%gra:\t1|2|NSUBJ 2|0|ROOT 3|2|OBJ 4|3|AMOD 5|2|PUNCT
@End
";

#[test]
fn aggregate_by_pair_combines_two_files_with_same_pair_key() {
    let (_t1, path1) = write_fixture(SIMPLE_SPA_FIXTURE);
    let (_t2, path2) = write_fixture(SIMPLE_SPA_FIXTURE);
    let key = PairKey::new("eng,spa");
    let a1 = analyze_file(&path1, key.clone()).expect("a1");
    let a2 = analyze_file(&path2, key.clone()).expect("a2");

    let agg = aggregate_by_pair(&[a1, a2]);
    assert_eq!(agg.len(), 1);
    let pair = agg.get(&key).expect("pair entry");
    assert_eq!(pair.files, 2);
    assert_eq!(pair.at_s_total, 2);
    assert_eq!(pair.spliced, 2);
    assert_eq!(pair.pos_counts.get("noun").copied(), Some(2));
}

// ---------------------------------------------------------------------------
// Architectural win: retrace group
//
// `<hello mundo> [//]` is a group retrace — the two words inside `<...>`
// are retraced and do NOT have %mor entries. A regex analyzer that counts
// main-tier whitespace tokens sees "hello", "mundo", "[//]", "world@s", so
// it pairs `world@s` with position 2 (because [//] is skipped as a bracket
// token), but %mor only has 1 item ("noun|world"), and reports
// `missing_mor` at position 2.
//
// The AST walker using `TierDomain::Mor` short-circuits on
// `UtteranceContent::Retrace`, so `world@s` pairs at position 0 —
// matching %mor's 1-item list exactly. No missing_mor.
// ---------------------------------------------------------------------------

const GROUP_RETRACE_FIXTURE: &str = "@UTF8
@Begin
@Languages:\tspa, eng
@Participants:\tPAR Participant
@ID:\tspa|test|PAR|||||Participant|||
*PAR:\t<hello mundo> [//] world@s .
%mor:\tnoun|world .
%gra:\t1|0|ROOT 2|1|PUNCT
@End
";

#[test]
fn analyze_file_pairs_at_s_correctly_across_group_retrace() {
    let (_tmp, path) = write_fixture(GROUP_RETRACE_FIXTURE);
    let result = analyze_file(&path, PairKey::new("spa,eng")).expect("analyze_file");

    assert_eq!(result.analyses.len(), 1, "expected 1 @s word");
    let a = &result.analyses[0];
    assert_eq!(
        a.status,
        AtSStatus::Spliced,
        "AST walker should pair world@s with noun|world — regex \
         analyzer would report missing_mor here"
    );
    assert_eq!(
        a.pos.as_ref().map(|p| p.as_str().to_string()),
        Some("noun".to_string())
    );
    assert_eq!(a.occurrence.mor_position, 0);
}

// ---------------------------------------------------------------------------
// Smoke: the end-to-end report writer touches all four artifacts.
// ---------------------------------------------------------------------------

#[test]
fn end_to_end_run_writes_all_four_artifacts() {
    // Create a mini eval-set + morphotag-output layout, then invoke `run`.
    let tmp = tempfile::tempdir().expect("tempdir");
    let morphotag_dir = tmp.path().join("morphotag-out");
    std::fs::create_dir_all(&morphotag_dir).unwrap();
    let cha_path = morphotag_dir.join("deu_test.cha");
    std::fs::write(&cha_path, PHRASAL_VERB_FIXTURE).unwrap();

    let eval_set = tmp.path().join("eval-set.jsonl");
    std::fs::write(
        &eval_set,
        r#"{"path": "somewhere/deu_test.cha", "pair_key": "deu,eng"}
"#,
    )
    .unwrap();

    let output_dir = tmp.path().join("report");
    let args = crate::cli::args::L2MorphotagEvalArgs {
        eval_set: eval_set.clone(),
        morphotag_output: morphotag_dir.clone(),
        output: output_dir.clone(),
    };
    super::run(&args).expect("run");

    for artifact in &["per-word.csv", "per-pair.csv", "flagged.csv", "summary.md"] {
        assert!(
            Path::new(&output_dir).join(artifact).is_file(),
            "missing artifact: {artifact}"
        );
    }
}

// ---------------------------------------------------------------------------
// Wave 4: per-utterance outcome classification
// ---------------------------------------------------------------------------

mod outcome_tests {
    use talkbank_model::alignment::helpers::{MorAlignableWordCount, MorItemCount};

    use super::super::analysis::classify_utterance_outcome;
    use super::super::types::UtteranceOutcome;

    fn mor(n: usize) -> MorAlignableWordCount {
        MorAlignableWordCount::new(n)
    }

    fn item(n: usize) -> Option<MorItemCount> {
        Some(MorItemCount::new(n))
    }

    #[test]
    fn zero_alignable_no_mor_is_not_applicable() {
        assert_eq!(
            classify_utterance_outcome(mor(0), None),
            UtteranceOutcome::NotApplicable,
        );
    }

    #[test]
    fn zero_alignable_empty_mor_placeholder_is_not_applicable() {
        assert_eq!(
            classify_utterance_outcome(mor(0), item(0)),
            UtteranceOutcome::NotApplicable,
        );
    }

    #[test]
    fn zero_alignable_nonempty_mor_is_anomaly() {
        let outcome = classify_utterance_outcome(mor(0), item(3));
        assert!(matches!(
            outcome,
            UtteranceOutcome::CountMismatchInFile { .. }
        ));
        assert!(outcome.is_anomaly());
    }

    #[test]
    fn matching_counts_is_aligned() {
        let outcome = classify_utterance_outcome(mor(5), item(5));
        match outcome {
            UtteranceOutcome::Aligned { n_words } => assert_eq!(n_words.get(), 5),
            other => panic!("expected Aligned(5), got {other:?}"),
        }
        assert!(!outcome.is_anomaly());
    }

    #[test]
    fn mismatched_counts_is_count_mismatch_in_file() {
        let outcome = classify_utterance_outcome(mor(5), item(3));
        match outcome {
            UtteranceOutcome::CountMismatchInFile { n_alignable, n_mor } => {
                assert_eq!(n_alignable.get(), 5);
                assert_eq!(n_mor.get(), 3);
            }
            other => panic!("expected CountMismatchInFile, got {other:?}"),
        }
    }

    #[test]
    fn alignable_without_mor_tier_is_absorbed_failure() {
        let outcome = classify_utterance_outcome(mor(4), None);
        match outcome {
            UtteranceOutcome::PipelineAbsorbedFailure { n_alignable } => {
                assert_eq!(n_alignable.get(), 4);
            }
            other => panic!("expected PipelineAbsorbedFailure, got {other:?}"),
        }
        assert!(outcome.is_anomaly());
    }

    #[test]
    fn not_applicable_and_aligned_are_not_anomalies() {
        assert!(!UtteranceOutcome::NotApplicable.is_anomaly());
        assert!(!UtteranceOutcome::Aligned { n_words: mor(1) }.is_anomaly());
    }
}
