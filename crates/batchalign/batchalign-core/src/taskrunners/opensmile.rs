//! `OpenSmileTaskRunner` — extracts eGeMAPS / ComParE features into a
//! `MetricsArtifact`.
//!
//! Reads audio from either `BAValue::Media` directly or, post-ASR, from the
//! `MediaInput` attached to `BAValue::Chat`. Terminal task: replaces the
//! incoming value with `BAValue::Metrics(...)`.

use crate::base::BAValue;
use crate::base::ProgressEvent;
use crate::base::ProgressSink;
use crate::base::Task;
use crate::base::TaskInput;
use crate::base::{Dispatcher, TaskRunner};
use crate::metrics::{MetricsArtifact, MetricsKind};
use crate::proto::opensmile::{OpenSmileInput, OpenSmileOutput};
use crate::utils::{BAError, BAResult, prepare_pcm};
use async_trait::async_trait;
use smol_str::SmolStr;

pub struct OpenSmileTaskRunner;

#[async_trait]
impl TaskRunner for OpenSmileTaskRunner {
    const TASK: Task = Task::OpenSmile;

    async fn apply(
        &self,
        value: &mut BAValue,
        dispatcher: &dyn Dispatcher,
        sink: &dyn ProgressSink,
    ) -> BAResult<()> {
        let media = match value {
            BAValue::Media(m) => m.clone(),
            BAValue::Chat(c) => c.media().cloned().ok_or_else(|| {
                BAError::Internal("OpenSmileTaskRunner: chat has no attached media".into())
            })?,
            BAValue::Failed { .. } => return Ok(()),
            other => {
                return Err(BAError::Internal(format!(
                    "OpenSmileTaskRunner: expected Media or Chat, got {}",
                    other.kind()
                )));
            }
        };

        sink.emit(ProgressEvent::stage_started(&media.source_id, Task::OpenSmile));

        let audio = prepare_pcm(&media)
            .map_err(|e| BAError::Internal(format!("audio_prep: {e:#}")))?;

        let input = OpenSmileInput {
            source_id: media.source_id.clone(),
            audio,
            // The default eGeMAPS feature set lives on the input only as a
            // hint; backends select the actual set via their own constructor
            // (`OpenSmileBackend(feature_set="ComParE_2016")`).
            feature_set: SmolStr::new_static("eGeMAPSv02"),
        };

        let output_raw = dispatcher.dispatch(TaskInput::OpenSmile(input)).await?;
        let output: OpenSmileOutput = output_raw.try_into()?;

        let sid = media.source_id.clone();
        *value = BAValue::Metrics(MetricsArtifact {
            source_id: media.source_id,
            producer: SmolStr::new(format!("opensmile:{}", output.feature_set)),
            kind: MetricsKind::Opensmile,
            table: output.table,
        });

        sink.emit(ProgressEvent::stage_injected(&sid, Task::OpenSmile));
        Ok(())
    }
}
