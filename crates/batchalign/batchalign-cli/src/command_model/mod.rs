//! Authoritative command-model accessors for released processing commands.
//!
//! Phase 1 keeps the existing recipe metadata structures and moves authority to
//! this module, so the rest of the app stops choosing between parallel command
//! catalogs.

mod catalog;

#[allow(unused_imports)]
pub(crate) use crate::recipe_runner::command_spec::{
    CapabilityPlan, CapabilitySurface, CatalogEntry, CommandFamily, PlannerKind,
};
#[allow(unused_imports)]
pub(crate) use crate::recipe_runner::materialize::{
    FileNamingPolicy, MaterializedArtifactRole, OutputPolicy, PlannedMaterializedFile,
    SidecarPolicy, StemRewrite,
};
#[allow(unused_imports)]
pub(crate) use crate::recipe_runner::recipe::{
    ExecutionMode, Recipe, RecipeStage, RecipeStageId, RecipeStagePresence, StageExecutionKind,
};

#[allow(unused_imports)]
pub(crate) use catalog::{
    command_spec, command_specs, legacy_command_definition, legacy_command_descriptor,
};
