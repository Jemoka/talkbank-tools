//! Typed planning layer from runner snapshots to immutable job plans.

use crate::api::DisplayPath;
use crate::command_model::{self, CatalogEntry, PlannedMaterializedFile};
use crate::recipe_runner::planner::{PlanningError, plan_work_units};
use crate::recipe_runner::runtime::discover_inputs_for_job;
use crate::recipe_runner::work_unit::{PlannedWorkUnit, TextWorkUnit};
use crate::store::RunnerJobSnapshot;

/// How the executor will resolve input and output files for a job.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IoMode {
    /// Runner reads and writes through the shared filesystem.
    Paths,
    /// Runner reads and writes from staged content under the job directory.
    Content,
}

/// All materialized output artifacts derived from one planned work unit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PlannedArtifactSet {
    /// Source display path that owns these output artifacts.
    pub source_display_path: DisplayPath,
    /// Primary output plus any sidecars for the work unit.
    pub files: Vec<PlannedMaterializedFile>,
}

/// Immutable execution input for one job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct JobPlan {
    /// Authoritative command model entry for the released command.
    pub spec: &'static CatalogEntry,
    /// Typed work units derived from the runner snapshot.
    pub work_units: Vec<PlannedWorkUnit>,
    /// Planned output artifacts for each work unit.
    pub artifacts: Vec<PlannedArtifactSet>,
    /// Resolved I/O mode for execution.
    pub io_mode: IoMode,
}

/// Build one immutable job plan from the current runner snapshot.
pub(crate) fn build_job_plan(job: &RunnerJobSnapshot) -> Result<JobPlan, PlanningError> {
    let spec = command_model::command_spec(job.dispatch.command);
    let inputs = discover_inputs_for_job(job);
    let work_units = plan_work_units(spec.planner, &inputs)?;
    let artifacts = work_units
        .iter()
        .map(|work_unit| {
            let source_display_path = primary_display_path_for_work_unit(work_unit).clone();
            let files = crate::recipe_runner::materialize::plan_materialized_files(
                &source_display_path,
                spec.output_policy,
            );
            PlannedArtifactSet {
                source_display_path,
                files,
            }
        })
        .collect();

    Ok(JobPlan {
        spec,
        work_units,
        artifacts,
        io_mode: if job.filesystem.paths_mode {
            IoMode::Paths
        } else {
            IoMode::Content
        },
    })
}

/// Return the planned output artifacts for one source display path.
pub(crate) fn artifact_set_for_source<'a>(
    plan: &'a JobPlan,
    source_display_path: &DisplayPath,
) -> Option<&'a PlannedArtifactSet> {
    plan.artifacts
        .iter()
        .find(|artifacts| &artifacts.source_display_path == source_display_path)
}

fn primary_display_path_for_work_unit(work_unit: &PlannedWorkUnit) -> &DisplayPath {
    match work_unit {
        PlannedWorkUnit::Text(TextWorkUnit { source })
        | PlannedWorkUnit::MediaAnalysis(
            crate::recipe_runner::work_unit::MediaAnalysisWorkUnit { source },
        ) => &source.display_path,
        PlannedWorkUnit::Audio(crate::recipe_runner::work_unit::AudioWorkUnit { audio })
        | PlannedWorkUnit::Benchmark(crate::recipe_runner::work_unit::BenchmarkWorkUnit {
            audio,
            ..
        }) => &audio.display_path,
        PlannedWorkUnit::Compare(crate::recipe_runner::work_unit::CompareWorkUnit {
            main, ..
        }) => &main.display_path,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use tokio_util::sync::CancellationToken;

    use super::*;
    use crate::api::{CorrelationId, JobId, LanguageSpec, NumSpeakers, ReleasedCommand};
    use crate::options::{CommandOptions, CommonOptions, CompareOptions};
    use crate::store::{
        PendingJobFile, RunnerDispatchConfig, RunnerFilesystemConfig, RunnerJobIdentity,
    };

    fn compare_snapshot(paths_mode: bool) -> RunnerJobSnapshot {
        RunnerJobSnapshot {
            identity: RunnerJobIdentity {
                job_id: JobId::from("job-plan"),
                correlation_id: CorrelationId::from("corr-plan"),
            },
            dispatch: RunnerDispatchConfig {
                command: ReleasedCommand::Compare,
                lang: LanguageSpec::Resolved(crate::api::LanguageCode3::eng()),
                num_speakers: NumSpeakers(1),
                options: CommandOptions::Compare(CompareOptions {
                    common: CommonOptions::default(),
                    merge_abbrev: false.into(),
                }),
                runtime_state: BTreeMap::new(),
                debug_traces: false,
            },
            filesystem: RunnerFilesystemConfig {
                paths_mode,
                source_paths: vec![
                    batchalign_types::paths::ClientPath::from("/src/nested/sample.cha"),
                    batchalign_types::paths::ClientPath::from("/src/nested/sample.gold.cha"),
                ],
                output_paths: vec![
                    batchalign_types::paths::ClientPath::from("/out/nested/sample.cha"),
                    batchalign_types::paths::ClientPath::from("/out/nested/sample.gold.cha"),
                ],
                before_paths: Vec::new(),
                staging_dir: batchalign_types::paths::ServerPath::new("/tmp/staging"),
                media_mapping: Default::default(),
                media_subdir: Default::default(),
                source_dir: batchalign_types::paths::ClientPath::new("/src"),
            },
            cancel_token: CancellationToken::new(),
            pending_files: vec![
                PendingJobFile {
                    file_index: 0,
                    filename: DisplayPath::from("nested/sample.cha"),
                    has_chat: true,
                },
                PendingJobFile {
                    file_index: 1,
                    filename: DisplayPath::from("nested/sample.gold.cha"),
                    has_chat: true,
                },
            ],
        }
    }

    #[test]
    fn compare_plan_uses_one_pair_and_compare_sidecar() {
        let plan = build_job_plan(&compare_snapshot(true)).expect("compare plan");
        assert_eq!(plan.spec.command, ReleasedCommand::Compare);
        assert_eq!(plan.io_mode, IoMode::Paths);
        assert_eq!(plan.work_units.len(), 1);
        assert_eq!(plan.artifacts.len(), 1);
        assert_eq!(
            plan.artifacts[0].source_display_path,
            DisplayPath::from("nested/sample.cha")
        );
        assert_eq!(plan.artifacts[0].files.len(), 2);
        assert_eq!(
            plan.artifacts[0].files[1].display_path,
            DisplayPath::from("nested/sample.compare.csv")
        );
    }

    #[test]
    fn content_mode_job_plan_tracks_content_io() {
        let plan = build_job_plan(&compare_snapshot(false)).expect("compare plan");
        assert_eq!(plan.io_mode, IoMode::Content);
    }
}
