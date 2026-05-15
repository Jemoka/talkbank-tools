//! Work planners for the recipe-runner spike.

use std::collections::BTreeMap;
use std::path::Path;

use thiserror::Error;

use crate::api::DisplayPath;

use super::command_spec::PlannerKind;
use super::work_unit::{
    AudioWorkUnit, BenchmarkWorkUnit, CompareWorkUnit, DiscoveredInput, MediaAnalysisWorkUnit,
    PlannedWorkUnit, TextWorkUnit,
};

/// Planning-time error while deriving typed work units.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub(crate) enum PlanningError {
    /// The source path did not expose a file stem for companion derivation.
    #[error("cannot derive companion path for source input {0}")]
    MissingFileStem(DisplayPath),
    /// The source path did not expose a file name for companion derivation.
    #[error("cannot derive companion file name for source input {0}")]
    MissingFileName(DisplayPath),
}

/// Plan typed work units for one command family.
pub(crate) fn plan_work_units(
    planner: PlannerKind,
    inputs: &[DiscoveredInput],
) -> Result<Vec<PlannedWorkUnit>, PlanningError> {
    match planner {
        PlannerKind::TextInputs => Ok(inputs
            .iter()
            .cloned()
            .map(|source| PlannedWorkUnit::Text(TextWorkUnit { source }))
            .collect()),
        PlannerKind::AudioInputs => Ok(inputs
            .iter()
            .cloned()
            .map(|audio| PlannedWorkUnit::Audio(AudioWorkUnit { audio }))
            .collect()),
        PlannerKind::ComparePairs => plan_compare_pairs(inputs),
        PlannerKind::BenchmarkPairs => plan_benchmark_pairs(inputs),
        PlannerKind::MediaAnalysisInputs => Ok(inputs
            .iter()
            .cloned()
            .map(|source| PlannedWorkUnit::MediaAnalysis(MediaAnalysisWorkUnit { source }))
            .collect()),
    }
}

fn plan_compare_pairs(inputs: &[DiscoveredInput]) -> Result<Vec<PlannedWorkUnit>, PlanningError> {
    let discovered_by_display: BTreeMap<String, DiscoveredInput> = inputs
        .iter()
        .cloned()
        .map(|input| (input.display_path.to_string(), input))
        .collect();

    let mut planned = Vec::new();
    for main in inputs {
        if is_compare_gold(main.display_path.as_ref()) {
            continue;
        }
        let gold = discovered_by_display
            .get(derive_compare_gold_display_path(&main.display_path)?.as_ref())
            .cloned()
            .unwrap_or(derive_compare_gold_input(main)?);
        planned.push(PlannedWorkUnit::Compare(CompareWorkUnit {
            main: main.clone(),
            gold,
        }));
    }
    Ok(planned)
}

fn plan_benchmark_pairs(inputs: &[DiscoveredInput]) -> Result<Vec<PlannedWorkUnit>, PlanningError> {
    inputs
        .iter()
        .cloned()
        .map(|audio| {
            let gold_chat = derive_benchmark_gold_input(&audio)?;
            Ok(PlannedWorkUnit::Benchmark(BenchmarkWorkUnit {
                audio,
                gold_chat,
            }))
        })
        .collect()
}

fn is_compare_gold(display_path: &str) -> bool {
    display_path.ends_with(".gold.cha")
}

fn derive_compare_gold_display_path(
    main_display_path: &DisplayPath,
) -> Result<DisplayPath, PlanningError> {
    let path = Path::new(main_display_path.as_ref());
    let stem = path
        .file_stem()
        .ok_or_else(|| PlanningError::MissingFileStem(main_display_path.clone()))?
        .to_string_lossy();
    let parent = path.parent().unwrap_or_else(|| Path::new(""));
    Ok(DisplayPath::from(
        parent
            .join(format!("{stem}.gold.cha"))
            .to_string_lossy()
            .to_string(),
    ))
}

fn derive_compare_gold_input(main: &DiscoveredInput) -> Result<DiscoveredInput, PlanningError> {
    let gold_display_path = derive_compare_gold_display_path(&main.display_path)?;
    let gold_file_name = Path::new(gold_display_path.as_ref())
        .file_name()
        .ok_or_else(|| PlanningError::MissingFileName(gold_display_path.clone()))?;
    let gold_file_name = gold_file_name.to_owned();
    Ok(DiscoveredInput {
        display_path: gold_display_path,
        source_path: main.source_path.with_file_name(gold_file_name),
        before_path: None,
    })
}

fn derive_benchmark_gold_input(audio: &DiscoveredInput) -> Result<DiscoveredInput, PlanningError> {
    let display_path = Path::new(audio.display_path.as_ref());
    let stem = display_path
        .file_stem()
        .ok_or_else(|| PlanningError::MissingFileStem(audio.display_path.clone()))?
        .to_string_lossy();
    let parent = display_path.parent().unwrap_or_else(|| Path::new(""));
    Ok(DiscoveredInput {
        display_path: DisplayPath::from(
            parent
                .join(format!("{stem}.cha"))
                .to_string_lossy()
                .to_string(),
        ),
        source_path: audio.source_path.with_extension("cha"),
        before_path: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recipe_runner::command_spec::PlannerKind;

    #[test]
    fn compare_planner_skips_gold_inputs_and_uses_discovered_companion() {
        let inputs = vec![
            DiscoveredInput::new("sample.cha", "/tmp/sample.cha"),
            DiscoveredInput::new("sample.gold.cha", "/tmp/sample.gold.cha"),
        ];
        let planned = plan_work_units(PlannerKind::ComparePairs, &inputs).expect("planned pairs");
        assert_eq!(planned.len(), 1);
        let PlannedWorkUnit::Compare(pair) = &planned[0] else {
            panic!("expected compare pair");
        };
        assert_eq!(pair.main.display_path, DisplayPath::from("sample.cha"));
        assert_eq!(pair.gold.display_path, DisplayPath::from("sample.gold.cha"));
        assert_eq!(
            pair.gold.source_path,
            std::path::PathBuf::from("/tmp/sample.gold.cha")
        );
    }

    #[test]
    fn compare_planner_derives_missing_gold_paths_from_main() {
        let inputs = vec![DiscoveredInput::new(
            "nested/sample.cha",
            "/abs/nested/sample.cha",
        )];
        let planned = plan_work_units(PlannerKind::ComparePairs, &inputs).expect("planned pairs");
        let PlannedWorkUnit::Compare(pair) = &planned[0] else {
            panic!("expected compare pair");
        };
        assert_eq!(
            pair.gold.display_path,
            DisplayPath::from("nested/sample.gold.cha")
        );
        assert_eq!(
            pair.gold.source_path,
            std::path::PathBuf::from("/abs/nested/sample.gold.cha")
        );
    }

    #[test]
    fn benchmark_planner_derives_gold_chat_from_audio() {
        let inputs = vec![DiscoveredInput::new(
            "audio/session.wav",
            "/abs/audio/session.wav",
        )];
        let planned =
            plan_work_units(PlannerKind::BenchmarkPairs, &inputs).expect("planned benchmark");
        let PlannedWorkUnit::Benchmark(unit) = &planned[0] else {
            panic!("expected benchmark unit");
        };
        assert_eq!(
            unit.gold_chat.display_path,
            DisplayPath::from("audio/session.cha")
        );
        assert_eq!(
            unit.gold_chat.source_path,
            std::path::PathBuf::from("/abs/audio/session.cha")
        );
    }
}
