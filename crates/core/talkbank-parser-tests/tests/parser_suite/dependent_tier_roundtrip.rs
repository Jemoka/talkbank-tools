//! Dependent-tier fragment parity tests against the reference corpus.
//!
//! These tests ensure fragment entrypoints for dependent tiers match the same
//! parser's whole-file AST on real tiers drawn from the sacred reference corpus.

use std::fs;
use std::path::{Path, PathBuf};

use talkbank_model::ErrorCollector;
use talkbank_model::ParseOutcome;
use talkbank_model::model::{DependentTier, SemanticEq, WriteChat};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::test_error::TestError;
use walkdir::WalkDir;

use super::parser_impl::parser_suite;

fn reference_root() -> PathBuf {
    let mut full = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    full.push("../../resources/corpus/reference");
    full
}

fn relative_display(path: &Path) -> String {
    let root = reference_root();
    path.strip_prefix(&root)
        .unwrap_or(path)
        .display()
        .to_string()
}

fn collect_reference_files() -> Result<Vec<PathBuf>, TestError> {
    let mut files = Vec::new();
    for entry in WalkDir::new(reference_root()) {
        let entry = entry.map_err(|err| TestError::Failure(format!("walkdir failure: {err}")))?;
        if entry.file_type().is_file() && entry.path().extension().is_some_and(|ext| ext == "cha") {
            files.push(entry.into_path());
        }
    }
    files.sort();
    Ok(files)
}

fn collect_dependent_tiers(file: &talkbank_model::ChatFile) -> Vec<DependentTier> {
    let mut tiers = Vec::new();
    for utterance in file.utterances() {
        tiers.extend(utterance.dependent_tiers.iter().cloned());
    }
    tiers
}

fn tier_to_full_line(tier: &DependentTier) -> String {
    tier.to_chat_string()
}

fn parse_specific_tier(
    parser: &TreeSitterParser,
    tier: &DependentTier,
    content: &str,
    errors: &ErrorCollector,
) -> Option<ParseOutcome<DependentTier>> {
    Some(match tier {
        DependentTier::Mor(_) => parser
            .parse_mor_tier_fragment(content, 0, errors)
            .map(DependentTier::Mor),
        DependentTier::Gra(_) => parser
            .parse_gra_tier_fragment(content, 0, errors)
            .map(DependentTier::Gra),
        DependentTier::Pho(_) => parser
            .parse_pho_tier_fragment(content, 0, errors)
            .map(DependentTier::Pho),
        DependentTier::Mod(_) => return None,
        DependentTier::Sin(_) => parser
            .parse_sin_tier_fragment(content, 0, errors)
            .map(DependentTier::Sin),
        DependentTier::Act(_) => parser
            .parse_act_tier_fragment(content, 0, errors)
            .map(DependentTier::Act),
        DependentTier::Cod(_) => parser
            .parse_cod_tier_fragment(content, 0, errors)
            .map(DependentTier::Cod),
        DependentTier::Add(_) => parser
            .parse_add_tier_fragment(content, 0, errors)
            .map(DependentTier::Add),
        DependentTier::Com(_) => parser
            .parse_com_tier_fragment(content, 0, errors)
            .map(DependentTier::Com),
        DependentTier::Exp(_) => parser
            .parse_exp_tier_fragment(content, 0, errors)
            .map(DependentTier::Exp),
        DependentTier::Gpx(_) => parser
            .parse_gpx_tier_fragment(content, 0, errors)
            .map(DependentTier::Gpx),
        DependentTier::Int(_) => parser
            .parse_int_tier_fragment(content, 0, errors)
            .map(DependentTier::Int),
        DependentTier::Sit(_) => parser
            .parse_sit_tier_fragment(content, 0, errors)
            .map(DependentTier::Sit),
        DependentTier::Spa(_) => parser
            .parse_spa_tier_fragment(content, 0, errors)
            .map(DependentTier::Spa),
        DependentTier::Wor(_) => parser
            .parse_wor_tier_fragment(content, 0, errors)
            .map(DependentTier::Wor),
        DependentTier::Alt(_)
        | DependentTier::Coh(_)
        | DependentTier::Def(_)
        | DependentTier::Eng(_)
        | DependentTier::Err(_)
        | DependentTier::Fac(_)
        | DependentTier::Flo(_)
        | DependentTier::Gls(_)
        | DependentTier::Ort(_)
        | DependentTier::Par(_)
        | DependentTier::Tim(_)
        | DependentTier::Modsyl(_)
        | DependentTier::Phosyl(_)
        | DependentTier::Phoaln(_)
        | DependentTier::UserDefined(_)
        | DependentTier::Unsupported(_) => return None,
    })
}

#[test]
fn reference_dependent_tiers_roundtrip_for_every_parser() -> Result<(), TestError> {
    let files = collect_reference_files()?;

    for parser in parser_suite()? {
        for path in &files {
            let source = fs::read_to_string(path).map_err(|err| {
                TestError::Failure(format!("failed to read {}: {err}", path.display()))
            })?;

            let file_errors = ErrorCollector::new();
            let parsed_file = match parser.parse_chat_file_fragment(&source, 0, &file_errors) {
                ParseOutcome::Parsed(file) => file,
                ParseOutcome::Rejected => {
                    return Err(TestError::Failure(format!(
                        "[{}] rejected whole-file parse for {}",
                        "tree-sitter",
                        path.display()
                    )));
                }
            };

            if !file_errors.is_empty() {
                return Err(TestError::Failure(format!(
                    "[{}] whole-file parse errors for {}: {:?}",
                    "tree-sitter",
                    path.display(),
                    file_errors.to_vec()
                )));
            }

            for tier in collect_dependent_tiers(&parsed_file) {
                let tier_line = tier_to_full_line(&tier);
                let tier_errors = ErrorCollector::new();
                let reparsed =
                    match parser.parse_dependent_tier_fragment(&tier_line, 0, &tier_errors) {
                        ParseOutcome::Parsed(tier) => tier,
                        ParseOutcome::Rejected => {
                            return Err(TestError::Failure(format!(
                                "[{}] parse_dependent_tier rejected `{}` from {}",
                                "tree-sitter",
                                tier_line,
                                relative_display(path)
                            )));
                        }
                    };

                if !tier_errors.is_empty() {
                    return Err(TestError::Failure(format!(
                        "[{}] parse_dependent_tier errors for `{}` from {}: {:?}",
                        "tree-sitter",
                        tier_line,
                        relative_display(path),
                        tier_errors.to_vec()
                    )));
                }

                if !tier.semantic_eq(&reparsed) {
                    return Err(TestError::Failure(format!(
                        "[{}] parse_dependent_tier semantic mismatch for `{}` from {}",
                        "tree-sitter",
                        tier_line,
                        relative_display(path)
                    )));
                }

                let (_, content) = tier_line
                    .split_once('\t')
                    .unwrap_or_else(|| panic!("dependent tier missing tab separator: {tier_line}"));
                let typed_errors = ErrorCollector::new();
                if let Some(reparsed_typed) =
                    parse_specific_tier(&parser, &tier, content, &typed_errors)
                {
                    let reparsed_typed = match reparsed_typed {
                        ParseOutcome::Parsed(tier) => tier,
                        ParseOutcome::Rejected => {
                            return Err(TestError::Failure(format!(
                                "[{}] typed parser rejected `{}` from {}",
                                "tree-sitter",
                                tier_line,
                                relative_display(path)
                            )));
                        }
                    };

                    if !typed_errors.is_empty() {
                        return Err(TestError::Failure(format!(
                            "[{}] typed parser errors for `{}` from {}: {:?}",
                            "tree-sitter",
                            tier_line,
                            relative_display(path),
                            typed_errors.to_vec()
                        )));
                    }

                    if !tier.semantic_eq(&reparsed_typed) {
                        return Err(TestError::Failure(format!(
                            "[{}] typed parser semantic mismatch for `{}` from {}",
                            "tree-sitter",
                            tier_line,
                            relative_display(path)
                        )));
                    }
                }
            }
        }
    }

    Ok(())
}
