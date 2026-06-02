//! `UtrTaskRunner` — Utterance Timing Recovery.
//!
//! **STUB**: Task 4 replaces this with the real implementation that
//! decodes the sibling audio, dispatches an ASR-shaped backend call,
//! converts the response to `AsrTimingToken`s, runs the
//! Global/TwoPass Hirschberg strategy from `taskrunners/utr/`, and
//! injects `BulletSource::Utr` bullets onto untimed utterances.
//!
//! For now this just emits a `StageSkipped` event so a pipeline that
//! declares `Task::Utr` compiles and runs end-to-end against an
//! already-timed CHAT.

use crate::base::BAValue;
use crate::base::ProgressEvent;
use crate::base::ProgressKind;
use crate::base::ProgressSink;
use crate::base::Task;
use crate::base::{Dispatcher, TaskRunner};
use crate::utils::{BAError, BAResult};
use async_trait::async_trait;

pub struct UtrTaskRunner;

#[async_trait]
impl TaskRunner for UtrTaskRunner {
    const TASK: Task = Task::Utr;

    async fn apply(
        &self,
        value: &mut BAValue,
        _dispatcher: &dyn Dispatcher,
        sink: std::sync::Arc<dyn ProgressSink>,
    ) -> BAResult<()> {
        let chat = match value {
            BAValue::Chat(c) => c,
            BAValue::Failed { .. } => return Ok(()),
            other => {
                return Err(BAError::Internal(format!(
                    "UtrTaskRunner: expected BAValue::Chat, got {}",
                    other.kind()
                )));
            }
        };
        sink.emit(ProgressEvent {
            source_id: chat.source_id().clone(),
            task: Some(Task::Utr),
            kind: ProgressKind::StageSkipped,
            completed: 0,
            total: 0,
            label: "UTR not yet implemented".into(),
        });
        Ok(())
    }
}
