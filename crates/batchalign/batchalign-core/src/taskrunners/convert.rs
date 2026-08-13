//! `ConvertTaskRunner` — media decode and native encoder dispatch glue.

use crate::base::{BAValue, Dispatcher, ProgressEvent, ProgressSink, Task, TaskInput, TaskRunner};
use crate::proto::convert::{ConvertInput, MediaOutput};
use crate::utils::{BAError, BAResult, prepare_pcm_interleaved};
use async_trait::async_trait;
use std::sync::Arc;

pub struct ConvertTaskRunner;

#[async_trait]
impl TaskRunner for ConvertTaskRunner {
    const TASK: Task = Task::Convert;

    async fn apply(
        &self,
        value: &mut BAValue,
        dispatcher: &dyn Dispatcher,
        sink: Arc<dyn ProgressSink>,
    ) -> BAResult<()> {
        let media = match value {
            BAValue::Media(media) => media.clone(),
            BAValue::Failed { .. } => return Ok(()),
            other => {
                return Err(BAError::Internal(format!(
                    "ConvertTaskRunner expected BAValue::Media, got {}",
                    other.kind()
                )));
            }
        };

        sink.emit(ProgressEvent::stage_started(&media.source_id, Task::Convert));
        let audio = prepare_pcm_interleaved(&media)
            .map_err(|err| BAError::Internal(format!("convert audio decode: {err:#}")))?;
        let output: MediaOutput = dispatcher
            .dispatch(TaskInput::Convert(ConvertInput {
                source_id: media.source_id.clone(),
                audio,
            }))
            .await?
            .try_into()?;
        *value = BAValue::MediaOutput(output);
        sink.emit(ProgressEvent::stage_injected(&media.source_id, Task::Convert));
        Ok(())
    }
}
