//! batchalign-core — types and traits for the batchalign rewrite (spec2.md).
//!
//! Layout:
//! - `utils` — errors, `SourceId`, media-input shapes, PCM preparation.
//! - `base` — `Task` DAG, runner/dispatcher traits, `TaskInput`/`TaskOutput`
//!   unions, the `BAValue` flow type (with `Paired` inlined), the typestate
//!   `Chat<S>` wrapper, and the progress-event channel.
//! - `metrics` — terminal-task table/artifact types.
//! - `proto` — closed wire types (hand-mirrored with `python/batchalign/_core/proto.py`).
//! - `backends` — `Backend` trait and Rust-side backend implementations
//!   (currently: `compare`, the pure-AST Compare task).
//! - `taskrunners` — concrete `TaskRunner` implementations, one per `Task`.
//! - `pipeline` — orchestrator that wires runners + backends + cache + engine
//!   together. Moved here from `batchalign-engine`.

#![allow(dead_code)]

pub mod backends;
pub mod base;
pub mod metrics;
pub mod proto;
pub mod taskrunners;
pub mod utils;

#[cfg(feature = "python")]
pub mod python;

pub use backends::{Backend, BackendCallError, BackendMeta, BatchPolicy};
pub use base::{
    BAValue, Chat, Dispatcher, DynTaskRunner, NotValidated, Paired, ProgressEvent, ProgressKind,
    ProgressSink, Task, TaskInput, TaskOutput, TaskRunner, Validated,
};
pub use metrics::{MetricsArtifact, MetricsKind, MetricsRow, MetricsTable};
pub use utils::{
    AudioError, BAError, BAResult, MediaInput, PreparedAudio, SourceId, SpeakerLabel, prepare_pcm,
};
