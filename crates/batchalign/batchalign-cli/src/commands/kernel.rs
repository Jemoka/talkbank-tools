//! Resource-aware kernel planning for command-owned execution.

use crate::ReleasedCommand;
use crate::api::MemoryMb;
use crate::host_policy::HostExecutionPolicy;
use crate::types::runtime;
use crate::worker::WorkerBootstrapMode;

use super::catalog::released_command_definition;
use super::spec::{
    BatchingPolicy, ConstrainedHostPolicy, ModelSharingPolicy, ParallelismPolicy, ResourceLane,
    SchedulingPolicy, WarmupPolicy,
};

/// Kernel-facing worker-lane hint derived from one command's performance
/// profile and the current runtime caps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExecutionLaneHint {
    /// GPU-bound lane with a hard worker cap.
    Gpu {
        /// Maximum concurrent GPU-backed workers.
        max_workers: usize,
    },
    /// CPU lane where either process or free-threaded workers may matter.
    Cpu {
        /// Hard cap for process-isolated workers.
        max_process_workers: usize,
        /// Hard cap for free-threaded/shared-model workers.
        max_thread_workers: usize,
    },
    /// IO/media lane where thread capacity is the dominant limit.
    Io {
        /// Hard cap for the lane.
        max_workers: usize,
    },
    /// Mixed lane touching both CPU and GPU stages.
    Mixed {
        /// Maximum concurrent GPU-backed workers.
        max_gpu_workers: usize,
        /// Hard cap for free-threaded/shared-model workers.
        max_thread_workers: usize,
    },
    /// Composite commands delegate lane selection to child commands.
    Delegated,
}

impl ExecutionLaneHint {
    fn max_parallelism(self) -> usize {
        match self {
            Self::Gpu { max_workers } => max_workers.max(1),
            Self::Cpu {
                max_process_workers,
                max_thread_workers,
            } => max_process_workers.max(max_thread_workers).max(1),
            Self::Io { max_workers } => max_workers.max(1),
            Self::Mixed {
                max_gpu_workers,
                max_thread_workers,
            } => max_gpu_workers.max(max_thread_workers).max(1),
            Self::Delegated => 1,
        }
    }
}

/// Resource-aware kernel plan for one released command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CommandKernelPlan {
    /// Released command the plan was derived for.
    pub command: ReleasedCommand,
    /// High-level scheduling shape requested by the command.
    pub scheduling: SchedulingPolicy,
    /// How the command expects model state to be shared.
    pub model_sharing: ModelSharingPolicy,
    /// Whether the command benefits from batching.
    pub batching: BatchingPolicy,
    /// Dominant resource lane for the command.
    pub resource_lane: ResourceLane,
    /// How the command should behave on constrained-memory hosts.
    pub constrained_host: ConstrainedHostPolicy,
    /// Whether the command is eligible for speculative/background warmup.
    pub warmup: WarmupPolicy,
    /// Actual worker bootstrap mode chosen for this host.
    pub worker_bootstrap: WorkerBootstrapMode,
    /// Kernel-facing worker-lane hint.
    pub execution_lane: ExecutionLaneHint,
    /// Suggested file-level parallelism bound for this command and file count.
    pub file_parallelism_hint: usize,
    /// Per-command execution reservation from runtime constants.
    pub execution_budget_mb: MemoryMb,
    /// Additional per-file working-set budget from runtime constants.
    pub per_file_buffer_mb: MemoryMb,
    /// Whether host-memory admission must stay enabled.
    pub uses_host_memory_gate: bool,
}

impl CommandKernelPlan {
    /// Derive a resource-aware plan for one released command and discovered file
    /// count.
    #[cfg(test)]
    pub(crate) fn for_command(command: ReleasedCommand, file_count: usize) -> Self {
        Self::for_command_with_policy(command, file_count, &HostExecutionPolicy::default())
    }

    /// Derive a resource-aware plan under one explicit host policy.
    pub(crate) fn for_command_with_policy(
        command: ReleasedCommand,
        file_count: usize,
        host_policy: &HostExecutionPolicy,
    ) -> Self {
        let definition = released_command_definition(command);
        let execution_lane = execution_lane_for(
            definition.resource_lane(),
            definition.model_sharing_policy(),
        );
        let file_parallelism_hint = host_policy.resolved_file_parallelism(
            definition.constrained_host_policy(),
            suggested_parallelism(definition.parallelism_policy(), execution_lane, file_count),
        );

        Self {
            command,
            scheduling: definition.scheduling_policy(),
            model_sharing: definition.model_sharing_policy(),
            batching: definition.batching_policy(),
            resource_lane: definition.resource_lane(),
            constrained_host: definition.constrained_host_policy(),
            warmup: definition.warmup_policy(),
            worker_bootstrap: host_policy.bootstrap_mode,
            execution_lane,
            file_parallelism_hint,
            execution_budget_mb: runtime::command_execution_budget_mb(command.as_ref()),
            per_file_buffer_mb: runtime::mb_per_file_mb(),
            uses_host_memory_gate: definition.uses_host_memory_gate(),
        }
    }
}

fn execution_lane_for(
    resource_lane: ResourceLane,
    model_sharing: ModelSharingPolicy,
) -> ExecutionLaneHint {
    match model_sharing {
        ModelSharingPolicy::DelegatedToSubcommands => ExecutionLaneHint::Delegated,
        ModelSharingPolicy::SharedWarmWorkers => match resource_lane {
            ResourceLane::GpuHeavy => ExecutionLaneHint::Gpu {
                max_workers: runtime::max_gpu_workers(),
            },
            ResourceLane::CpuBound => ExecutionLaneHint::Cpu {
                max_process_workers: runtime::max_process_workers(),
                max_thread_workers: runtime::max_thread_workers(),
            },
            ResourceLane::IoBound => ExecutionLaneHint::Io {
                max_workers: runtime::max_thread_workers(),
            },
            ResourceLane::Mixed => ExecutionLaneHint::Mixed {
                max_gpu_workers: runtime::max_gpu_workers(),
                max_thread_workers: runtime::max_thread_workers(),
            },
        },
    }
}

fn suggested_parallelism(
    parallelism: ParallelismPolicy,
    execution_lane: ExecutionLaneHint,
    file_count: usize,
) -> usize {
    let discovered_files = file_count.max(1);
    match parallelism {
        ParallelismPolicy::SingleDispatchPerJob | ParallelismPolicy::DelegatedToSubcommands => 1,
        ParallelismPolicy::BoundedFileWorkers => {
            discovered_files.min(execution_lane.max_parallelism())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CommandKernelPlan, ExecutionLaneHint};
    use crate::ReleasedCommand;
    use crate::commands::spec::{
        BatchingPolicy, ConstrainedHostPolicy, SchedulingPolicy, WarmupPolicy,
    };
    use crate::host_policy::HostExecutionPolicy;
    use crate::types::runtime;
    use crate::worker::WorkerBootstrapMode;

    #[test]
    fn transcribe_kernel_plan_keeps_gpu_parallelism_cap() {
        let plan = CommandKernelPlan::for_command(ReleasedCommand::Transcribe, 99);
        assert_eq!(plan.scheduling, SchedulingPolicy::PerFileAudio);
        assert_eq!(plan.batching, BatchingPolicy::InternalStageBatching);
        assert_eq!(
            plan.execution_lane,
            ExecutionLaneHint::Gpu {
                max_workers: runtime::max_gpu_workers(),
            }
        );
        assert_eq!(
            plan.constrained_host,
            ConstrainedHostPolicy::SequentialFallback
        );
        assert_eq!(plan.warmup, WarmupPolicy::BackgroundEligible);
        assert_eq!(plan.worker_bootstrap, WorkerBootstrapMode::Profile);
        assert_eq!(
            plan.file_parallelism_hint,
            runtime::max_gpu_workers().max(1)
        );
    }

    #[test]
    fn morphotag_kernel_plan_stays_single_dispatch_batch() {
        let plan = CommandKernelPlan::for_command(ReleasedCommand::Morphotag, 12);
        assert_eq!(plan.scheduling, SchedulingPolicy::CrossFileBatch);
        assert_eq!(plan.batching, BatchingPolicy::CrossFileBatch);
        assert_eq!(
            plan.constrained_host,
            ConstrainedHostPolicy::SequentialFallback
        );
        assert_eq!(plan.file_parallelism_hint, 1);
        assert!(plan.uses_host_memory_gate);
    }

    #[test]
    fn compare_kernel_plan_keeps_paired_profile() {
        let plan = CommandKernelPlan::for_command(ReleasedCommand::Compare, 3);
        assert_eq!(plan.scheduling, SchedulingPolicy::ReferenceProjection);
        assert_eq!(plan.batching, BatchingPolicy::PairedInputs);
        assert_eq!(
            plan.constrained_host,
            ConstrainedHostPolicy::SequentialFallback
        );
        assert_eq!(plan.file_parallelism_hint, 1);
        assert_eq!(
            plan.execution_budget_mb,
            runtime::command_execution_budget_mb("compare")
        );
    }

    #[test]
    fn benchmark_kernel_plan_delegates_small_host_policy() {
        let plan = CommandKernelPlan::for_command(ReleasedCommand::Benchmark, 4);
        assert_eq!(
            plan.constrained_host,
            ConstrainedHostPolicy::DelegatedToSubcommands
        );
        assert_eq!(plan.warmup, WarmupPolicy::DelegatedToSubcommands);
    }

    #[test]
    fn small_host_plan_clamps_transcribe_to_single_file() {
        let host =
            HostExecutionPolicy::from_memory_tier(runtime::MemoryTier::from_total_mb(16_000));
        let plan =
            CommandKernelPlan::for_command_with_policy(ReleasedCommand::Transcribe, 8, &host);
        assert_eq!(plan.file_parallelism_hint, 1);
        assert_eq!(plan.worker_bootstrap, WorkerBootstrapMode::Task);
    }
}
