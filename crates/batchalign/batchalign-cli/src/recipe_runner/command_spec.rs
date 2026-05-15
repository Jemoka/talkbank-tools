//! `CatalogEntry` — static metadata for one entry in the
//! `recipe_command_catalog()`.
//!
//! Renamed from `CommandSpec` in Phase β to free the `CommandSpec`
//! name for the public `batchalign-types::command_spec::CommandSpec`
//! (resource/classification spec). The two types are orthogonal,
//! cross-referenced by `ReleasedCommand` identity. See
//! `docs/architecture/2026-05-10-phase-beta-command-spec.md`.

use crate::api::ReleasedCommand;
use crate::worker::InferTask;

use super::materialize::OutputPolicy;
use super::recipe::{ExecutionMode, Recipe};

/// High-level command family in the replacement architecture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CommandFamily {
    /// Main transcript plus reference transcript projection.
    ReferenceProjection,
    /// Audio-first sequential recipes such as transcribe and align.
    AudioSequential,
    /// Cross-unit text commands that still expose per-file results.
    BatchedText,
    /// Composite commands that reuse other recipes.
    Composite,
    /// Media-analysis commands that emit non-CHAT artifacts.
    MediaAnalysis,
}

/// Which planner shape owns source discovery for a command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PlannerKind {
    /// Plain CHAT inputs with optional `--before`.
    TextInputs,
    /// Plain audio inputs.
    AudioInputs,
    /// Main transcript + gold companion pairing.
    ComparePairs,
    /// Audio input + derived gold CHAT pairing.
    BenchmarkPairs,
    /// Media-analysis audio inputs.
    MediaAnalysisInputs,
}

/// Whether a released command is owned directly by one recipe or by recipe composition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CapabilitySurface {
    /// One recipe owns the released command.
    RecipeOwned,
    /// The released command is defined by composing other recipes.
    Composite,
}

/// Worker-capability requirements for one released command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CapabilityPlan {
    /// Worker infer tasks required somewhere in the recipe.
    pub infer_tasks: &'static [InferTask],
    /// Whether the released command is recipe-owned or composed.
    pub surface: CapabilitySurface,
}

/// Static command metadata for the recipe-runner catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CatalogEntry {
    /// Stable released command identity.
    pub command: ReleasedCommand,
    /// High-level family for contributor understanding.
    pub family: CommandFamily,
    /// Planner shape used to derive work units.
    pub planner: PlannerKind,
    /// Execution mode surfaced to the runtime.
    pub execution_mode: ExecutionMode,
    /// Worker capability requirements.
    pub capabilities: CapabilityPlan,
    /// Output naming and sidecar policy.
    pub output_policy: OutputPolicy,
    /// Ordered stage recipe for the command.
    pub recipe: &'static Recipe,
}
