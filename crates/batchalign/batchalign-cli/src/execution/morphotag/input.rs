use crate::runner::DispatchHostContext;
use crate::store::RunnerJobSnapshot;

pub(super) use crate::execution::text_io::LoadedTextInputs as LoadedMorphotagInputs;

pub(super) async fn load_morphotag_inputs(
    job: &RunnerJobSnapshot,
    host: &DispatchHostContext,
) -> LoadedMorphotagInputs {
    crate::execution::text_io::load_text_inputs(job, host, true).await
}
