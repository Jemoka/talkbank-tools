//! Worker profile domain type.
//!
//! A [`WorkerProfile`] groups related [`crate::worker::InferTask`]s into fewer
//! Python worker processes so loaded models can be shared within a single process.
//!
//! This module holds the profile enum and the methods that depend only on types
//! already in `batchalign-types`.  Methods that depend on `batchalign`-internal
//! infrastructure (runtime free-threaded flag, command catalog) live as free
//! functions in `batchalign::worker::target`.  They will be folded back here as
//! a method in Phase β Task 2, once `MemoryTier` also lives in this crate.

use crate::api::ReleasedCommand;
use crate::worker::InferTask;

/// Worker profile grouping related [`InferTask`]s into fewer processes.
///
/// Instead of spawning one worker per `InferTask`, profiles group related tasks
/// so that loaded models are shared within a single process:
///
/// - **Gpu**: ASR, FA, Speaker — GPU-bound models, concurrent via Python
///   `ThreadPoolExecutor` (PyTorch releases the GIL during CUDA kernels).
///   Max 1 process per (lang, engine_overrides) key.
/// - **Stanza**: Morphosyntax, Utseg, Coref — Stanza NLP processors, sequential
///   per process. Multiple processes for CPU parallelism (auto-tuned).
/// - **Io**: Translate, OpenSMILE, AVQI — lightweight API/library calls.
///   Max 1 process per key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkerProfile {
    /// GPU-bound models (ASR, FA, Speaker). Concurrent via threads inside one process.
    Gpu,
    /// Stanza NLP processors (Morphosyntax, Utseg, Coref). Multi-process for CPU parallelism.
    Stanza,
    /// Lightweight API/library calls (Translate, OpenSMILE, AVQI).
    Io,
}

impl WorkerProfile {
    /// Map one [`InferTask`] to its profile.
    pub fn for_task(task: InferTask) -> Self {
        match task {
            InferTask::Asr | InferTask::Fa | InferTask::Speaker => Self::Gpu,
            InferTask::Morphosyntax | InferTask::Utseg | InferTask::Coref => Self::Stanza,
            InferTask::Translate | InferTask::Opensmile | InferTask::Avqi => Self::Io,
        }
    }

    /// The string label used in logs and worker keys.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Gpu => "profile:gpu",
            Self::Stanza => "profile:stanza",
            Self::Io => "profile:io",
        }
    }

    /// The profile name used in the ``--profile`` CLI arg sent to Python.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Gpu => "gpu",
            Self::Stanza => "stanza",
            Self::Io => "io",
        }
    }

    /// Parse a profile name from a CLI argument or registry entry.
    ///
    /// Returns `None` for unrecognized names.
    pub fn try_from_name(name: &str) -> Option<Self> {
        match name {
            "gpu" => Some(Self::Gpu),
            "stanza" => Some(Self::Stanza),
            "io" => Some(Self::Io),
            _ => None,
        }
    }

    /// Like `is_concurrent`, but takes an explicit free-threaded flag.
    ///
    /// Use this in tests or contexts where the runtime flag is supplied externally.
    /// The non-parametric `is_concurrent()` variant lives in `batchalign::worker::target`
    /// because it calls `batchalign`'s `is_free_threaded_runtime()`.
    ///
    /// GPU workers always use concurrent serving (PyTorch releases the GIL during
    /// CUDA kernels). Stanza workers use concurrent serving only when running on
    /// free-threaded Python 3.14t, where OS threads share one model instance
    /// instead of each process holding a full private copy.
    pub fn is_concurrent_for_runtime(&self, free_threaded: bool) -> bool {
        match self {
            Self::Gpu => true,
            // Stanza workers share one model via ThreadPoolExecutor on 3.14t,
            // giving the same throughput as separate processes with 77% less
            // memory (see python-versioning.md benchmarks, 2026-02-19).
            Self::Stanza => free_threaded,
            Self::Io => false,
        }
    }

    /// Default maximum worker processes per ``(profile, lang, engine_overrides)`` key.
    ///
    /// GPU: 1 process (concurrent via threads).
    /// Stanza: `auto_tune` (multiple processes for CPU parallelism).
    /// IO: 1 process (lightweight).
    pub fn default_max_workers(&self, auto_tune: usize) -> usize {
        match self {
            Self::Gpu => 1,
            Self::Stanza => auto_tune,
            Self::Io => 1,
        }
    }

    /// Per-tier startup reservation (MB) for this profile.
    ///
    /// Restored to method form in Phase β Task 2 once `MemoryTier`
    /// joined `WorkerProfile` in `batchalign-types`.
    pub fn startup_reservation_mb_for_tier(
        &self,
        tier: &crate::memory::MemoryTier,
    ) -> crate::api::MemoryMb {
        match self {
            Self::Gpu => tier.gpu_startup_mb,
            Self::Stanza => tier.stanza_startup_mb,
            Self::Io => tier.io_startup_mb,
        }
    }

    /// Map a released command name to the profile needed for that command's infer-task worker.
    ///
    /// Returns `None` for commands that do not require an infer-task worker.
    ///
    /// Note: the mapping is embedded here to avoid a dependency on `batchalign`'s command
    /// catalog.  It mirrors the `primary_infer_task` logic in `batchalign::command_model`
    /// (`catalog.rs:primary_infer_task`), which selects the first entry from each command's
    /// `infer_tasks` list.  If the catalog changes, this mapping must be kept in sync.
    pub fn for_command(command: ReleasedCommand) -> Option<Self> {
        let task = match command {
            ReleasedCommand::Morphotag => InferTask::Morphosyntax,
            ReleasedCommand::Utseg => InferTask::Utseg,
            ReleasedCommand::Translate => InferTask::Translate,
            ReleasedCommand::Coref => InferTask::Coref,
            ReleasedCommand::Align => InferTask::Fa,
            ReleasedCommand::Transcribe | ReleasedCommand::TranscribeS => InferTask::Asr,
            ReleasedCommand::Opensmile => InferTask::Opensmile,
            ReleasedCommand::Avqi => InferTask::Avqi,
            // Compare: primary infer task is Morphosyntax (MORPHOSYNTAX_TASKS[0]).
            ReleasedCommand::Compare => InferTask::Morphosyntax,
            // Benchmark: primary infer task is Asr (BENCHMARK_TASKS[0]).
            ReleasedCommand::Benchmark => InferTask::Asr,
        };
        Some(Self::for_task(task))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worker::InferTask;

    #[test]
    fn worker_profile_for_task_classifications_match_today() {
        assert_eq!(WorkerProfile::for_task(InferTask::Asr), WorkerProfile::Gpu);
        assert_eq!(WorkerProfile::for_task(InferTask::Fa), WorkerProfile::Gpu);
        assert_eq!(
            WorkerProfile::for_task(InferTask::Speaker),
            WorkerProfile::Gpu
        );
        assert_eq!(
            WorkerProfile::for_task(InferTask::Morphosyntax),
            WorkerProfile::Stanza
        );
        assert_eq!(
            WorkerProfile::for_task(InferTask::Utseg),
            WorkerProfile::Stanza
        );
        assert_eq!(
            WorkerProfile::for_task(InferTask::Coref),
            WorkerProfile::Stanza
        );
        assert_eq!(
            WorkerProfile::for_task(InferTask::Translate),
            WorkerProfile::Io
        );
        assert_eq!(
            WorkerProfile::for_task(InferTask::Opensmile),
            WorkerProfile::Io
        );
        assert_eq!(WorkerProfile::for_task(InferTask::Avqi), WorkerProfile::Io);
    }
}
