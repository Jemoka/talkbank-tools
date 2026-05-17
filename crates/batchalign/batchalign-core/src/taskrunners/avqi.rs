//! `AvqiTaskRunner` — AVQI acoustic voice quality index.
//!
//! Terminal task. Mirrors `OpenSmileTaskRunner`: pulls audio from `Media`
//! directly or `Chat::media()`, dispatches, replaces the BAValue with a
//! `MetricsArtifact { kind: Avqi, ... }`.

use crate::base::BAValue;
use crate::base::ProgressEvent;
use crate::base::ProgressSink;
use crate::base::Task;
use crate::base::TaskInput;
use crate::base::{Dispatcher, TaskRunner};
use crate::metrics::{MetricsArtifact, MetricsKind};
use crate::proto::avqi::{AvqiInput, AvqiOutput};
use crate::utils::{BAError, BAResult, prepare_pcm};
use async_trait::async_trait;
use smol_str::SmolStr;

pub struct AvqiTaskRunner;

#[async_trait]
impl TaskRunner for AvqiTaskRunner {
    const TASK: Task = Task::Avqi;

    async fn apply(
        &self,
        value: &mut BAValue,
        dispatcher: &dyn Dispatcher,
        sink: &dyn ProgressSink,
    ) -> BAResult<()> {
        let media = match value {
            BAValue::Media(m) => m.clone(),
            BAValue::Chat(c) => c.media().cloned().ok_or_else(|| {
                BAError::Internal("AvqiTaskRunner: chat has no attached media".into())
            })?,
            BAValue::Failed { .. } => return Ok(()),
            other => {
                return Err(BAError::Internal(format!(
                    "AvqiTaskRunner: expected Media or Chat, got {}",
                    other.kind()
                )));
            }
        };

        sink.emit(ProgressEvent::stage_started(&media.source_id, Task::Avqi));

        let audio = prepare_pcm(&media)
            .map_err(|e| BAError::Internal(format!("audio_prep: {e:#}")))?;

        let input = AvqiInput {
            source_id: media.source_id.clone(),
            audio,
        };

        let output_raw = dispatcher.dispatch(TaskInput::Avqi(input)).await?;
        let output: AvqiOutput = output_raw.try_into()?;

        let sid = media.source_id.clone();
        *value = BAValue::Metrics(MetricsArtifact {
            source_id: media.source_id,
            producer: SmolStr::new_static("avqi:v1"),
            kind: MetricsKind::Avqi,
            table: output.table,
        });

        sink.emit(ProgressEvent::stage_injected(&sid, Task::Avqi));
        Ok(())
    }
}
