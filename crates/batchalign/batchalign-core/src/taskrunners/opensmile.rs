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
use serde::Deserialize;
use smol_str::SmolStr;

pub struct OpenSmileTaskRunner;

#[derive(Clone, Debug, Deserialize)]
pub struct OpenSmileConfig {
    #[serde(default = "default_feature_set")]
    pub feature_set: SmolStr,
}

fn default_feature_set() -> SmolStr {
    SmolStr::new_static("eGeMAPSv02")
}

impl Default for OpenSmileConfig {
    fn default() -> Self {
        Self {
            feature_set: default_feature_set(),
        }
    }
}

#[async_trait]
impl TaskRunner for OpenSmileTaskRunner {
    const TASK: Task = Task::OpenSmile;
    type Config = OpenSmileConfig;

    async fn apply(
        &self,
        cfg: &Self::Config,
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
            feature_set: cfg.feature_set.clone(),
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
