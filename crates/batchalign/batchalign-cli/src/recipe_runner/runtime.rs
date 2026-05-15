//! Live runtime adapters for the recipe-runner spike.
//!
//! These helpers let the current runner reuse recipe-runner planning and output
//! metadata without a flag-day dispatcher rewrite.

use std::path::{Path, PathBuf};

use crate::api::{DisplayPath, ReleasedCommand};
use crate::command_model;
use crate::store::{PendingJobFile, RunnerFilesystemConfig, RunnerJobSnapshot};

use super::materialize::{
    MaterializedArtifactRole, PlannedMaterializedFile, plan_materialized_files,
};
use super::planner::{PlanningError, plan_work_units};
use super::work_unit::{DiscoveredInput, PlannedWorkUnit};

/// Derive one discovered input from the runner's current filesystem layout.
pub(crate) fn discover_input_for_pending_file(
    filesystem: &RunnerFilesystemConfig,
    file: &PendingJobFile,
) -> DiscoveredInput {
    let source_path = if filesystem.paths_mode && file.file_index < filesystem.source_paths.len() {
        filesystem.source_paths[file.file_index]
            .assume_shared_filesystem()
            .as_path()
            .to_owned()
    } else {
        filesystem
            .staging_dir
            .join("input")
            .join(file.filename.as_ref())
            .as_path()
            .to_owned()
    };
    let before_path = (!filesystem.before_paths.is_empty()
        && file.file_index < filesystem.before_paths.len())
    .then(|| {
        filesystem.before_paths[file.file_index]
            .assume_shared_filesystem()
            .as_path()
            .to_owned()
    });
    DiscoveredInput {
        display_path: file.filename.clone(),
        source_path,
        before_path,
    }
}

/// Derive all discovered inputs for one runner job.
pub(crate) fn discover_inputs_for_job(job: &RunnerJobSnapshot) -> Vec<DiscoveredInput> {
    job.pending_files
        .iter()
        .map(|file| discover_input_for_pending_file(&job.filesystem, file))
        .collect()
}

/// Plan command-family-specific work units from the runner snapshot.
pub(crate) fn plan_work_units_for_job(
    command: ReleasedCommand,
    job: &RunnerJobSnapshot,
) -> Result<Vec<PlannedWorkUnit>, PlanningError> {
    let inputs = discover_inputs_for_job(job);
    plan_work_units(command_model::command_spec(command).planner, &inputs)
}

/// Plan all materialized artifacts for one released command and source path.
pub(crate) fn planned_output_artifacts(
    command: ReleasedCommand,
    source_display_path: &DisplayPath,
) -> Vec<PlannedMaterializedFile> {
    plan_materialized_files(
        source_display_path,
        command_model::command_spec(command).output_policy,
    )
}

/// Return the primary result artifact for one command/input pair.
pub(crate) fn primary_output_artifact(
    command: ReleasedCommand,
    source_display_path: &DisplayPath,
) -> PlannedMaterializedFile {
    // Recipe-catalog invariant: every command has exactly one
    // `MaterializedArtifactRole::Primary` artifact in its
    // `OutputPolicy`. The catalog test in
    // `recipe_runner/catalog.rs::tests` enforces this; reaching the
    // expect would mean a catalog drift caught before merge.
    #[allow(clippy::expect_used)]
    planned_output_artifacts(command, source_display_path)
        .into_iter()
        .find(|artifact| artifact.role == MaterializedArtifactRole::Primary)
        .expect("recipe runner output policy must include a primary artifact")
}

/// Return all sidecar artifacts for one command/input pair.
pub(crate) fn sidecar_output_artifacts(
    command: ReleasedCommand,
    source_display_path: &DisplayPath,
) -> Vec<PlannedMaterializedFile> {
    planned_output_artifacts(command, source_display_path)
        .into_iter()
        .filter(|artifact| artifact.role == MaterializedArtifactRole::Sidecar)
        .collect()
}

/// Derive only the primary result display path for one command/input pair.
pub(crate) fn result_display_path_for_command(
    command: ReleasedCommand,
    filename: &str,
) -> DisplayPath {
    primary_output_artifact(command, &DisplayPath::from(filename)).display_path
}

/// Resolve the concrete write path for one planned output artifact.
pub(crate) fn output_write_path(
    filesystem: &RunnerFilesystemConfig,
    file_index: usize,
    result_display_path: &DisplayPath,
) -> PathBuf {
    if filesystem.paths_mode && file_index < filesystem.output_paths.len() {
        let result_name = Path::new(result_display_path.as_ref())
            .file_name()
            .unwrap_or_default();
        let output_server_path = filesystem.output_paths[file_index].assume_shared_filesystem();
        Path::new(output_server_path.as_str())
            .parent()
            .map(|parent| parent.join(result_name))
            .unwrap_or_else(|| result_name.into())
    } else {
        filesystem
            .staging_dir
            .join("output")
            .join(result_display_path.as_ref())
            .as_path()
            .to_owned()
    }
}

fn staged_output_path(
    filesystem: &RunnerFilesystemConfig,
    result_display_path: &DisplayPath,
) -> PathBuf {
    filesystem
        .staging_dir
        .join("output")
        .join(result_display_path.as_ref())
        .as_path()
        .to_owned()
}

async fn write_text_file(path: &Path, content: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(path, content).await
}

/// One CHAT-output destination for a single file in a job: enough
/// state to derive both the primary on-disk write path and the staged
/// copy. Bundling the (filesystem, file_index, display_path) triple
/// gives all the writers a single typed seam — and lets future
/// per-target preconditions (e.g., paths_mode-aware skip logic) attach
/// in one place rather than at every callsite.
pub(crate) struct ChatOutputTarget<'a> {
    pub filesystem: &'a RunnerFilesystemConfig,
    pub file_index: usize,
    pub display_path: &'a DisplayPath,
}

impl<'a> ChatOutputTarget<'a> {
    pub fn new(
        filesystem: &'a RunnerFilesystemConfig,
        file_index: usize,
        display_path: &'a DisplayPath,
    ) -> Self {
        Self {
            filesystem,
            file_index,
            display_path,
        }
    }

    fn primary_path(&self) -> PathBuf {
        output_write_path(self.filesystem, self.file_index, self.display_path)
    }

    fn staged_path(&self) -> PathBuf {
        staged_output_path(self.filesystem, self.display_path)
    }
}

/// Write a text result to the command's primary target and to staged output.
///
/// Paths-mode jobs still write to the execution host's requested output path,
/// but they also preserve a staged copy so the submitting CLI can download and
/// write the same result back to its own local filesystem.
pub(crate) async fn write_text_output_artifact(
    target: &ChatOutputTarget<'_>,
    content: &str,
) -> std::io::Result<()> {
    let write_path = target.primary_path();
    write_text_file(&write_path, content).await?;

    let staged_path = target.staged_path();
    if staged_path != write_path {
        write_text_file(&staged_path, content).await?;
    }

    Ok(())
}

/// Write a CHAT result to the command's primary target and staged output,
/// suppressing the write at any path where the only difference vs the
/// existing on-disk text is inside the `[ba3 <command> | ...]` provenance
/// comment for `command`.
///
/// Use this for any pipeline that injects a `ProvenanceComment` for a
/// known [`ReleasedCommand`] before serializing CHAT. The gate eliminates
/// the "re-running the same command produced a 1-line timestamp/version
/// diff per file" failure mode that produces large, semantically empty
/// commits in corpus repos.
///
/// Behavior per write target:
///
/// - Target file does not exist → write (first run).
/// - Existing bytes are byte-equal to `content` → skip (already correct).
/// - Existing bytes differ from `content` only inside the `[ba3 <command>]`
///   provenance line → skip (the only would-be change is the timestamp /
///   engine slot, which we deliberately do not propagate).
/// - Existing bytes differ in any other content (`%mor`, `%gra`, `%wor`,
///   another command's provenance, anything else) → write.
///
/// The primary write_path and the staged output path are evaluated
/// independently. In the common paths_mode layout the staged copy lives
/// in `staging_dir/output/...` and is fresh per job, so it almost always
/// triggers a write; the primary path is the one the gate effectively
/// guards. We still apply the same logic to both for symmetry — there
/// is no scenario where it is correct to update one and not the other.
pub(crate) async fn write_chat_output_artifact_with_provenance_gate(
    target: &ChatOutputTarget<'_>,
    content: &str,
    command: ReleasedCommand,
) -> std::io::Result<()> {
    let write_path = target.primary_path();
    write_chat_if_meaningful_diff(&write_path, content, command).await?;

    let staged_path = target.staged_path();
    if staged_path != write_path {
        write_chat_if_meaningful_diff(&staged_path, content, command).await?;
    }

    Ok(())
}

/// Read the existing file at `path` (if any), compare it to `candidate`
/// for the specific `command`'s provenance gate, and write only if a
/// real (non-provenance-only) difference exists.
///
/// The read is best-effort: any read error other than NotFound is
/// surfaced. NotFound is treated as "first run" and triggers a write.
///
/// Cheap pre-check: the gate only fires when the existing-vs-candidate
/// difference is bounded to a single provenance line. If the candidate's
/// length differs from the existing file's length by more than a
/// provenance line could plausibly contribute, the gate cannot fire and
/// we skip the read entirely. The bound is generous (one full
/// provenance line is well under 200 bytes; doubling that absorbs
/// engine-string / language-tag drift) — we want false positives that
/// fall through to the existing read+compare path, not false negatives
/// that wrongly suppress a write.
async fn write_chat_if_meaningful_diff(
    path: &Path,
    candidate: &str,
    command: ReleasedCommand,
) -> std::io::Result<()> {
    match tokio::fs::metadata(path).await {
        Ok(meta) => {
            let existing_len = meta.len();
            let candidate_len = candidate.len() as u64;
            let diff = existing_len.abs_diff(candidate_len);
            if diff > PROVENANCE_LINE_DIFF_BUDGET_BYTES {
                // The two files differ by more than a provenance line
                // could contribute; the gate cannot possibly fire and
                // there is no point reading the existing file just to
                // compare. Write through.
                return write_text_file(path, candidate).await;
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            // First run — file does not exist yet. Write through.
            return write_text_file(path, candidate).await;
        }
        Err(error) => return Err(error),
    }

    match tokio::fs::read_to_string(path).await {
        Ok(existing) => {
            if existing == candidate {
                // Bytes already match. Skip — avoid mtime churn.
                return Ok(());
            }
            if crate::provenance::is_provenance_only_difference(&existing, candidate, command) {
                // The only would-be change is inside the command's
                // provenance line. Per the no-spurious-update policy,
                // do not write.
                return Ok(());
            }
            write_text_file(path, candidate).await
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            // Race: file existed at metadata time but is gone now.
            // Treat as first-run and write through.
            write_text_file(path, candidate).await
        }
        Err(error) => Err(error),
    }
}

/// Bound on how much the candidate output's byte length may differ from
/// the on-disk byte length while still being eligible for the
/// provenance-only-diff gate. A typical
/// `[ba3 morphotag | engine=stanza-1.11.1 ; lang=eng | 2026-05-08T02:52:17-04:00]`
/// line is ~95 bytes; we double that to absorb engine-string and
/// timezone-offset drift comfortably.
const PROVENANCE_LINE_DIFF_BUDGET_BYTES: u64 = 200;

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use tokio_util::sync::CancellationToken;

    use super::*;
    use crate::api::{CorrelationId, JobId, LanguageSpec, NumSpeakers};
    use crate::options::{CommandOptions, CommonOptions, CompareOptions};
    use crate::store::{
        PendingJobFile, RunnerDispatchConfig, RunnerFilesystemConfig, RunnerJobIdentity,
        RunnerJobSnapshot,
    };

    fn runner_snapshot(
        command: ReleasedCommand,
        pending_files: Vec<PendingJobFile>,
        source_paths: Vec<&str>,
        before_paths: Vec<&str>,
    ) -> RunnerJobSnapshot {
        RunnerJobSnapshot {
            identity: RunnerJobIdentity {
                job_id: JobId::from("job-runtime"),
                correlation_id: CorrelationId::from("corr-runtime"),
            },
            dispatch: RunnerDispatchConfig {
                command,
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
                paths_mode: true,
                source_paths: source_paths
                    .into_iter()
                    .map(batchalign_types::paths::ClientPath::from)
                    .collect(),
                output_paths: vec![
                    batchalign_types::paths::ClientPath::new("/out/main.cha");
                    pending_files.len()
                ],
                before_paths: before_paths
                    .into_iter()
                    .map(batchalign_types::paths::ClientPath::from)
                    .collect(),
                staging_dir: batchalign_types::paths::ServerPath::new("/tmp/staging"),
                media_mapping: Default::default(),
                media_subdir: Default::default(),
                source_dir: batchalign_types::paths::ClientPath::new("/source"),
            },
            cancel_token: CancellationToken::new(),
            pending_files,
        }
    }

    #[test]
    fn discover_input_carries_before_path() {
        let file = PendingJobFile {
            file_index: 0,
            filename: DisplayPath::from("nested/sample.cha"),
            has_chat: true,
        };
        let job = runner_snapshot(
            ReleasedCommand::Morphotag,
            vec![file.clone()],
            vec!["/src/nested/sample.cha"],
            vec!["/before/nested/sample.cha"],
        );
        let input = discover_input_for_pending_file(&job.filesystem, &file);
        assert_eq!(input.source_path, PathBuf::from("/src/nested/sample.cha"));
        assert_eq!(
            input.before_path,
            Some(PathBuf::from("/before/nested/sample.cha"))
        );
    }

    #[test]
    fn compare_job_planning_uses_gold_companion_shape() {
        let job = runner_snapshot(
            ReleasedCommand::Compare,
            vec![
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
            vec!["/src/nested/sample.cha", "/src/nested/sample.gold.cha"],
            vec![],
        );
        let planned = plan_work_units_for_job(ReleasedCommand::Compare, &job).expect("planned");
        assert_eq!(planned.len(), 1);
        let PlannedWorkUnit::Compare(unit) = &planned[0] else {
            panic!("expected compare unit");
        };
        assert_eq!(
            unit.main.display_path,
            DisplayPath::from("nested/sample.cha")
        );
        assert_eq!(
            unit.gold.display_path,
            DisplayPath::from("nested/sample.gold.cha")
        );
    }

    #[test]
    fn result_display_paths_follow_recipe_catalog() {
        assert_eq!(
            result_display_path_for_command(ReleasedCommand::Transcribe, "sub/nested.wav"),
            DisplayPath::from("sub/nested.cha")
        );
        assert_eq!(
            result_display_path_for_command(ReleasedCommand::Benchmark, "sub/nested.wav"),
            DisplayPath::from("sub/nested.cha")
        );
        assert_eq!(
            result_display_path_for_command(ReleasedCommand::Opensmile, "sub/nested.wav"),
            DisplayPath::from("sub/nested.opensmile.csv")
        );
        assert_eq!(
            result_display_path_for_command(ReleasedCommand::Avqi, "sub/nested.cs.wav"),
            DisplayPath::from("sub/nested.avqi.txt")
        );
    }

    #[test]
    fn compare_sidecars_follow_recipe_catalog() {
        let sidecars =
            sidecar_output_artifacts(ReleasedCommand::Compare, &DisplayPath::from("a/b.cha"));
        assert_eq!(sidecars.len(), 1);
        assert_eq!(
            sidecars[0].display_path,
            DisplayPath::from("a/b.compare.csv")
        );
    }

    #[test]
    fn output_write_path_preserves_staging_relative_dirs() {
        let filesystem = RunnerFilesystemConfig {
            paths_mode: false,
            source_paths: Vec::new(),
            output_paths: Vec::new(),
            before_paths: Vec::new(),
            staging_dir: batchalign_types::paths::ServerPath::new("/tmp/staging"),
            media_mapping: Default::default(),
            media_subdir: Default::default(),
            source_dir: batchalign_types::paths::ClientPath::new("/source"),
        };
        let write_path = output_write_path(
            &filesystem,
            0,
            &DisplayPath::from("nested/result.compare.csv"),
        );
        assert_eq!(
            write_path,
            PathBuf::from("/tmp/staging/output/nested/result.compare.csv")
        );
    }

    #[tokio::test]
    async fn write_text_output_artifact_keeps_staged_copy_for_paths_mode() {
        let root = tempfile::tempdir().expect("tempdir");
        let host_out = root.path().join("host/output/sample.cha");
        let filesystem = RunnerFilesystemConfig {
            paths_mode: true,
            source_paths: vec![batchalign_types::paths::ClientPath::new(
                root.path()
                    .join("input/sample.cha")
                    .to_string_lossy()
                    .to_string(),
            )],
            output_paths: vec![batchalign_types::paths::ClientPath::new(
                host_out.to_string_lossy().to_string(),
            )],
            before_paths: Vec::new(),
            staging_dir: batchalign_types::paths::ServerPath::from(root.path().join("jobs/job-1")),
            media_mapping: Default::default(),
            media_subdir: Default::default(),
            source_dir: batchalign_types::paths::ClientPath::new(
                root.path().join("input").to_string_lossy().to_string(),
            ),
        };

        let display = DisplayPath::from("nested/sample.cha");
        let target = ChatOutputTarget::new(&filesystem, 0, &display);
        write_text_output_artifact(&target, "@Begin\n@End\n")
            .await
            .expect("write output");

        assert_eq!(
            std::fs::read_to_string(&host_out).expect("host output"),
            "@Begin\n@End\n"
        );
        assert_eq!(
            std::fs::read_to_string(
                filesystem
                    .staging_dir
                    .join("output")
                    .join("nested/sample.cha")
            )
            .expect("staged output"),
            "@Begin\n@End\n"
        );
    }

    /// Re-running morphotag over an unchanged corpus must not modify the
    /// on-disk file just to update the provenance comment's timestamp.
    /// Pre-condition: a CHAT file exists at the primary output path with
    /// an old `[ba3 morphotag | ...]` provenance line. We then call the
    /// gated writer with new text that differs only in that line's
    /// timestamp. Post-condition: the file's bytes are unchanged.
    #[tokio::test]
    async fn write_chat_output_artifact_with_provenance_gate_skips_when_only_provenance_differs() {
        let root = tempfile::tempdir().expect("tempdir");
        let host_out = root.path().join("host/output/sample.cha");
        std::fs::create_dir_all(host_out.parent().unwrap()).unwrap();

        let original = "\
@UTF8
@Begin
@Languages:\teng
@Participants:\tPAR Participant
@ID:\teng|test|PAR|||||Participant|||
@Comment:\t[ba3 morphotag | engine=stanza-1.11.1 ; lang=eng | 2026-03-29T18:30:00-04:00]
*PAR:\thello .
%mor:\tco|hello .
@End
";
        std::fs::write(&host_out, original).unwrap();

        let staging_dir = root.path().join("jobs/job-1");
        let filesystem = RunnerFilesystemConfig {
            paths_mode: true,
            source_paths: vec![batchalign_types::paths::ClientPath::new(
                root.path()
                    .join("input/sample.cha")
                    .to_string_lossy()
                    .to_string(),
            )],
            output_paths: vec![batchalign_types::paths::ClientPath::new(
                host_out.to_string_lossy().to_string(),
            )],
            before_paths: Vec::new(),
            staging_dir: batchalign_types::paths::ServerPath::from(staging_dir.clone()),
            media_mapping: Default::default(),
            media_subdir: Default::default(),
            source_dir: batchalign_types::paths::ClientPath::new(
                root.path().join("input").to_string_lossy().to_string(),
            ),
        };

        // Same content as `original` except for the provenance line's
        // timestamp. This is the canonical pointless-rerun shape we want
        // to suppress.
        let candidate = "\
@UTF8
@Begin
@Languages:\teng
@Participants:\tPAR Participant
@ID:\teng|test|PAR|||||Participant|||
@Comment:\t[ba3 morphotag | engine=stanza-1.11.1 ; lang=eng | 2026-05-08T02:52:17-04:00]
*PAR:\thello .
%mor:\tco|hello .
@End
";

        let display = DisplayPath::from("nested/sample.cha");
        let target = ChatOutputTarget::new(&filesystem, 0, &display);
        write_chat_output_artifact_with_provenance_gate(
            &target,
            candidate,
            ReleasedCommand::Morphotag,
        )
        .await
        .expect("gated write");

        let after = std::fs::read_to_string(&host_out).expect("read after");
        assert_eq!(
            after, original,
            "primary output bytes must be unchanged when only the provenance differs"
        );
    }

    /// Real `%mor` content change must still write through. The gate is
    /// strictly a noise-suppressor; any real difference must fall back
    /// to the existing write semantics.
    #[tokio::test]
    async fn write_chat_output_artifact_with_provenance_gate_writes_when_real_content_differs() {
        let root = tempfile::tempdir().expect("tempdir");
        let host_out = root.path().join("host/output/sample.cha");
        std::fs::create_dir_all(host_out.parent().unwrap()).unwrap();

        let original = "\
@Comment:\t[ba3 morphotag | engine=stanza-1.11.1 ; lang=eng | 2026-03-29T18:30:00-04:00]
*PAR:\thello .
%mor:\tco|hello .
";
        std::fs::write(&host_out, original).unwrap();

        let staging_dir = root.path().join("jobs/job-1");
        let filesystem = RunnerFilesystemConfig {
            paths_mode: true,
            source_paths: vec![batchalign_types::paths::ClientPath::new(
                root.path()
                    .join("input/sample.cha")
                    .to_string_lossy()
                    .to_string(),
            )],
            output_paths: vec![batchalign_types::paths::ClientPath::new(
                host_out.to_string_lossy().to_string(),
            )],
            before_paths: Vec::new(),
            staging_dir: batchalign_types::paths::ServerPath::from(staging_dir.clone()),
            media_mapping: Default::default(),
            media_subdir: Default::default(),
            source_dir: batchalign_types::paths::ClientPath::new(
                root.path().join("input").to_string_lossy().to_string(),
            ),
        };

        // Different `%mor` content — a legitimate result update.
        let candidate = "\
@Comment:\t[ba3 morphotag | engine=stanza-1.11.1 ; lang=eng | 2026-05-08T02:52:17-04:00]
*PAR:\thello .
%mor:\tco|hi .
";

        let display = DisplayPath::from("nested/sample.cha");
        let target = ChatOutputTarget::new(&filesystem, 0, &display);
        write_chat_output_artifact_with_provenance_gate(
            &target,
            candidate,
            ReleasedCommand::Morphotag,
        )
        .await
        .expect("gated write");

        let after = std::fs::read_to_string(&host_out).expect("read after");
        assert_eq!(
            after, candidate,
            "primary output must be replaced when the new %mor content differs"
        );
    }

    /// First run (no existing file at the primary output path): the gate
    /// has nothing to compare against and must write through.
    #[tokio::test]
    async fn write_chat_output_artifact_with_provenance_gate_writes_when_target_missing() {
        let root = tempfile::tempdir().expect("tempdir");
        let host_out = root.path().join("host/output/sample.cha");
        // Note: target file does NOT exist yet.

        let staging_dir = root.path().join("jobs/job-1");
        let filesystem = RunnerFilesystemConfig {
            paths_mode: true,
            source_paths: vec![batchalign_types::paths::ClientPath::new(
                root.path()
                    .join("input/sample.cha")
                    .to_string_lossy()
                    .to_string(),
            )],
            output_paths: vec![batchalign_types::paths::ClientPath::new(
                host_out.to_string_lossy().to_string(),
            )],
            before_paths: Vec::new(),
            staging_dir: batchalign_types::paths::ServerPath::from(staging_dir.clone()),
            media_mapping: Default::default(),
            media_subdir: Default::default(),
            source_dir: batchalign_types::paths::ClientPath::new(
                root.path().join("input").to_string_lossy().to_string(),
            ),
        };

        let candidate = "\
@Comment:\t[ba3 morphotag | engine=stanza-1.11.1 ; lang=eng | 2026-05-08T02:52:17-04:00]
*PAR:\thello .
%mor:\tco|hello .
";

        let display = DisplayPath::from("nested/sample.cha");
        let target = ChatOutputTarget::new(&filesystem, 0, &display);
        write_chat_output_artifact_with_provenance_gate(
            &target,
            candidate,
            ReleasedCommand::Morphotag,
        )
        .await
        .expect("gated write");

        let after = std::fs::read_to_string(&host_out).expect("read after");
        assert_eq!(after, candidate);
    }
}
