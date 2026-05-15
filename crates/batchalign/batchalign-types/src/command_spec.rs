//! Static per-command metadata. Sole canonical source of resource
//! shape and classification for every released batchalign command.
//!
//! Per the Phase β spec
//! (`docs/architecture/2026-05-10-phase-beta-command-spec.md`),
//! `CommandSpec` is orthogonal to `recipe_runner::CatalogEntry`
//! (planning/execution shape). They cross-reference by
//! `ReleasedCommand` identity.

use crate::api::{MemoryMb, ReleasedCommand};
use crate::worker::InferTask;
use crate::worker_profile::WorkerProfile;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Multiplier applied to the static memory budget to account for
/// transient allocation spikes during model loading. Validated > 1.0
/// at construction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LoadingOverhead(pub f64);

impl LoadingOverhead {
    /// Construct without validation. Use only with literal values in
    /// `const` context. Runtime construction should validate via a
    /// (yet-to-be-added) `try_new` constructor.
    pub const fn new_unchecked(value: f64) -> Self {
        Self(value)
    }
}

/// Whether a command needs a process-isolated worker.
///
/// Trinary because the historical TOML's `process_commands.gil` and
/// `process_commands.free_threaded` lists are NOT mutually exclusive —
/// some commands (`opensmile`, `avqi`, `compare`) appear in both,
/// indicating "needs a process worker in both runtime modes."
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GilProcessNeed {
    /// Needs a process worker regardless of runtime (in BOTH TOML lists).
    Always,
    /// Needs a process worker only when GIL is enabled
    /// (in `process_commands.gil` only).
    OnlyInGilRuntime,
    /// Always thread-safe (in NEITHER list). No commands today; the
    /// variant exists so the type captures the full classification space.
    Never,
}

impl GilProcessNeed {
    /// Whether THIS runtime needs a process worker for the command.
    pub fn needs_process(self, is_free_threaded_runtime: bool) -> bool {
        match (self, is_free_threaded_runtime) {
            (Self::Always, _) => true,
            (Self::OnlyInGilRuntime, false) => true,
            (Self::OnlyInGilRuntime, true) => false,
            (Self::Never, _) => false,
        }
    }
}

/// Static per-command metadata.
///
/// Sole canonical source for resource + classification. The
/// `recipe_runner::CatalogEntry` type holds the orthogonal
/// planning/execution shape; the two cross-reference by
/// `ReleasedCommand` identity.
#[derive(Debug, Clone, Copy)]
pub struct CommandSpec {
    /// Stable released command identity.
    pub name: ReleasedCommand,
    /// Worker infer tasks the command's recipe dispatches to.
    /// Slice rather than scalar for composite commands (benchmark);
    /// today only benchmark would carry a multi-task list, but
    /// every InferTask::Eval-class addition would land here.
    pub tasks: &'static [InferTask],
    /// Literal cmd2task TOML label. Verbatim string emitted to
    /// `runtime_constants.toml::cmd2task` for Python's import-time
    /// dict. Most commands' label equals their first `tasks` element's
    /// snake_case name; `utseg` (label `"utterance"`) and `benchmark`
    /// (label `"asr,eval"`) diverge — see the table in the Phase β
    /// plan, Step 4.1.
    pub task_label: &'static str,
    /// Worker profile classification (Gpu / Stanza / Io).
    pub profile: WorkerProfile,
    /// Whether this command needs a process worker per runtime mode.
    pub gil_process_need: GilProcessNeed,
    /// Base memory budget when running on a process worker.
    pub base_mb_process: MemoryMb,
    /// Base memory budget when running on a thread worker.
    pub base_mb_threaded: MemoryMb,
    /// Multiplier applied to the base budget for the model-loading spike.
    pub loading_overhead: LoadingOverhead,
}

impl CommandSpec {
    /// Pick the base budget appropriate for the runtime.
    pub fn base_mb_for_runtime(&self, is_free_threaded_runtime: bool) -> MemoryMb {
        if is_free_threaded_runtime && !self.gil_process_need.needs_process(true) {
            self.base_mb_threaded
        } else {
            self.base_mb_process
        }
    }
}

// ---------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------

/// The authoritative registry. Exhaustive over `ReleasedCommand`.
///
/// Order matches `ReleasedCommand::ALL` for stable codegen output.
/// All values verified from `python/batchalign/runtime_constants.toml` at plan-write time
/// (2026-05-10).
pub const COMMAND_SPECS: &[CommandSpec] = &[
    CommandSpec {
        name: ReleasedCommand::Align,
        tasks: &[InferTask::Fa],
        task_label: "fa",
        profile: WorkerProfile::Gpu,
        // GPU workers handle process isolation; GIL does not apply.
        // Not in process_commands.{gil,free_threaded} in the TOML.
        gil_process_need: GilProcessNeed::Never,
        base_mb_process: MemoryMb(4_000),
        base_mb_threaded: MemoryMb(4_000),
        loading_overhead: LoadingOverhead::new_unchecked(1.5),
    },
    CommandSpec {
        name: ReleasedCommand::Transcribe,
        tasks: &[InferTask::Asr],
        task_label: "asr",
        profile: WorkerProfile::Gpu,
        // GPU workers handle process isolation; GIL does not apply.
        // Not in process_commands.{gil,free_threaded} in the TOML.
        gil_process_need: GilProcessNeed::Never,
        base_mb_process: MemoryMb(1_500),
        base_mb_threaded: MemoryMb(1_500),
        loading_overhead: LoadingOverhead::new_unchecked(1.5),
    },
    CommandSpec {
        name: ReleasedCommand::TranscribeS,
        tasks: &[InferTask::Asr],
        task_label: "asr",
        profile: WorkerProfile::Gpu,
        // GPU workers handle process isolation; GIL does not apply.
        // Not in process_commands.{gil,free_threaded} in the TOML.
        gil_process_need: GilProcessNeed::Never,
        base_mb_process: MemoryMb(2_500),
        base_mb_threaded: MemoryMb(2_500),
        loading_overhead: LoadingOverhead::new_unchecked(1.5),
    },
    CommandSpec {
        name: ReleasedCommand::Translate,
        tasks: &[InferTask::Translate],
        task_label: "translate",
        profile: WorkerProfile::Io,
        // Io (API-call) worker; no subprocess isolation needed.
        // Not in process_commands.{gil,free_threaded} in the TOML.
        gil_process_need: GilProcessNeed::Never,
        base_mb_process: MemoryMb(4_000),
        base_mb_threaded: MemoryMb(4_000),
        loading_overhead: LoadingOverhead::new_unchecked(1.5),
    },
    CommandSpec {
        name: ReleasedCommand::Morphotag,
        tasks: &[InferTask::Morphosyntax],
        task_label: "morphosyntax",
        profile: WorkerProfile::Stanza,
        gil_process_need: GilProcessNeed::OnlyInGilRuntime,
        base_mb_process: MemoryMb(8_000),
        base_mb_threaded: MemoryMb(2_000),
        loading_overhead: LoadingOverhead::new_unchecked(1.5),
    },
    CommandSpec {
        name: ReleasedCommand::Coref,
        tasks: &[InferTask::Coref],
        task_label: "coref",
        profile: WorkerProfile::Stanza,
        gil_process_need: GilProcessNeed::OnlyInGilRuntime,
        base_mb_process: MemoryMb(4_000),
        base_mb_threaded: MemoryMb(2_000),
        loading_overhead: LoadingOverhead::new_unchecked(1.5),
    },
    CommandSpec {
        name: ReleasedCommand::Utseg,
        tasks: &[InferTask::Utseg],
        // Diverges from InferTask snake_case name: historical TOML label
        // is "utterance", not "utseg". Task 5 codegen emits this verbatim.
        task_label: "utterance",
        profile: WorkerProfile::Stanza,
        gil_process_need: GilProcessNeed::OnlyInGilRuntime,
        base_mb_process: MemoryMb(6_000),
        base_mb_threaded: MemoryMb(2_000),
        loading_overhead: LoadingOverhead::new_unchecked(1.5),
    },
    CommandSpec {
        name: ReleasedCommand::Benchmark,
        tasks: &[InferTask::Asr],
        // Composite label: benchmark runs ASR + eval. InferTask::Eval doesn't
        // exist (it's a string label in cmd2task only), so tasks carries only
        // the typed InferTask. The task_label is preserved verbatim for
        // Task 5 codegen's cmd2task projection.
        task_label: "asr,eval",
        profile: WorkerProfile::Gpu,
        // GPU workers handle process isolation; GIL does not apply.
        // Not in process_commands.{gil,free_threaded} in the TOML.
        gil_process_need: GilProcessNeed::Never,
        base_mb_process: MemoryMb(1_500),
        base_mb_threaded: MemoryMb(1_500),
        loading_overhead: LoadingOverhead::new_unchecked(1.5),
    },
    CommandSpec {
        name: ReleasedCommand::Opensmile,
        tasks: &[InferTask::Opensmile],
        task_label: "opensmile",
        profile: WorkerProfile::Io,
        // In both process_commands.gil and process_commands.free_threaded —
        // needs process isolation regardless of runtime.
        gil_process_need: GilProcessNeed::Always,
        base_mb_process: MemoryMb(500),
        base_mb_threaded: MemoryMb(500),
        loading_overhead: LoadingOverhead::new_unchecked(1.5),
    },
    CommandSpec {
        name: ReleasedCommand::Compare,
        tasks: &[InferTask::Morphosyntax],
        task_label: "morphosyntax",
        profile: WorkerProfile::Stanza,
        // In both process_commands.gil and process_commands.free_threaded.
        gil_process_need: GilProcessNeed::Always,
        base_mb_process: MemoryMb(8_000),
        base_mb_threaded: MemoryMb(2_000),
        loading_overhead: LoadingOverhead::new_unchecked(1.5),
    },
    CommandSpec {
        name: ReleasedCommand::Avqi,
        tasks: &[InferTask::Avqi],
        task_label: "avqi",
        profile: WorkerProfile::Io,
        // In both process_commands.gil and process_commands.free_threaded.
        gil_process_need: GilProcessNeed::Always,
        base_mb_process: MemoryMb(1_500),
        base_mb_threaded: MemoryMb(1_500),
        loading_overhead: LoadingOverhead::new_unchecked(1.5),
    },
];

// ---------------------------------------------------------------------------
// Lookup
// ---------------------------------------------------------------------------

/// Lookup helper. Panics if the registry is missing an entry —
/// the exhaustive-coverage test catches that at test time.
#[allow(clippy::expect_used)]
pub fn command_spec_for(name: ReleasedCommand) -> &'static CommandSpec {
    COMMAND_SPECS
        .iter()
        .find(|s| s.name == name)
        .expect("released command missing from COMMAND_SPECS — see exhaustive-coverage test")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::ReleasedCommand;
    use crate::worker::InferTask;
    use crate::worker_profile::WorkerProfile;

    /// Catalog invariant: every released command has an entry.
    /// Adding a new ReleasedCommand variant fails here.
    #[test]
    fn every_released_command_has_a_spec() {
        for command in &ReleasedCommand::ALL {
            let spec = command_spec_for(*command);
            assert_eq!(
                spec.name, *command,
                "registry entry for {command:?} has wrong name"
            );
        }
    }

    /// Spot-check a few commands to ensure the registry's classifications
    /// match the historical TOML.
    #[test]
    fn morphotag_classification_matches_toml() {
        let spec = command_spec_for(ReleasedCommand::Morphotag);
        assert_eq!(spec.profile, WorkerProfile::Stanza);
        assert_eq!(spec.gil_process_need, GilProcessNeed::OnlyInGilRuntime);
        assert_eq!(spec.base_mb_process.0, 8_000);
        assert_eq!(spec.base_mb_threaded.0, 2_000);
        assert_eq!(spec.tasks, &[InferTask::Morphosyntax]);
        assert_eq!(spec.task_label, "morphosyntax");
    }

    #[test]
    fn opensmile_classification_matches_toml() {
        let spec = command_spec_for(ReleasedCommand::Opensmile);
        assert_eq!(spec.profile, WorkerProfile::Io);
        assert_eq!(spec.gil_process_need, GilProcessNeed::Always);
    }

    #[test]
    fn benchmark_carries_composite_task_label() {
        let spec = command_spec_for(ReleasedCommand::Benchmark);
        assert_eq!(spec.task_label, "asr,eval");
        assert_eq!(spec.tasks, &[InferTask::Asr]);
    }

    #[test]
    fn utseg_task_label_diverges_from_infer_task_name() {
        let spec = command_spec_for(ReleasedCommand::Utseg);
        assert_eq!(
            spec.task_label, "utterance",
            "utseg's cmd2task TOML value is the historical 'utterance', \
             not the snake_case InferTask name"
        );
        assert_eq!(spec.tasks, &[InferTask::Utseg]);
    }
}
