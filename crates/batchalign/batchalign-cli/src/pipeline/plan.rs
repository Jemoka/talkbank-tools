//! Minimal sequential pipeline planner.

use std::collections::HashSet;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::time::Instant;

use tracing::info;

use crate::error::ServerError;

/// Boxed async stage future.
pub(crate) type StageFuture<'a> =
    Pin<Box<dyn Future<Output = Result<(), ServerError>> + Send + 'a>>;

/// Async stage function pointer.
pub(crate) type StageFn<C> = for<'a> fn(&'a mut C) -> StageFuture<'a>;

/// Identifiers for internal pipeline stages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum StageId {
    /// Parse input content.
    Parse,
    /// Run pre-validation.
    PreValidate,
    /// Clear existing derived tiers or annotations.
    ClearExisting,
    /// Extract worker payloads.
    CollectPayloads,
    /// Run worker inference.
    Infer,
    /// Apply inference results to the document.
    ApplyResults,
    /// Run post-validation.
    PostValidate,
    /// Run ASR inference.
    AsrInfer,
    /// Run dedicated speaker diarization when requested.
    SpeakerDiarization,
    /// Convert ASR output into utterances.
    AsrPostprocess,
    /// Build CHAT from utterances.
    BuildChat,
    /// Optional utterance segmentation pass.
    OptionalUtseg,
    /// Optional morphosyntax pass.
    OptionalMorphosyntax,
    /// Finalize the output text.
    Serialize,
}

impl StageId {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Parse => "parse",
            Self::PreValidate => "pre_validate",
            Self::ClearExisting => "clear_existing",
            Self::CollectPayloads => "collect_payloads",
            Self::Infer => "infer",
            Self::ApplyResults => "apply_results",
            Self::PostValidate => "post_validate",
            Self::AsrInfer => "asr_infer",
            Self::SpeakerDiarization => "speaker_diarization",
            Self::AsrPostprocess => "asr_postprocess",
            Self::BuildChat => "build_chat",
            Self::OptionalUtseg => "optional_utseg",
            Self::OptionalMorphosyntax => "optional_morphosyntax",
            Self::Serialize => "serialize",
        }
    }
}

impl fmt::Display for StageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Static metadata for a single stage.
pub(crate) struct StageSpec<C> {
    /// Stable stage identifier.
    pub id: StageId,
    /// Other stages that must complete first.
    pub deps: Vec<StageId>,
    /// Whether the stage should be included for the current run.
    pub enabled: fn(&C) -> bool,
    /// Stage implementation.
    pub run: StageFn<C>,
}

impl<C> StageSpec<C> {
    /// Construct a stage specification.
    pub(crate) fn new(
        id: StageId,
        deps: Vec<StageId>,
        enabled: fn(&C) -> bool,
        run: StageFn<C>,
    ) -> Self {
        Self {
            id,
            deps,
            enabled,
            run,
        }
    }
}

/// A concrete pipeline plan for one command.
pub(crate) struct PipelinePlan<C> {
    /// Ordered stage list. The runner validates dependencies before execution.
    pub stages: Vec<StageSpec<C>>,
}

impl<C> PipelinePlan<C> {
    /// Construct a new plan.
    pub(crate) fn new(stages: Vec<StageSpec<C>>) -> Self {
        Self { stages }
    }
}

/// Execute a plan sequentially while respecting declared dependencies.
///
/// If `on_stage` is provided, it is called before each stage executes
/// with `(stage_id, completed_count, total_enabled_count)`.
pub(crate) async fn run_plan<C>(
    command: &'static str,
    plan: &PipelinePlan<C>,
    ctx: &mut C,
    on_stage: Option<&(dyn Fn(StageId, usize, usize) + Send + Sync)>,
) -> Result<Vec<StageId>, ServerError> {
    let mut all_ids = HashSet::new();
    let mut enabled = HashSet::new();

    for stage in &plan.stages {
        if !all_ids.insert(stage.id) {
            return Err(ServerError::Validation(format!(
                "{command} pipeline has duplicate stage id {stage_id}",
                stage_id = stage.id
            )));
        }
        if (stage.enabled)(ctx) {
            enabled.insert(stage.id);
        }
    }

    for stage in &plan.stages {
        if !enabled.contains(&stage.id) {
            continue;
        }
        for dep in &stage.deps {
            if !enabled.contains(dep) {
                return Err(ServerError::Validation(format!(
                    "{command} pipeline stage {stage_id} depends on disabled stage {dep}",
                    stage_id = stage.id
                )));
            }
        }
    }

    let mut completed = HashSet::new();
    let mut executed = Vec::with_capacity(enabled.len());

    while completed.len() < enabled.len() {
        let mut progress = false;

        for stage in &plan.stages {
            if !enabled.contains(&stage.id) || completed.contains(&stage.id) {
                continue;
            }
            if !stage.deps.iter().all(|dep| completed.contains(dep)) {
                continue;
            }

            if let Some(cb) = on_stage {
                cb(stage.id, completed.len(), enabled.len());
            }

            let started = Instant::now();
            info!(command, stage = %stage.id, "Starting pipeline stage");
            (stage.run)(ctx).await?;
            info!(
                command,
                stage = %stage.id,
                duration_ms = started.elapsed().as_millis() as u64,
                "Completed pipeline stage"
            );

            completed.insert(stage.id);
            executed.push(stage.id);
            progress = true;
        }

        if !progress {
            return Err(ServerError::Validation(format!(
                "{command} pipeline could not make progress; check stage dependencies",
            )));
        }
    }

    Ok(executed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct TestContext {
        log: Vec<StageId>,
        enable_optional: bool,
    }

    fn stage_one<'a>(ctx: &'a mut TestContext) -> StageFuture<'a> {
        Box::pin(async move {
            ctx.log.push(StageId::AsrInfer);
            Ok(())
        })
    }

    fn stage_two<'a>(ctx: &'a mut TestContext) -> StageFuture<'a> {
        Box::pin(async move {
            ctx.log.push(StageId::BuildChat);
            Ok(())
        })
    }

    fn stage_optional<'a>(ctx: &'a mut TestContext) -> StageFuture<'a> {
        Box::pin(async move {
            ctx.log.push(StageId::OptionalUtseg);
            Ok(())
        })
    }

    fn always_enabled(_: &TestContext) -> bool {
        true
    }

    fn optional_enabled(ctx: &TestContext) -> bool {
        ctx.enable_optional
    }

    #[tokio::test]
    async fn run_plan_respects_dependencies_and_enabled_flag() {
        let plan = PipelinePlan::new(vec![
            StageSpec::new(
                StageId::BuildChat,
                vec![StageId::AsrInfer],
                always_enabled,
                stage_two,
            ),
            StageSpec::new(StageId::AsrInfer, vec![], always_enabled, stage_one),
            StageSpec::new(
                StageId::OptionalUtseg,
                vec![StageId::BuildChat],
                optional_enabled,
                stage_optional,
            ),
        ]);

        let mut ctx = TestContext::default();
        let executed = run_plan("test", &plan, &mut ctx, None).await.unwrap();

        assert_eq!(executed, vec![StageId::AsrInfer, StageId::BuildChat]);
        assert_eq!(ctx.log, vec![StageId::AsrInfer, StageId::BuildChat]);
    }

    #[tokio::test]
    async fn run_plan_rejects_disabled_dependency() {
        let plan = PipelinePlan::new(vec![
            StageSpec::new(
                StageId::OptionalUtseg,
                vec![StageId::BuildChat],
                always_enabled,
                stage_optional,
            ),
            StageSpec::new(StageId::BuildChat, vec![], optional_enabled, stage_two),
        ]);

        let mut ctx = TestContext::default();
        let err = run_plan("test", &plan, &mut ctx, None).await.unwrap_err();

        assert!(
            err.to_string()
                .contains("depends on disabled stage build_chat")
        );
    }

    #[tokio::test]
    async fn run_plan_rejects_duplicate_stage_ids() {
        let plan = PipelinePlan::new(vec![
            StageSpec::new(StageId::AsrInfer, vec![], always_enabled, stage_one),
            StageSpec::new(StageId::AsrInfer, vec![], always_enabled, stage_one),
        ]);

        let mut ctx = TestContext::default();
        let err = run_plan("test", &plan, &mut ctx, None).await.unwrap_err();

        assert!(err.to_string().contains("duplicate stage id asr_infer"));
    }

    #[tokio::test]
    async fn run_plan_rejects_cycles() {
        let plan = PipelinePlan::new(vec![
            StageSpec::new(
                StageId::AsrInfer,
                vec![StageId::BuildChat],
                always_enabled,
                stage_one,
            ),
            StageSpec::new(
                StageId::BuildChat,
                vec![StageId::AsrInfer],
                always_enabled,
                stage_two,
            ),
        ]);

        let mut ctx = TestContext::default();
        let err = run_plan("test", &plan, &mut ctx, None).await.unwrap_err();

        assert!(
            err.to_string()
                .contains("could not make progress; check stage dependencies")
        );
    }
}
